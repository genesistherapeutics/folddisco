use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::index::lookup::LookupTable;

/// Release dates in lookup ordinal order, matching posting values.
pub struct ReleaseDates {
    dates: Vec<Option<u32>>,
}

impl ReleaseDates {
    pub fn load(path: &Path, lookup: &LookupTable) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|err| format!("Cannot open release dates {}: {err}", path.display()))?;
        Self::read(BufReader::new(file), lookup)
            .map_err(|err| format!("Invalid release dates {}: {err}", path.display()))
    }

    fn read(reader: impl BufRead, lookup: &LookupTable) -> Result<Self, String> {
        let mut ordinals = HashMap::with_capacity(lookup.len());
        for (ordinal, name) in lookup.names().enumerate() {
            // Candidate accumulation and output lookup must address the same target.
            if lookup.entry(ordinal).id != ordinal {
                return Err(format!(
                    "release-date filtering requires lookup IDs to equal row ordinals; target {name:?} at ordinal {ordinal} has ID {}",
                    lookup.entry(ordinal).id
                ));
            }
            if ordinals.insert(name, ordinal).is_some() {
                return Err(format!("duplicate lookup target {name:?}"));
            }
        }
        let mut lines = reader.lines();
        let header = lines
            .next()
            .ok_or("missing target\\trelease_date header")?
            .map_err(|err| err.to_string())?;
        if header != "target\trelease_date" {
            return Err("expected header target\\trelease_date".to_string());
        }
        let mut dates = vec![None; lookup.len()];
        let mut seen = vec![false; lookup.len()];
        for (index, line) in lines.enumerate() {
            let line_number = index + 2;
            let line = line.map_err(|err| format!("line {line_number}: {err}"))?;
            let (name, value) = line
                .split_once('\t')
                .filter(|(name, value)| !name.is_empty() && !value.contains('\t'))
                .ok_or_else(|| {
                    format!("line {line_number}: expected exactly two tab-separated fields")
                })?;
            let &ordinal = ordinals
                .get(name)
                .ok_or_else(|| format!("line {line_number}: unknown target {name:?}"))?;
            if seen[ordinal] {
                return Err(format!("line {line_number}: duplicate target {name:?}"));
            }
            dates[ordinal] = if value == "?" {
                None
            } else {
                Some(parse_date(value).map_err(|err| format!("line {line_number}: {err}"))?)
            };
            seen[ordinal] = true;
        }
        if let Some(ordinal) = seen.iter().position(|present| !present) {
            return Err(format!(
                "missing release date for target {:?}; use ? for unknown dates",
                lookup.name(ordinal)
            ));
        }
        Ok(Self { dates })
    }

    /// Unknown dates and dates equal to the cutoff are excluded.
    pub fn eligible_before(&self, cutoff: &str) -> Result<Vec<bool>, String> {
        let cutoff = parse_date(cutoff)?;
        Ok(self
            .dates
            .iter()
            .map(|date| date.is_some_and(|date| date < cutoff))
            .collect())
    }
}

fn parse_date(value: &str) -> Result<u32, String> {
    let invalid = || {
        format!("invalid date {value:?}; expected a Gregorian YYYY-MM-DD date (years 0001–9999)")
    };
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, byte)| i == 4 || i == 7 || byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    let number = |slice: &[u8]| {
        slice
            .iter()
            .fold(0u32, |n, byte| n * 10 + u32::from(byte - b'0'))
    };
    let year = number(&bytes[..4]);
    let month = number(&bytes[5..7]);
    let day = number(&bytes[8..]);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(invalid()),
    };
    if year == 0 || day == 0 || day > days {
        return Err(invalid());
    }
    Ok(year * 10000 + month * 100 + day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::lookup::load_lookup_from_file;

    fn lookup() -> LookupTable {
        lookup_from_text("0\t/a.pdb\t10\t90\n1\t/b.pdb\t10\t90\n2\t/c.pdb\t10\t90\n")
    }

    fn lookup_from_text(content: &str) -> LookupTable {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "folddisco_release_dates_{}_{unique}.lookup",
            std::process::id()
        ));
        std::fs::write(&path, content).unwrap();
        let table = load_lookup_from_file(path.to_str().unwrap());
        let _ = std::fs::remove_file(format!("{}.cache", path.display()));
        let _ = std::fs::remove_file(path);
        table
    }

    #[test]
    fn rejects_nonidentity_lookup_ids() {
        for content in [
            "2\t/a.pdb\t10\t90\n0\t/b.pdb\t10\t90\n1\t/c.pdb\t10\t90\n",
            "0\t/a.pdb\t10\t90\n0\t/b.pdb\t10\t90\n2\t/c.pdb\t10\t90\n",
        ] {
            let table = lookup_from_text(content);
            let error = ReleaseDates::read(
                "target\trelease_date\n/a.pdb\t2000-01-01\n/b.pdb\t?\n/c.pdb\t2020-01-01\n"
                    .as_bytes(),
                &table,
            )
            .err()
            .expect("nonidentity lookup IDs must be rejected");
            assert!(
                error.contains("lookup IDs to equal row ordinals"),
                "{error}"
            );
        }
    }

    #[test]
    fn strict_calendar_dates() {
        for valid in ["0001-01-01", "2000-02-29", "2024-02-29", "9999-12-31"] {
            assert!(parse_date(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "?",
            "0000-01-01",
            "1900-02-29",
            "2023-02-29",
            "2024-04-31",
            "2024-00-01",
            "2024-13-01",
            "2024-01-00",
            "2024-1-01",
            "2024-01-1",
            " 2024-01-01",
            "2024-01-01 ",
            "２０２４-01-01",
            "2024/01/01",
            "2024-01-01T00:00:00",
        ] {
            assert!(parse_date(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn matches_exact_names_and_lookup_ordinals_with_exclusive_cutoff() {
        let table = lookup();
        let dates = ReleaseDates::read(
            "target\trelease_date\n/c.pdb\t?\n/b.pdb\t2024-01-01\n/a.pdb\t2023-12-31\n".as_bytes(),
            &table,
        )
        .unwrap();
        assert_eq!(
            dates.eligible_before("2024-01-01").unwrap(),
            vec![true, false, false]
        );
        assert_eq!(
            dates.eligible_before("2024-01-02").unwrap(),
            vec![true, true, false]
        );
        assert!(dates.eligible_before("2024-02-30").is_err());
    }

    #[test]
    fn rejects_incomplete_ambiguous_or_malformed_metadata() {
        let table = lookup();
        let valid = "target\trelease_date\n/a.pdb\t2000-01-01\n/b.pdb\t?\n/c.pdb\t2020-01-01\n";
        for bad in [
            String::new(),
            valid.replacen("target\trelease_date\n", "", 1),
            valid.replace("/b.pdb\t?\n", ""),
            format!("{valid}/a.pdb\t2000-01-01\n"),
            format!("{valid}/extra.pdb\t?\n"),
            valid.replace("/a.pdb", "a"),
            valid.replace("/a.pdb", "2"),
            valid.replace("2000-01-01", "2000-02-30"),
            valid.replace("/b.pdb\t?", "/b.pdb\t"),
            valid.replace("/b.pdb\t?", "/b.pdb\t?\textra"),
            format!("{valid}\n"),
        ] {
            assert!(
                ReleaseDates::read(bad.as_bytes(), &table).is_err(),
                "{bad:?}"
            );
        }
        assert!(ReleaseDates::read(valid.replace('\n', "\r\n").as_bytes(), &table).is_ok());
    }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("folddisco_dates_{}_{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_folddisco"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cutoff_applies_before_top_one_and_supports_native_batch() {
    let tmp = TestDir::new();
    let structures = tmp.0.join("structures");
    fs::create_dir(&structures).unwrap();
    let query = Path::new(env!("CARGO_MANIFEST_DIR")).join("query/4CHA.pdb");
    for name in ["first.pdb", "second.pdb"] {
        fs::copy(&query, structures.join(name)).unwrap();
    }
    let index = tmp.0.join("index");
    run(&[
        "index",
        "-y",
        "PDBMotif",
        "-p",
        structures.to_str().unwrap(),
        "-i",
        index.to_str().unwrap(),
        "-t",
        "1",
    ]);
    let results = tmp.0.join("results.tsv");
    let mut args = vec![
        "query",
        "-i",
        index.to_str().unwrap(),
        "-p",
        query.to_str().unwrap(),
        "-q",
        "B57,B102,C195",
        "--skip-match",
        "--top",
        "1",
        "--format-output",
        "tid",
        "-o",
        results.to_str().unwrap(),
    ];
    run(&args);
    let winner = fs::read_to_string(&results).unwrap().trim().to_string();
    assert!(!winner.is_empty());
    let lookup = fs::read_to_string(index.with_extension("lookup")).unwrap();
    let names: Vec<_> = lookup
        .lines()
        .map(|line| line.split('\t').nth(1).unwrap())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&winner.as_str()));
    let eligible = names.iter().find(|&&name| name != winner).unwrap();
    let dates = tmp.0.join("dates.tsv");
    let mut metadata = String::from("target\trelease_date\n");
    for name in &names {
        let date = if *name == winner {
            "2024-01-01"
        } else {
            "2020-01-01"
        };
        metadata.push_str(&format!("{name}\t{date}\n"));
    }
    fs::write(&dates, metadata).unwrap();
    args.extend([
        "--release-dates",
        dates.to_str().unwrap(),
        "--released-before",
        "2024-01-01",
    ]);
    run(&args);
    assert_eq!(fs::read_to_string(&results).unwrap().trim(), *eligible);

    let batch = tmp.0.join("queries.tsv");
    let out_one = tmp.0.join("one.tsv");
    let out_two = tmp.0.join("two.tsv");
    fs::write(
        &batch,
        format!(
            "{}\tB57,B102,C195\t{}\n{}\tB57,B102,C195\t{}\n",
            query.display(),
            out_one.display(),
            query.display(),
            out_two.display()
        ),
    )
    .unwrap();
    run(&[
        "query",
        "-i",
        index.to_str().unwrap(),
        "-q",
        batch.to_str().unwrap(),
        "--skip-match",
        "--top",
        "1",
        "--format-output",
        "tid",
        "-t",
        "2",
        "--release-dates",
        dates.to_str().unwrap(),
        "--released-before",
        "2024-01-01",
    ]);
    for output in [out_one, out_two] {
        assert_eq!(fs::read_to_string(output).unwrap().trim(), *eligible);
    }

    *args.last_mut().unwrap() = "2020-01-01";
    run(&args);
    assert!(fs::read_to_string(&results).unwrap().trim().is_empty());
    *args.last_mut().unwrap() = "2024-02-30";
    let invalid = Command::new(env!("CARGO_BIN_EXE_folddisco"))
        .args(&args)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid date"));
}

#[test]
fn cutoff_requires_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_folddisco"))
        .args(["query", "-i", "absent", "--released-before", "2024-01-01"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--release-dates"));
}

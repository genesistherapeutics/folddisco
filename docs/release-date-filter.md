# Release-date filtering

Use `--released-before YYYY-MM-DD` together with `--release-dates /path/to/release-dates.tsv` when querying an existing index. Both flags are required together. Only targets with a known release date strictly earlier than the cutoff can contribute candidate matches. Targets released on the cutoff and targets with unknown dates are excluded. This is a CPU-only query filter; it needs no GPU and does not rebuild or modify the index. IDF weighting still uses the full database, so scores are not recalibrated to the filtered subset.

The sidecar is UTF-8 TSV with the literal header `target<TAB>release_date`. For example (fields below are separated by tabs):

```tsv
target	release_date
/data/structures/1abc.pdb	1999-04-07
/data/structures/2def.pdb	?
```

Each target is the exact name stored in the index lookup table, including any path and extension. It is not a PDB accession, numeric structure ID, or Foldcomp database key. Row order is arbitrary: the loader maps names to lookup ordinals used by the postings. Every lookup target must appear exactly once; duplicate, missing, or extra names are errors, and duplicate names in the lookup are ambiguous and rejected. There are exactly two fields per row, with no comments or blank rows. LF and CRLF line endings are supported. Unknown release dates must be explicit `?`, not omitted or left blank.

Dates must be real Gregorian calendar dates in `YYYY-MM-DD` format, with years 0001–9999. Leap-year rules are enforced. Whitespace, timestamps, and shortened dates are rejected, including in the cutoff.

Build metadata from an authoritative original public release date source. Preserve its source snapshot and provenance. Do not substitute download dates, filesystem modification times, CIF conversion timestamps, or revision dates for the original release date. When a date cannot be established, use `?` rather than inventing one.

Publish the sidecar as an immutable version alongside the catalog/index version, with checksums for both the sidecar and its exact lookup file in the publication manifest. Record the date-source snapshot/version, date-field definition, and generation procedure there. The CLI validates names and dates; it does not verify external checksums or the truth of the supplied release dates. Reuse across machines is safe only when lookup names agree exactly. If lookup paths are rewritten, rewrite and revalidate sidecar names as part of the same catalog publication and publish new checksums; the old sidecar is not automatically portable to those names.

Filtered queries also require each lookup row's numeric ID to equal its zero-based row ordinal. Nonidentity IDs, including shuffled or duplicate IDs, are rejected before searching because candidate accumulation and result retrieval must address the same target. This restriction does not constrain TSV row order.

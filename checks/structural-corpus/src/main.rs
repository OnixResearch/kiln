//! Out-of-tree line-agreement harness for the structural review diff.
//!
//! Reads OmniDiff's pinned fixture corpus, projects its `text_mappings` and
//! `kiln_highlight::structural_diff` output to per-line touched labels, and
//! writes one CSV row per parsed fixture. This is a measurement harness, not a
//! correctness proof or a release gate by itself.

use kiln_highlight::{
    Language, StructuralBounds, StructuralFallback, StructuralOutcome, structural_diff,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn language(path: &Path) -> Option<Language> {
    let name = path.file_name()?.to_str()?;
    let inner = name.strip_suffix(".test")?;
    let extension = Path::new(inner).extension()?.to_str()?.to_ascii_lowercase();
    let probe = format!("file.{extension}");
    kiln_highlight::language_for_path(&probe)
}

fn touched_ranges(mapping: &serde_json::Value, side: &str) -> BTreeSet<u32> {
    let mut touched = BTreeSet::new();
    let Some(mappings) = mapping
        .get("text_mappings")
        .and_then(|value| value.as_array())
    else {
        return touched;
    };
    for projection in mappings {
        if projection.get("name").and_then(|value| value.as_str()) != Some("Full") {
            continue;
        }
        let Some(entries) = projection.get("entries").and_then(|value| value.as_array()) else {
            continue;
        };
        for entry in entries {
            let Some(ranges) = entry.get(side).and_then(|value| value.as_array()) else {
                continue;
            };
            for range in ranges {
                let (Some(start), Some(end)) = (
                    range.get("start_row").and_then(|value| value.as_u64()),
                    range.get("end_row").and_then(|value| value.as_u64()),
                ) else {
                    continue;
                };
                for row in start..=end {
                    touched.insert(u32::try_from(row).unwrap_or(u32::MAX) + 1);
                }
            }
        }
    }
    touched
}

fn structural_touched(
    outcome: &StructuralOutcome,
) -> (BTreeSet<u32>, BTreeSet<u32>, Option<&'static str>) {
    let mut old = BTreeSet::new();
    let mut new = BTreeSet::new();
    match outcome {
        StructuralOutcome::Fallback { reason } => (old, new, Some(reason_name(*reason))),
        StructuralOutcome::Structural(file) => {
            for hunk in &file.hunks {
                for row in &hunk.rows {
                    if row.novel.is_empty() {
                        continue;
                    }
                    if let Some(line) = row.old_line {
                        old.insert(line);
                    }
                    if let Some(line) = row.new_line {
                        new.insert(line);
                    }
                }
            }
            (old, new, None)
        }
    }
}

fn reason_name(reason: StructuralFallback) -> &'static str {
    match reason {
        StructuralFallback::UnsupportedLanguage => "unsupported_language",
        StructuralFallback::Binary => "binary",
        StructuralFallback::TooLarge => "too_large",
        StructuralFallback::ParseError => "parse_error",
        StructuralFallback::TooManyNodes => "too_many_nodes",
        StructuralFallback::WorkBound => "work_bound",
        StructuralFallback::Interrupted => "interrupted",
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let index = (sorted.len() - 1) * percentile / 100;
    sorted[index]
}

fn mismatch(left: &BTreeSet<u32>, right: &BTreeSet<u32>) -> usize {
    left.symmetric_difference(right).count()
}

fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: structural-corpus <diffs-root> [baseline.csv]")?;
    let baseline = std::env::args().nth(2);
    let root = PathBuf::from(root);
    let mut rows = Vec::new();
    for dataset in fs::read_dir(&root).map_err(|error| error.to_string())? {
        let dataset = dataset.map_err(|error| error.to_string())?;
        if !dataset
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        for fixture in fs::read_dir(dataset.path()).map_err(|error| error.to_string())? {
            let fixture = fixture.map_err(|error| error.to_string())?;
            if !fixture
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let path = fixture.path();
            let mapping_path = path.join("human_mapping.json");
            let Ok(mapping_bytes) = fs::read(&mapping_path) else {
                continue;
            };
            let mapping: serde_json::Value = serde_json::from_slice(&mapping_bytes)
                .map_err(|error| format!("{}: {error}", mapping_path.display()))?;
            let before = fs::read_dir(&path)
                .map_err(|error| error.to_string())?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|entry| {
                    entry
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("before.") && name.ends_with(".test"))
                });
            let after = fs::read_dir(&path)
                .map_err(|error| error.to_string())?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|entry| {
                    entry
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("after.") && name.ends_with(".test"))
                });
            let Some((before, after)) = before.zip(after) else {
                continue;
            };
            let Some(language) = language(&before) else {
                continue;
            };
            let old = fs::read(&before).map_err(|error| error.to_string())?;
            let new = fs::read(&after).map_err(|error| error.to_string())?;
            let started = Instant::now();
            let outcome = structural_diff(
                Some(language),
                Some(&old),
                Some(&new),
                &StructuralBounds::DEFAULT,
                &|| false,
            );
            let elapsed_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
            let (old_touched, new_touched, fallback) = structural_touched(&outcome);
            let expected_old = touched_ranges(&mapping, "before");
            let expected_new = touched_ranges(&mapping, "after");
            rows.push((
                dataset.file_name().to_string_lossy().into_owned(),
                fixture.file_name().to_string_lossy().into_owned(),
                language.name(),
                fallback,
                mismatch(&old_touched, &expected_old),
                mismatch(&new_touched, &expected_new),
                expected_old.len(),
                expected_new.len(),
                elapsed_us,
            ));
        }
    }
    rows.sort();
    println!("dataset,fixture,language,fallback,old_mismatch,new_mismatch,old_touched,new_touched");
    for row in &rows {
        println!(
            "{},{},{},{},{},{},{},{}",
            row.0,
            row.1,
            row.2,
            row.3.unwrap_or(""),
            row.4,
            row.5,
            row.6,
            row.7
        );
    }
    let parsed = rows.iter().filter(|row| row.3.is_none()).count();
    let clean = rows
        .iter()
        .filter(|row| row.3.is_none() && row.4 == 0 && row.5 == 0)
        .count();
    eprintln!(
        "{} parsed fixtures, {} with zero mismatched lines, {} fallback",
        parsed,
        clean,
        rows.len() - parsed
    );
    #[derive(Default)]
    struct Stats {
        fixtures: usize,
        parsed: usize,
        fallback: std::collections::BTreeMap<&'static str, usize>,
        zero: usize,
        at_most_one_percent: usize,
        times: Vec<u64>,
    }
    let mut by_language: std::collections::BTreeMap<&str, Stats> =
        std::collections::BTreeMap::new();
    for row in &rows {
        let stats = by_language.entry(row.2).or_default();
        stats.fixtures += 1;
        if let Some(reason) = row.3 {
            *stats.fallback.entry(reason).or_default() += 1;
            continue;
        }
        stats.parsed += 1;
        stats.times.push(row.8);
        let mismatched = row.4 + row.5;
        let touched = row.6 + row.7;
        if mismatched == 0 {
            stats.zero += 1;
        }
        let rate = if touched == 0 {
            if mismatched == 0 { 0.0 } else { 1.0 }
        } else {
            mismatched as f64 / touched as f64
        };
        if rate <= 0.01 {
            stats.at_most_one_percent += 1;
        }
    }
    eprintln!("language,fixtures,parsed,fallback,zero_mismatch,at_most_1pct,p50_us,p90_us,p99_us");
    for (language, mut stats) in by_language {
        stats.times.sort_unstable();
        let fallback = stats.fallback.values().sum::<usize>();
        eprintln!(
            "{language},{},{},{},{},{},{},{},{}",
            stats.fixtures,
            stats.parsed,
            fallback,
            stats.zero,
            stats.at_most_one_percent,
            percentile(&stats.times, 50),
            percentile(&stats.times, 90),
            percentile(&stats.times, 99)
        );
    }
    if let Some(path) = baseline {
        let text = fs::read_to_string(&path).map_err(|error| format!("{path}: {error}"))?;
        let mut expected = std::collections::BTreeMap::new();
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 8 {
                continue;
            }
            let key = format!("{}/{}", fields[0], fields[1]);
            expected.insert(
                key,
                (
                    fields[3].to_owned(),
                    fields[4].parse::<usize>().unwrap_or(usize::MAX),
                    fields[5].parse::<usize>().unwrap_or(usize::MAX),
                ),
            );
        }
        let mut regressions = Vec::new();
        for row in &rows {
            let key = format!("{}/{}", row.0, row.1);
            let Some((old_fallback, old_old, old_new)) = expected.get(&key) else {
                continue;
            };
            let current_fallback = row.3.unwrap_or("");
            if old_fallback.is_empty() && !current_fallback.is_empty() {
                regressions.push(format!("{key}: fallback {current_fallback}"));
            }
            if row.4 > *old_old || row.5 > *old_new {
                regressions.push(format!(
                    "{key}: mismatches {}->{} / {}->{}",
                    old_old, row.4, old_new, row.5
                ));
            }
        }
        if !regressions.is_empty() {
            for regression in &regressions {
                eprintln!("regression: {regression}");
            }
            return Err(format!("{} structural regressions", regressions.len()));
        }
        eprintln!("baseline {}: no regressions", path);
    }
    Ok(())
}

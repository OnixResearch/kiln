//! Bounded report normalization. Recorded documents are evidence, never gate authority.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_REPORT_BYTES: usize = 2_097_152;
pub const MAX_TOTAL_REPORT_BYTES: usize = 16_777_216;
pub const MAX_REPORTS: usize = 128;
pub const MAX_ITEMS: usize = 20_000;
const MAX_NODES: u32 = 100_000;
const MAX_DEPTH: usize = 64;
const MAX_FIELD_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Junit,
    Cobertura,
    Lcov,
    Gate,
    Review,
    Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportDeclaration {
    pub kind: ReportKind,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportBinding {
    pub run_id: String,
    pub source_revision: String,
    pub attempt_id: String,
    pub execution: Value,
    pub node_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportUpload {
    #[serde(flatten)]
    pub binding: ReportBinding,
    pub kind: ReportKind,
    pub name: String,
    pub body_base64: Option<String>,
    pub blake3: Option<String>,
    pub unavailable: Option<String>,
}

impl<'de> Deserialize<'de> for ReportUpload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Decode a flat wire type explicitly: serde flatten and deny_unknown_fields
        // are not a supported combination, and ingress must reject extra identities.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            run_id: String,
            source_revision: String,
            attempt_id: String,
            execution: Value,
            node_id: String,
            kind: ReportKind,
            name: String,
            body_base64: Option<String>,
            blake3: Option<String>,
            unavailable: Option<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            binding: ReportBinding {
                run_id: wire.run_id,
                source_revision: wire.source_revision,
                attempt_id: wire.attempt_id,
                execution: wire.execution,
                node_id: wire.node_id,
            },
            kind: wire.kind,
            name: wire.name,
            body_base64: wire.body_base64,
            blake3: wire.blake3,
            unavailable: wire.unavailable,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub kind: ReportKind,
    pub source_blake3: String,
    pub source_bytes: usize,
    pub tests: Option<TestReport>,
    pub coverage: Option<Coverage>,
    pub evidence: Option<Evidence>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestReport {
    pub cases: Vec<Case>,
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub errors: u64,
    pub skipped: u64,
    pub duration_seconds: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Passed,
    Failed,
    Error,
    Skipped,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub suite: String,
    pub class_name: String,
    pub name: String,
    pub status: CaseStatus,
    pub duration_seconds: Option<f64>,
    pub message: Option<String>,
    pub details: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    pub format: String,
    pub lines_found: u64,
    pub lines_hit: u64,
    pub branches_found: Option<u64>,
    pub branches_hit: Option<u64>,
    pub line_rate: Option<f64>,
    pub branch_rate: Option<f64>,
    pub files: Vec<CoverageFile>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverageFile {
    pub path: String,
    pub lines: Vec<CoverageLine>,
    pub branches_found: Option<u64>,
    pub branches_hit: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverageLine {
    pub number: u64,
    pub hits: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub document: Value,
    pub fields: Vec<EvidenceField>,
    pub links: Vec<EvidenceField>,
    pub accepted_gate: bool,
    pub claim_boundary: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceField {
    pub path: String,
    pub value: Value,
}

/// Safe declaration path; file shells additionally traverse directory descriptors without symlinks.
pub fn safe_relative_path(path: &Path) -> bool {
    path.to_str().is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 4096
            && !s.chars().any(|c| c.is_control() || c == '\\')
            && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
    }) && path.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Exact run/source/attempt/execution/node admission happens before parsing or retention.
pub fn validate_upload(
    upload: &ReportUpload,
    expected: &ReportBinding,
) -> Result<Option<(Report, Vec<u8>)>, String> {
    if &upload.binding != expected || !safe_relative_path(Path::new(&upload.name)) {
        return Err("report binding or name mismatch".into());
    }
    match (&upload.body_base64, &upload.blake3, &upload.unavailable) {
        (None, None, Some(reason)) if reason == "missing" || reason == "unreadable" => Ok(None),
        (Some(body), Some(hash), None) => {
            if body.len() > MAX_REPORT_BYTES.div_ceil(3) * 4 {
                return Err("report encoding exceeds bound".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|_| "report base64 invalid")?;
            if blake3::hash(&bytes).to_hex().as_str() != hash {
                return Err("report source identity mismatch".into());
            }
            let report = parse(upload.kind, &bytes, expected)?;
            Ok(Some((report, bytes)))
        }
        _ => Err("report requires exact bytes and identity or explicit unavailability".into()),
    }
}

pub fn parse(kind: ReportKind, bytes: &[u8], binding: &ReportBinding) -> Result<Report, String> {
    if bytes.is_empty() || bytes.len() > MAX_REPORT_BYTES {
        return Err("report byte bound exceeded".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "report is not UTF-8")?;
    let mut report = Report {
        kind,
        source_blake3: blake3::hash(bytes).to_hex().to_string(),
        source_bytes: bytes.len(),
        tests: None,
        coverage: None,
        evidence: None,
    };
    match kind {
        ReportKind::Junit => report.tests = Some(junit(&xml(text)?)?),
        ReportKind::Cobertura => report.coverage = Some(cobertura(&xml(text)?)?),
        ReportKind::Lcov => report.coverage = Some(lcov(text)?),
        ReportKind::Gate | ReportKind::Review | ReportKind::Provenance => {
            report.evidence = Some(evidence(text, binding)?)
        }
    }
    Ok(report)
}

fn xml(text: &str) -> Result<Document<'_>, String> {
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("report XML declarations forbidden".into());
    }
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
        },
    )
    .map_err(|_| "report XML malformed or exceeds node bound")?;
    for node in doc.descendants() {
        if node.ancestors().take(MAX_DEPTH + 2).count() > MAX_DEPTH + 1
            || node.text().is_some_and(|s| s.len() > MAX_FIELD_BYTES)
            || node.attributes().any(|a| a.value().len() > MAX_FIELD_BYTES)
            || (node.is_element() && node.tag_name().namespace().is_some())
        {
            return Err("report XML depth, field or namespace invalid".into());
        }
    }
    Ok(doc)
}
fn string(value: &str) -> Result<String, String> {
    if value.len() > MAX_FIELD_BYTES || value.contains('\0') {
        return Err("report field exceeds bound".into());
    }
    Ok(value.to_owned())
}
fn uint(value: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("report count invalid".into());
    }
    value.parse().map_err(|_| "report count overflow".into())
}
fn metric(value: &str) -> Result<f64, String> {
    let n: f64 = value.parse().map_err(|_| "report metric invalid")?;
    if !n.is_finite() || n < 0.0 {
        return Err("report metric nonfinite or negative".into());
    }
    Ok(n)
}
fn optional_count(node: Node<'_, '_>, key: &str) -> Result<Option<u64>, String> {
    node.attribute(key).map(uint).transpose()
}
fn optional_time(node: Node<'_, '_>) -> Result<Option<f64>, String> {
    node.attribute("time").map(metric).transpose()
}
fn check_count(node: Node<'_, '_>, key: &str, actual: u64) -> Result<(), String> {
    if optional_count(node, key)?.is_some_and(|n| n != actual) {
        return Err("report declared counts disagree with records".into());
    }
    Ok(())
}
fn junit(doc: &Document<'_>) -> Result<TestReport, String> {
    let root = doc.root_element();
    if !matches!(root.tag_name().name(), "testsuite" | "testsuites") {
        return Err("JUnit root invalid".into());
    }
    for node in root.descendants().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        let parent = node
            .parent()
            .filter(|n| n.is_element())
            .map(|n| n.tag_name().name());
        let valid = match tag {
            "testsuites" => node == root || parent == Some("testsuites"),
            "testsuite" => node == root || matches!(parent, Some("testsuite" | "testsuites")),
            "testcase" => parent == Some("testsuite"),
            "failure" | "error" | "skipped" => parent == Some("testcase"),
            "system-out" | "system-err" | "properties" => {
                matches!(parent, Some("testsuite" | "testsuites" | "testcase"))
            }
            "property" => parent == Some("properties"),
            _ => false,
        };
        if !valid {
            return Err("JUnit unsupported or misplaced element".into());
        }
    }
    let mut cases = Vec::new();
    let mut keys = BTreeSet::new();
    for node in root.descendants().filter(|n| n.has_tag_name("testcase")) {
        if cases.len() >= MAX_ITEMS || !node.parent().is_some_and(|p| p.has_tag_name("testsuite")) {
            return Err("JUnit case placement or count invalid".into());
        }
        let name = string(node.attribute("name").ok_or("JUnit case name missing")?)?;
        if name.is_empty() {
            return Err("JUnit case name empty".into());
        }
        let class_name = string(node.attribute("classname").unwrap_or(""))?;
        let mut ancestry = node
            .ancestors()
            .filter(|n| n.has_tag_name("testsuite"))
            .map(|n| n.attribute("name").unwrap_or(""))
            .collect::<Vec<_>>();
        ancestry.reverse();
        let suite = ancestry.join("/");
        if !keys.insert((suite.clone(), class_name.clone(), name.clone())) {
            return Err("JUnit duplicate ambiguous case".into());
        }
        let mut outcomes = node.children().filter(|n| {
            n.is_element() && matches!(n.tag_name().name(), "failure" | "error" | "skipped")
        });
        let outcome = outcomes.next();
        if outcomes.next().is_some() {
            return Err("JUnit case has conflicting outcomes".into());
        }
        let status = match outcome.map(|n| n.tag_name().name()) {
            Some("failure") => CaseStatus::Failed,
            Some("error") => CaseStatus::Error,
            Some("skipped") => CaseStatus::Skipped,
            _ => CaseStatus::Passed,
        };
        if let Some(recorded) = node.attribute("status") {
            let consistent = match recorded {
                "run" => true,
                "passed" => status == CaseStatus::Passed,
                "failed" => matches!(status, CaseStatus::Failed | CaseStatus::Error),
                "skipped" | "notrun" | "disabled" => status == CaseStatus::Skipped,
                _ => false,
            };
            if !consistent {
                return Err("JUnit status disagrees with outcome or is unsupported".into());
            }
        }
        let details = outcome
            .map(|n| {
                n.descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>()
            })
            .map(|s| string(&s))
            .transpose()?;
        cases.push(Case {
            suite,
            class_name,
            name,
            status,
            duration_seconds: optional_time(node)?,
            message: outcome
                .and_then(|n| n.attribute("message"))
                .map(string)
                .transpose()?,
            details,
            file: node.attribute("file").map(string).transpose()?,
            line: optional_count(node, "line")?,
        });
    }
    for suite in root
        .descendants()
        .filter(|n| n.is_element() && matches!(n.tag_name().name(), "testsuite" | "testsuites"))
    {
        if suite != root
            && !suite
                .parent()
                .is_some_and(|p| matches!(p.tag_name().name(), "testsuite" | "testsuites"))
        {
            return Err("JUnit suite placement invalid".into());
        }
        optional_time(suite)?;
        let mut counts = [0u64; 4];
        for child in suite.descendants().filter(|n| n.has_tag_name("testcase")) {
            counts[0] += 1;
            for (count, tag) in counts
                .iter_mut()
                .skip(1)
                .zip(["failure", "error", "skipped"])
            {
                if child.children().any(|n| n.has_tag_name(tag)) {
                    *count += 1;
                }
            }
        }
        for (attr, actual) in ["tests", "failures", "errors", "skipped"]
            .iter()
            .zip(counts)
        {
            check_count(suite, attr, actual)?;
        }
    }
    let mut result = TestReport {
        total: cases.len() as u64,
        passed: 0,
        failed: 0,
        errors: 0,
        skipped: 0,
        duration_seconds: Some(0.0),
        cases,
    };
    for case in &result.cases {
        match case.status {
            CaseStatus::Passed => result.passed += 1,
            CaseStatus::Failed => result.failed += 1,
            CaseStatus::Error => result.errors += 1,
            CaseStatus::Skipped => result.skipped += 1,
        }
        result.duration_seconds = result
            .duration_seconds
            .zip(case.duration_seconds)
            .map(|(a, b)| a + b);
        if result.duration_seconds.is_some_and(|n| !n.is_finite()) {
            return Err("JUnit total time overflow".into());
        }
    }
    Ok(result)
}

fn ratio(hit: u64, found: u64) -> Option<f64> {
    (found != 0).then(|| hit as f64 / found as f64)
}
fn source_path(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.split(['/', '\\']).any(|p| p == "..")
    {
        return Err("coverage source path invalid".into());
    }
    Ok(value.to_owned())
}
fn coverage(format: &str, files: Vec<CoverageFile>) -> Result<Coverage, String> {
    let lines_found = files.iter().map(|f| f.lines.len() as u64).sum();
    let lines_hit = files
        .iter()
        .flat_map(|f| &f.lines)
        .filter(|l| l.hits > 0)
        .count() as u64;
    let branches_found = files
        .iter()
        .try_fold(0u64, |a, f| a.checked_add(f.branches_found?));
    let branches_hit = files
        .iter()
        .try_fold(0u64, |a, f| a.checked_add(f.branches_hit?));
    if files.len() > MAX_ITEMS || lines_found > MAX_ITEMS as u64 {
        return Err("coverage record count exceeds bound".into());
    }
    Ok(Coverage {
        format: format.into(),
        lines_found,
        lines_hit,
        branches_found,
        branches_hit,
        line_rate: ratio(lines_hit, lines_found),
        branch_rate: branches_hit
            .zip(branches_found)
            .and_then(|(h, f)| ratio(h, f)),
        files,
    })
}
fn check_rate(node: Node<'_, '_>, attr: &str, actual: Option<f64>) -> Result<(), String> {
    if let Some(value) = node.attribute(attr) {
        let n = metric(value)?;
        if n > 1.0 || actual.is_some_and(|a| (a - n).abs() > 0.005_001) {
            return Err("coverage rate invalid or disagrees with records".into());
        }
    }
    Ok(())
}
fn cobertura(doc: &Document<'_>) -> Result<Coverage, String> {
    let root = doc.root_element();
    if !root.has_tag_name("coverage") {
        return Err("Cobertura root invalid".into());
    }
    let mut files: BTreeMap<String, BTreeMap<u64, (u64, u64, u64)>> = BTreeMap::new();
    let mut count = 0usize;
    for class in root.descendants().filter(|n| n.has_tag_name("class")) {
        let path = source_path(
            class
                .attribute("filename")
                .ok_or("coverage filename missing")?,
        )?;
        let lines = files.entry(path).or_default();
        for line in class
            .children()
            .filter(|n| n.has_tag_name("lines"))
            .flat_map(|n| n.children())
            .filter(|n| n.has_tag_name("line"))
        {
            count += 1;
            if count > MAX_ITEMS {
                return Err("coverage line count exceeds bound".into());
            }
            let number = uint(line.attribute("number").ok_or("coverage line missing")?)?;
            let hits = uint(line.attribute("hits").ok_or("coverage hits missing")?)?;
            if number == 0 {
                return Err("coverage line zero".into());
            }
            let (found, hit) = match line.attribute("branch") {
                None | Some("false") => (0, 0),
                Some("true") => {
                    let condition = line
                        .attribute("condition-coverage")
                        .ok_or("coverage branch counts missing")?;
                    let (percent, counts) = condition
                        .split_once("% (")
                        .ok_or("coverage branch counts malformed")?;
                    let percent = metric(percent)?;
                    let (h, f) = counts
                        .strip_suffix(')')
                        .and_then(|s| s.split_once('/'))
                        .ok_or("coverage branch counts malformed")?;
                    let (h, f) = (uint(h)?, uint(f)?);
                    if f == 0
                        || h > f
                        || percent > 100.0
                        || (100.0 * h as f64 / f as f64 - percent).abs() > 0.5001
                    {
                        return Err("coverage branch metric invalid".into());
                    }
                    (f, h)
                }
                _ => return Err("coverage branch flag invalid".into()),
            };
            if lines.insert(number, (hits, found, hit)).is_some() {
                return Err("coverage duplicate ambiguous line".into());
            }
        }
    }
    if files.is_empty() {
        return Err("Cobertura report has no source records".into());
    }
    let branch_known = root.attribute("branches-valid").is_some()
        || root.attribute("branches-covered").is_some()
        || root.attribute("branch-rate").is_some()
        || root
            .descendants()
            .any(|n| n.has_tag_name("line") && n.attribute("branch").is_some());
    let files = files
        .into_iter()
        .map(|(path, records)| {
            let mut branches_found = 0u64;
            let mut branches_hit = 0u64;
            let mut lines = Vec::new();
            for (number, (hits, found, hit)) in records {
                branches_found = branches_found
                    .checked_add(found)
                    .ok_or("coverage branches overflow")?;
                branches_hit = branches_hit
                    .checked_add(hit)
                    .ok_or("coverage branches overflow")?;
                lines.push(CoverageLine { number, hits });
            }
            Ok(CoverageFile {
                path,
                lines,
                branches_found: branch_known.then_some(branches_found),
                branches_hit: branch_known.then_some(branches_hit),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let result = coverage("cobertura", files)?;
    check_count(root, "lines-valid", result.lines_found)?;
    check_count(root, "lines-covered", result.lines_hit)?;
    if let Some(found) = result.branches_found {
        check_count(root, "branches-valid", found)?;
    }
    if let Some(hit) = result.branches_hit {
        check_count(root, "branches-covered", hit)?;
    }
    check_rate(root, "line-rate", result.line_rate)?;
    check_rate(root, "branch-rate", result.branch_rate)?;
    for class in root
        .descendants()
        .filter(|n| n.has_tag_name("class") || n.has_tag_name("package"))
    {
        for attr in ["line-rate", "branch-rate"] {
            if class.attribute(attr).is_some() {
                let n = metric(class.attribute(attr).unwrap_or(""))?;
                if n > 1.0 {
                    return Err("coverage rate exceeds one".into());
                }
            }
        }
    }
    Ok(result)
}

fn lcov(text: &str) -> Result<Coverage, String> {
    #[derive(Default)]
    struct Record {
        path: String,
        lines: BTreeMap<u64, u64>,
        branches: BTreeMap<(u64, u64, u64), bool>,
        counts: BTreeMap<String, u64>,
        functions: BTreeSet<String>,
        function_hits: BTreeMap<String, u64>,
    }
    let mut current: Option<Record> = None;
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut items = 0usize;
    for line in text.lines() {
        if line.len() > MAX_FIELD_BYTES {
            return Err("LCOV field exceeds bound".into());
        }
        if line.is_empty() {
            continue;
        }
        if line == "end_of_record" {
            let r = current.take().ok_or("LCOV record end without source")?;
            let lf = r.lines.len() as u64;
            let lh = r.lines.values().filter(|n| **n > 0).count() as u64;
            let bf = r.branches.len() as u64;
            let bh = r.branches.values().filter(|n| **n).count() as u64;
            let ff = r.functions.len() as u64;
            let fh = r.function_hits.values().filter(|n| **n > 0).count() as u64;
            if r.function_hits
                .keys()
                .any(|name| !r.functions.contains(name))
            {
                return Err("LCOV function hit without definition".into());
            }
            for (key, n) in [
                ("LF", lf),
                ("LH", lh),
                ("BRF", bf),
                ("BRH", bh),
                ("FNF", ff),
                ("FNH", fh),
            ] {
                if r.counts.get(key).is_some_and(|v| *v != n) {
                    return Err("LCOV declared counts disagree".into());
                }
            }
            if !paths.insert(r.path.clone()) {
                return Err("LCOV duplicate source record".into());
            }
            let branch_known = !r.branches.is_empty()
                || r.counts.contains_key("BRF")
                || r.counts.contains_key("BRH");
            files.push(CoverageFile {
                path: r.path,
                lines: r
                    .lines
                    .into_iter()
                    .map(|(number, hits)| CoverageLine { number, hits })
                    .collect(),
                branches_found: branch_known.then_some(bf),
                branches_hit: branch_known.then_some(bh),
            });
            continue;
        }
        let (key, value) = line.split_once(':').ok_or("LCOV malformed record")?;
        if key == "TN" {
            if current.is_some() {
                return Err("LCOV test name inside record".into());
            }
            string(value)?;
            continue;
        }
        if key == "SF" {
            if current.is_some() {
                return Err("LCOV source record not closed".into());
            }
            current = Some(Record {
                path: source_path(value)?,
                ..Record::default()
            });
            continue;
        }
        let r = current.as_mut().ok_or("LCOV data without source")?;
        items += 1;
        if items > MAX_ITEMS {
            return Err("LCOV record count exceeds bound".into());
        }
        match key {
            "DA" => {
                let mut parts = value.split(',');
                let n = uint(parts.next().ok_or("LCOV line missing")?)?;
                let hits = uint(parts.next().ok_or("LCOV hits missing")?)?;
                if let Some(checksum) = parts.next() {
                    if checksum.is_empty() || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err("LCOV checksum invalid".into());
                    }
                }
                if n == 0 || parts.next().is_some() || r.lines.insert(n, hits).is_some() {
                    return Err("LCOV duplicate or invalid line".into());
                }
            }
            "BRDA" => {
                let mut parts = value.split(',');
                let n = uint(parts.next().ok_or("LCOV branch malformed")?)?;
                let block = uint(parts.next().ok_or("LCOV branch malformed")?)?;
                let branch = uint(parts.next().ok_or("LCOV branch malformed")?)?;
                let taken = parts.next().ok_or("LCOV branch malformed")?;
                let hit = if taken == "-" {
                    false
                } else {
                    uint(taken)? > 0
                };
                if n == 0
                    || parts.next().is_some()
                    || r.branches.insert((n, block, branch), hit).is_some()
                {
                    return Err("LCOV duplicate or malformed branch".into());
                }
            }
            "LF" | "LH" | "BRF" | "BRH" | "FNF" | "FNH" => {
                if r.counts.insert(key.into(), uint(value)?).is_some() {
                    return Err("LCOV duplicate total".into());
                }
            }
            "FN" => {
                let (number, name) = value.split_once(',').ok_or("LCOV function malformed")?;
                if uint(number)? == 0 || name.is_empty() || !r.functions.insert(name.into()) {
                    return Err("LCOV function definition invalid or duplicate".into());
                }
            }
            "FNDA" => {
                let (hits, name) = value.split_once(',').ok_or("LCOV function malformed")?;
                if name.is_empty() || r.function_hits.insert(name.into(), uint(hits)?).is_some() {
                    return Err("LCOV function hit invalid or duplicate".into());
                }
            }
            _ => return Err("LCOV unsupported or malformed field".into()),
        }
    }
    if current.is_some() || files.is_empty() {
        return Err("LCOV incomplete or empty report".into());
    }
    coverage("lcov", files)
}

fn evidence(text: &str, binding: &ReportBinding) -> Result<Evidence, String> {
    // serde_json's recursion limit remains enabled; duplicate keys are rejected below.
    let value = strict_json(text)?;
    let object = value
        .as_object()
        .ok_or("native evidence document must be object")?;
    let document = if let Some(recorded) = object.get("binding") {
        let recorded: ReportBinding =
            serde_json::from_value(recorded.clone()).map_err(|_| "evidence binding invalid")?;
        if &recorded != binding {
            return Err("evidence belongs to another run or attempt".into());
        }
        object.get("document").unwrap_or(&value)
    } else {
        &value
    };
    let native = document
        .as_object()
        .ok_or("native evidence document must be object")?;
    let expected = serde_json::to_value(binding).map_err(|_| "evidence binding encoding failed")?;
    for key in [
        "run_id",
        "source_revision",
        "attempt_id",
        "execution",
        "node_id",
    ] {
        if native
            .get(key)
            .is_some_and(|v| Some(v) != expected.get(key))
        {
            return Err("native evidence claims a different run or attempt".into());
        }
    }
    let mut fields = Vec::new();
    let mut links = Vec::new();
    let mut nodes = 0usize;
    inspect(document, "", 0, &mut nodes, &mut fields, &mut links)?;
    Ok(Evidence { document: document.clone(), fields, links, accepted_gate:false,
        claim_boundary:"Recorded document only; native verdicts and identities are preserved. This upload is not gate acceptance, review acceptance, source trust, release readiness, or authority to act.".into() })
}
fn inspect(
    value: &Value,
    path: &str,
    depth: usize,
    nodes: &mut usize,
    fields: &mut Vec<EvidenceField>,
    links: &mut Vec<EvidenceField>,
) -> Result<(), String> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_ITEMS {
        return Err("evidence nesting or field count exceeds bound".into());
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                string(key)?;
                let key = key.replace('~', "~0").replace('/', "~1");
                inspect(
                    value,
                    &format!("{path}/{key}"),
                    depth + 1,
                    nodes,
                    fields,
                    links,
                )?;
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                inspect(
                    value,
                    &format!("{path}/{index}"),
                    depth + 1,
                    nodes,
                    fields,
                    links,
                )?;
            }
        }
        _ => {
            if value.as_str().is_some_and(|s| s.len() > MAX_FIELD_BYTES) {
                return Err("evidence field exceeds bound".into());
            }
            let field = EvidenceField {
                path: path.into(),
                value: value.clone(),
            };
            let key = path.rsplit('/').next().unwrap_or("");
            if value.is_string()
                && (key.ends_with("_id")
                    || key.ends_with("_ref")
                    || key.ends_with("_url")
                    || key.ends_with("_uri")
                    || key.ends_with("blake3")
                    || matches!(
                        key,
                        "id" | "identity" | "url" | "uri" | "digest" | "reference"
                    ))
            {
                links.push(field.clone());
            }
            fields.push(field);
        }
    }
    Ok(())
}

/// JSON with duplicate object names rejected at every depth, including native documents.
fn strict_json(text: &str) -> Result<Value, String> {
    struct Strict(Value);
    impl<'de> Deserialize<'de> for Strict {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Strict;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("bounded JSON without duplicate names")
                }
                fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Strict, E> {
                    Ok(Strict(Value::Bool(v)))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Strict, E> {
                    Ok(Strict(Value::from(v)))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Strict, E> {
                    Ok(Strict(Value::from(v)))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Strict, E> {
                    serde_json::Number::from_f64(v)
                        .map(|n| Strict(Value::Number(n)))
                        .ok_or_else(|| E::custom("nonfinite JSON number"))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Strict, E> {
                    Ok(Strict(Value::String(v.into())))
                }
                fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Strict, E> {
                    Ok(Strict(Value::String(v)))
                }
                fn visit_none<E: serde::de::Error>(self) -> Result<Strict, E> {
                    Ok(Strict(Value::Null))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Strict, E> {
                    Ok(Strict(Value::Null))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<Strict, A::Error> {
                    let mut a = Vec::new();
                    while let Some(v) = seq.next_element::<Strict>()? {
                        if a.len() >= MAX_ITEMS {
                            return Err(serde::de::Error::custom("JSON array bound"));
                        }
                        a.push(v.0);
                    }
                    Ok(Strict(Value::Array(a)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Strict, A::Error> {
                    let mut m = serde_json::Map::new();
                    while let Some(k) = map.next_key::<String>()? {
                        if m.len() >= MAX_ITEMS || m.contains_key(&k) {
                            return Err(serde::de::Error::custom(
                                "JSON duplicate key or count bound",
                            ));
                        }
                        let v = map.next_value::<Strict>()?;
                        m.insert(k, v.0);
                    }
                    Ok(Strict(Value::Object(m)))
                }
            }
            d.deserialize_any(Visitor)
        }
    }
    serde_json::from_str::<Strict>(text)
        .map(|v| v.0)
        .map_err(|_| "evidence JSON malformed, duplicate or exceeds depth/count bounds".into())
}

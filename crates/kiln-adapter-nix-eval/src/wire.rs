use std::collections::{BTreeMap, BTreeSet};

use kiln_core::discovery::{CacheAdvisory, DiscoveredJob, DiscoveredJobError, DiscoveredOutput};
use kiln_core::types::{Alias, ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{PortFailure, PortResult};
use serde::Deserialize;
use serde_json::Value;

use crate::profile::NixEvalLimits;

const STOCK_STORE_DIRECTORY: &str = "/nix/store";
const ONIX_WASM_TERM: &str = "wasm";
const ADJACENT_PAIR_ITEMS: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamAdmission {
    pub jobs: Vec<DiscoveredJob>,
    pub errors: Vec<DiscoveredJobError>,
    pub record_count: usize,
    pub total_bytes: usize,
}

#[derive(Debug)]
pub struct IncrementalDecoder {
    limits: NixEvalLimits,
    pending: Vec<u8>,
    jobs: Vec<DiscoveredJob>,
    errors: Vec<DiscoveredJobError>,
    record_count: usize,
    total_bytes: usize,
}

impl IncrementalDecoder {
    #[must_use]
    pub fn new(limits: NixEvalLimits) -> Self {
        Self {
            limits,
            pending: Vec::new(),
            jobs: Vec::new(),
            errors: Vec::new(),
            record_count: 0,
            total_bytes: 0,
        }
    }

    // r[impl kiln.nix_eval_jobs.stream_admission]
    pub fn push(&mut self, chunk: &[u8]) -> PortResult<()> {
        self.total_bytes = self.total_bytes.checked_add(chunk.len()).ok_or_else(|| {
            domain(
                "nix_eval_stream_overflow",
                "nix_eval.stream",
                "stream byte count overflowed",
            )
        })?;
        if self.total_bytes > self.limits.max_total_stdout_bytes {
            return Err(domain(
                "nix_eval_stream_bound",
                "nix_eval.stream",
                "stream exceeds its total byte bound",
            ));
        }
        self.pending.extend_from_slice(chunk);
        if self.pending.len() > self.limits.max_line_bytes && !self.pending.contains(&b'\n') {
            return Err(domain(
                "nix_eval_line_bound",
                "nix_eval.stream.line",
                "stream line exceeds its byte bound",
            ));
        }
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=newline).collect();
            let content = line.get(..newline).ok_or_else(|| {
                domain(
                    "nix_eval_line_slice",
                    "nix_eval.stream.line",
                    "stream line could not be bounded",
                )
            })?;
            self.admit_line(content)?;
        }
        if self.pending.len() > self.limits.max_line_bytes {
            return Err(domain(
                "nix_eval_line_bound",
                "nix_eval.stream.line",
                "stream line exceeds its byte bound",
            ));
        }
        Ok(())
    }

    pub fn finish(self) -> PortResult<StreamAdmission> {
        if !self.pending.is_empty() {
            return Err(domain(
                "nix_eval_stream_truncated",
                "nix_eval.stream",
                "stream ended within a JSON record",
            ));
        }
        let mut jobs = self.jobs;
        let mut errors = self.errors;
        jobs.sort_by(|left, right| left.attribute.cmp(&right.attribute));
        errors.sort_by(|left, right| left.attribute.cmp(&right.attribute));
        Ok(StreamAdmission {
            jobs,
            errors,
            record_count: self.record_count,
            total_bytes: self.total_bytes,
        })
    }

    fn admit_line(&mut self, line: &[u8]) -> PortResult<()> {
        if line.is_empty() || line.len() > self.limits.max_line_bytes {
            return Err(domain(
                "nix_eval_line_bound",
                "nix_eval.stream.line",
                "stream line is empty or exceeds its byte bound",
            ));
        }
        self.record_count = self.record_count.checked_add(1).ok_or_else(|| {
            domain(
                "nix_eval_record_overflow",
                "nix_eval.stream.records",
                "stream record count overflowed",
            )
        })?;
        if self.record_count > self.limits.max_records {
            return Err(domain(
                "nix_eval_record_bound",
                "nix_eval.stream.records",
                "stream record count exceeds its named bound",
            ));
        }
        let value: Value = serde_json::from_slice(line).map_err(|_| {
            domain(
                "nix_eval_record_json",
                "nix_eval.stream.record",
                "stream record is not valid JSON",
            )
        })?;
        let object = value.as_object().ok_or_else(|| {
            domain(
                "nix_eval_record_shape",
                "nix_eval.stream.record",
                "stream record is not a JSON object",
            )
        })?;
        if object.contains_key("error") {
            if self.errors.len() >= self.limits.max_job_errors {
                return Err(domain(
                    "nix_eval_error_bound",
                    "nix_eval.stream.errors",
                    "job error count exceeds its named bound",
                ));
            }
            let record: WireErrorRecord = serde_json::from_value(value).map_err(|_| {
                domain(
                    "nix_eval_error_schema",
                    "nix_eval.stream.error",
                    "job error record differs from the admitted schema",
                )
            })?;
            self.errors.push(normalize_error(record, self.limits)?);
            return Ok(());
        }
        if object.contains_key("drvPath") {
            if self.jobs.len() >= self.limits.max_jobs {
                return Err(domain(
                    "nix_eval_job_bound",
                    "nix_eval.stream.jobs",
                    "job count exceeds its named bound",
                ));
            }
            let record: WireJobRecord = serde_json::from_value(value).map_err(|_| {
                domain(
                    "nix_eval_job_schema",
                    "nix_eval.stream.job",
                    "job record differs from the admitted schema",
                )
            })?;
            self.jobs.push(normalize_job(record, self.limits)?);
            return Ok(());
        }
        Err(domain(
            "nix_eval_record_kind",
            "nix_eval.stream.record",
            "stream record is neither a supported job nor a job error",
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireErrorRecord {
    attr: String,
    #[serde(rename = "attrPath")]
    attr_path: Vec<String>,
    error: String,
    #[serde(default)]
    fatal: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireJobRecord {
    attr: String,
    #[serde(rename = "attrPath")]
    attr_path: Vec<String>,
    name: String,
    #[serde(rename = "storeDir")]
    store_directory: String,
    system: String,
    #[serde(rename = "drvPath")]
    derivation_path: String,
    outputs: BTreeMap<String, Option<String>>,
    #[serde(default)]
    meta: Option<Value>,
    #[serde(rename = "inputDrvs", default)]
    input_derivations: BTreeMap<String, Vec<String>>,
    #[serde(rename = "requiredSystemFeatures", default)]
    required_system_features: Vec<String>,
    #[serde(default)]
    constituents: Vec<String>,
    #[serde(rename = "namedConstituents", default)]
    named_constituents: Vec<String>,
    #[serde(rename = "globConstituents", default)]
    glob_constituents: bool,
    #[serde(rename = "isCached", default)]
    is_cached: Option<bool>,
    #[serde(rename = "cacheStatus", default)]
    cache_status: Option<String>,
    #[serde(rename = "neededBuilds", default)]
    needed_builds: Vec<String>,
    #[serde(rename = "neededSubstitutes", default)]
    needed_substitutes: Vec<String>,
}

fn normalize_error(
    record: WireErrorRecord,
    limits: NixEvalLimits,
) -> PortResult<DiscoveredJobError> {
    if record.attr_path.is_empty()
        || record.error.is_empty()
        || record.error.len() > limits.max_diagnostic_bytes
    {
        return Err(domain(
            "nix_eval_error_bound",
            "nix_eval.stream.error",
            "job error fields are empty or exceed their named bounds",
        ));
    }
    let code = if record.error.to_ascii_lowercase().contains(ONIX_WASM_TERM) {
        "unsupported_onix_builtin"
    } else {
        "nix_job_evaluation_error"
    };
    Ok(DiscoveredJobError {
        attribute: Alias::new(record.attr).map_err(PortFailure::Domain)?,
        diagnostic: Diagnostic::new(code, "nix_eval.job", &record.error),
        fatal: record.fatal,
    })
}

fn normalize_job(record: WireJobRecord, limits: NixEvalLimits) -> PortResult<DiscoveredJob> {
    if record.store_directory != STOCK_STORE_DIRECTORY
        || record.attr_path.is_empty()
        || record.outputs.is_empty()
        || record.outputs.len() > limits.max_outputs_per_job
        || record.input_derivations.len() > limits.max_dependencies_per_job
        || record.required_system_features.len() > limits.max_dependencies_per_job
        || record.constituents.len() > limits.max_dependencies_per_job
        || record.named_constituents.len() > limits.max_dependencies_per_job
        || record.needed_builds.len() > limits.max_dependencies_per_job
        || record.needed_substitutes.len() > limits.max_dependencies_per_job
        || record.glob_constituents
    {
        return Err(domain(
            "nix_eval_job_bound",
            "nix_eval.stream.job",
            "job fields exceed bounds or use an unsupported aggregate form",
        ));
    }
    let attribute_path = record
        .attr_path
        .into_iter()
        .map(Alias::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PortFailure::Domain)?;
    let mut outputs = Vec::with_capacity(record.outputs.len());
    for (name, path) in record.outputs {
        let path = path.ok_or_else(|| {
            domain(
                "nix_eval_output_missing",
                "nix_eval.stream.job.outputs",
                "job output path is absent",
            )
        })?;
        validate_store_path(&path, "nix_eval.stream.job.outputs")?;
        outputs.push(DiscoveredOutput {
            name: Alias::new(name).map_err(PortFailure::Domain)?,
            path: ExternalRef::new("nix", "store-path", &path).map_err(PortFailure::Domain)?,
        });
    }
    outputs.sort();
    validate_store_path(&record.derivation_path, "nix_eval.stream.job.derivation")?;
    if !record.derivation_path.ends_with(".drv") {
        return Err(domain(
            "nix_eval_derivation_path",
            "nix_eval.stream.job.derivation",
            "job derivation path does not name a Nix derivation",
        ));
    }
    let mut input_derivations = Vec::with_capacity(record.input_derivations.len());
    for (path, output_names) in record.input_derivations {
        if output_names.is_empty() || output_names.len() > limits.max_outputs_per_job {
            return Err(domain(
                "nix_eval_input_output_bound",
                "nix_eval.stream.job.input_derivations",
                "input derivation output names are empty or exceed their bound",
            ));
        }
        let unique_outputs: BTreeSet<&String> = output_names.iter().collect();
        if unique_outputs.len() != output_names.len() {
            return Err(domain(
                "nix_eval_input_output_duplicate",
                "nix_eval.stream.job.input_derivations",
                "input derivation repeats an output name",
            ));
        }
        validate_store_path(&path, "nix_eval.stream.job.input_derivations")?;
        if !path.ends_with(".drv") {
            return Err(domain(
                "nix_eval_input_derivation_path",
                "nix_eval.stream.job.input_derivations",
                "input derivation does not name a Nix derivation path",
            ));
        }
        input_derivations
            .push(ExternalRef::new("nix", "derivation", &path).map_err(PortFailure::Domain)?);
    }
    input_derivations.sort();
    if input_derivations.windows(ADJACENT_PAIR_ITEMS).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left == right)
    }) {
        return Err(domain(
            "nix_eval_input_derivation_duplicate",
            "nix_eval.stream.job.input_derivations",
            "job repeats an input derivation",
        ));
    }
    let metadata_identity = record
        .meta
        .as_ref()
        .map(|metadata| metadata_identity(metadata, limits.max_metadata_bytes))
        .transpose()?;
    let cache = normalize_cache(record.cache_status.as_deref(), record.is_cached)?;
    Ok(DiscoveredJob {
        attribute: Alias::new(record.attr).map_err(PortFailure::Domain)?,
        attribute_path,
        name: Alias::new(record.name).map_err(PortFailure::Domain)?,
        system: Alias::new(record.system).map_err(PortFailure::Domain)?,
        derivation: ExternalRef::new("nix", "derivation", &record.derivation_path)
            .map_err(PortFailure::Domain)?,
        outputs,
        input_derivations,
        metadata_identity,
        cache,
    })
}

fn normalize_cache(status: Option<&str>, deprecated: Option<bool>) -> PortResult<CacheAdvisory> {
    match (status, deprecated) {
        (None, None) => Ok(CacheAdvisory::NotChecked),
        (Some("cached"), Some(true)) => Ok(CacheAdvisory::Cached),
        (Some("local"), Some(true)) => Ok(CacheAdvisory::Local),
        (Some("notBuilt"), Some(false)) => Ok(CacheAdvisory::NotBuilt),
        (Some(_), Some(_)) => Err(domain(
            "nix_eval_cache_conflict",
            "nix_eval.stream.job.cache",
            "current and deprecated cache fields conflict or use an unknown class",
        )),
        _ => Err(domain(
            "nix_eval_cache_pair",
            "nix_eval.stream.job.cache",
            "current and deprecated cache fields must appear together",
        )),
    }
}

fn metadata_identity(metadata: &Value, max_bytes: usize) -> PortResult<ContentId> {
    let bytes = serde_json::to_vec(metadata).map_err(|_| {
        domain(
            "nix_eval_metadata_json",
            "nix_eval.stream.job.meta",
            "job metadata could not be encoded canonically",
        )
    })?;
    if bytes.len() > max_bytes {
        return Err(domain(
            "nix_eval_metadata_bound",
            "nix_eval.stream.job.meta",
            "job metadata exceeds its named byte bound",
        ));
    }
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
        .map_err(PortFailure::Domain)
}

fn validate_store_path(path: &str, field: &str) -> PortResult<()> {
    if !path.starts_with("/nix/store/") || path.contains("/../") || path.contains("/./") {
        return Err(domain(
            "nix_eval_store_path",
            field,
            "record path is not a canonical stock Nix store path",
        ));
    }
    Ok(())
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

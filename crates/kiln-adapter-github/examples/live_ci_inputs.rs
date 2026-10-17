//! Prepare native route identities from the actual admitted GitHub configuration.
//!
//! This operator utility performs no credential resolution, dispatch, or mutation.

use std::io::Write;
use std::path::Path;

use kiln_adapter_github::profile::load_config;
use kiln_adapter_github::service::workflow_plan;
use kiln_adapter_lattice::lower_request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let profile_path = args.next().ok_or("usage: live_ci_inputs SERVICE_JSON")?;
    if args.next().is_some() {
        return Err("usage: live_ci_inputs SERVICE_JSON".into());
    }
    let admitted = load_config(Path::new(&profile_path))
        .map_err(|error| format!("profile admission failed: {error:?}"))?;
    let plan = workflow_plan(
        &admitted.config.workflow,
        &admitted.identity,
        &admitted.pipeline_identity,
        &admitted.identity,
        None,
    )
    .map_err(|error| format!("workflow admission failed: {error:?}"))?;
    let native = lower_request(&admitted.lattice, &plan)
        .map_err(|error| format!("native lowering failed: {error:?}"))?;
    let output = serde_json::json!({
        "schema": "kiln.live-ci-native-inputs.v1",
        "service_identity": admitted.identity,
        "pipeline_identity": admitted.pipeline_identity,
        "contract": admitted.lattice.contract,
        "transport": admitted.transport,
        "profile_identity": native.profile_identity,
        "graph_identity": native.graph.identity,
        "nodes": native.graph.nodes,
    });
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &output)?;
    writeln!(writer)?;
    Ok(())
}

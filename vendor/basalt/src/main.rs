use basalt::CapabilityGrant;
use basalt::EnforcementRequest;
#[cfg(feature = "cli")]
use basalt::cli_fs::{FsError, OutputRoot, PolicyRoot};
use basalt::enforce;
use basalt::parse_policy_json;
use clap::Parser;
use clap::Subcommand;
use snafu::ResultExt;
use snafu::Snafu;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Enforce(EnforceArgs),
}

#[derive(Debug, Parser)]
struct EnforceArgs {
    /// Capability root containing the policy file. Operation is confined here.
    #[arg(long, default_value = ".")]
    policy_root: String,
    #[arg(long)]
    policy: String,
    /// Capability root for optional receipt output.
    #[arg(long)]
    output_dir: Option<String>,
    #[arg(long)]
    contract: String,
    #[arg(long)]
    resource: String,
    #[arg(long)]
    ability: String,
    #[arg(long = "capability", value_parser = parse_capability)]
    capabilities: Vec<CapabilityGrant>,
}

#[derive(Debug, Snafu)]
enum CliError {
    #[snafu(display("failed to open policy capability root: {source}"))]
    OpenPolicyRoot { source: FsError },
    #[snafu(display("failed to read policy file {path:?}: {source}"))]
    ReadPolicy { path: String, source: FsError },
    #[snafu(display("failed to open output capability root: {source}"))]
    OpenOutputRoot { source: FsError },
    #[snafu(display("failed to write receipt {path:?}: {source}"))]
    WriteReceipt { path: String, source: FsError },
    #[snafu(display("enforcement failed: {source}"))]
    Enforce { source: basalt::EnforcementError },
    #[snafu(display("failed to serialize receipt: {source}"))]
    Serialize { source: serde_json::Error },
}

type CliResult<T> = Result<T, CliError>;

fn parse_capability(input: &str) -> Result<CapabilityGrant, String> {
    let Some((resource, ability)) = input.split_once('=') else {
        return Err("expected RESOURCE=ABILITY".to_owned());
    };
    if resource.is_empty() || ability.is_empty() {
        return Err("capability resource and ability must be non-empty".to_owned());
    }
    Ok(CapabilityGrant::new(resource, ability))
}

fn main() -> CliResult<()> {
    let args = Args::parse();
    match args.command {
        Command::Enforce(enforce_args) => run_enforce(enforce_args),
    }
}

fn run_enforce(args: EnforceArgs) -> CliResult<()> {
    let policy_root = PolicyRoot::open(args.policy_root.as_str()).context(OpenPolicyRootSnafu)?;
    let policy_json = policy_root
        .read_policy(args.policy.as_str())
        .context(ReadPolicySnafu {
            path: args.policy.clone(),
        })?;
    let policy = parse_policy_json(policy_json.as_str()).context(EnforceSnafu)?;
    let request = EnforcementRequest {
        contract: args.contract,
        resource: args.resource,
        ability: args.ability,
        capabilities: args.capabilities,
    };
    let receipt = enforce(&policy, &request).context(EnforceSnafu)?;
    let output = serde_json::to_string(&receipt).context(SerializeSnafu)?;
    println!("{output}");

    if let Some(output_dir) = args.output_dir {
        let output_root = OutputRoot::open(output_dir.as_str()).context(OpenOutputRootSnafu)?;
        output_root
            .write("enforce-receipt.json", output.as_str())
            .context(WriteReceiptSnafu {
                path: "enforce-receipt.json".to_owned(),
            })?;
    }
    Ok(())
}

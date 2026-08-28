#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![cfg(feature = "test-fixture")]
#![allow(
    unknown_lints,
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed fixture invariants"
)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "integration tests keep concrete Aspen, Kiln, and Lattice owners visible"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their protocol ownership"
)]

use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use kiln_adapter_aspen::profile::{admit_profile, profile_fixture};
use kiln_adapter_aspen::transport::{
    AspenServiceDecision, AspenServiceTransport, IngressSource, ServiceTransportOutcome,
    UnixAspenServiceTransport, build_ingress,
};
use kiln_adapter_lattice::{
    LatticeAdapter, PINNED_LATTICE_CONTRACT_LICENSE, PINNED_LATTICE_REVISION,
    PINNED_LATTICE_SOURCE, UnixWorkflowExchange,
};
use kiln_adapter_radicle::{
    AdapterLimits, CapabilitySelection, ConfigEffectClass, ConfigNode,
    PINNED_RADICLE_BROKER_REVISION, PINNED_RADICLE_LICENSE, PINNED_RADICLE_RUNTIME_REVISION,
    RADICLE_SHELL_SCHEMA, REQUIRED_NON_CLAIMS, RadicleShellConfig, RuntimeMode, WorkflowTemplate,
    admit_config,
};
use kiln_aspen_host::cohort::{HostCohort, HostPaths};
use kiln_aspen_host::effect::{
    LatticeEffectPort, LatticeEffectPortOptions, ObservationPacer, ThreadObservationPacer,
};
use kiln_aspen_host::runtime::KilnAspenHostRuntime;
use kiln_aspen_host::server::{ServerConfig, serve};
use kiln_core::types::ExternalRef;
use kiln_core::workflow::{WorkflowBounds, WorkflowControls};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};
use lattice_workflow_contract::{
    ACCEPTED_SCHEMA, BLAKE3_PREFIX, ExecutionObservation, ExecutionState, Identity,
    NodeObservation, NodeState, OBSERVATION_SCHEMA, WorkflowAccepted, observation_identity,
};
use lattice_workflow_transport::{
    ExchangeCommand, ExchangeRequest, ExchangeSuccess, HandlerError, TransportLimits,
    WorkflowExchangeHandler, WorkflowExchangeService, read_frame, serve_listener,
};

#[path = "hosted/support.rs"]
mod support;
use support::*;

#[path = "hosted/accepted.rs"]
mod accepted_test;
#[path = "hosted/recovery.rs"]
mod recovery_test;
#[path = "hosted/rejected.rs"]
mod rejected_test;
#[path = "hosted/uncertain.rs"]
mod uncertain_test;

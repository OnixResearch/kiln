//! Operator placement adapter. Requests reuse the native Herdr dispatcher;
//! the API never starts, stops, or resumes a provider session directly.

use kiln_adapter_herdr::{HerdrInventory, MachineLoad, Occupancy, place};
use kiln_core::fleet::ClaimContext;
use kiln_core::fleet::placement::{MovePlan, SplitPlan};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use kiln_implementer::admission::{MoveStatus, SplitStatus};
use serde::Serialize;

use crate::api::Service;
use crate::controls::filter_draining_occupancy;

pub use kiln_core::fleet::placement::{MoveRequest, MoveSubmit, SplitRequest, SplitSubmit};

#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub split: bool,
    #[serde(rename = "move")]
    pub move_workload: bool,
    pub move_reason: &'static str,
    pub active_move: bool,
    pub active_move_reason: &'static str,
}

#[must_use]
pub fn capabilities(service: &Service) -> Capabilities {
    let enabled = service.config.dispatch.enabled && service.herdr_client().is_some();
    Capabilities {
        split: enabled,
        move_workload: enabled,
        move_reason: if enabled {
            "Only unstarted reservations with no bound session can be rebound."
        } else {
            "The Herdr dispatch owner is not configured."
        },
        active_move: false,
        active_move_reason: "No configured owner can commit a fenced cross-host checkpoint handoff and resume.",
    }
}

pub fn preview_move(
    service: &Service,
    store: &mut Implementer,
    request: MoveRequest,
    now_ms: u64,
) -> Result<MovePlan, String> {
    Ok(prepare_move(service, store, &request, None, now_ms)?.plan)
}

pub fn submit_move(
    service: &Service,
    store: &mut Implementer,
    submission: MoveSubmit,
    now_ms: u64,
) -> Result<MoveStatus, String> {
    let MoveSubmit {
        plan,
        attempt,
        fence,
        reservation,
        destination,
    } = submission;
    let request = MoveRequest {
        attempt,
        fence,
        reservation,
        destination,
    };
    if let Some(existing) = store.fleet_move_status(&plan, now_ms)? {
        if existing.plan.request != request {
            return Err("fleet_placement_changed: request does not match the retained move".into());
        }
        return Ok(existing);
    }
    prepare_move(service, store, &request, Some(&plan), now_ms)
}

pub fn status_move(
    store: &mut Implementer,
    identity: &ContentId,
    now_ms: u64,
) -> Result<MoveStatus, String> {
    store
        .fleet_move_status(identity, now_ms)?
        .ok_or_else(|| "fleet_placement_unknown: no such reserved move".into())
}

fn prepare_move(
    service: &Service,
    store: &mut Implementer,
    request: &MoveRequest,
    expected: Option<&ContentId>,
    now_ms: u64,
) -> Result<MoveStatus, String> {
    request
        .validate()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let herdr = service
        .herdr_client()
        .filter(|_| service.config.dispatch.enabled)
        .ok_or("fleet_placement_unavailable: the Herdr dispatch owner is not configured")?;
    let inventory = herdr.inventory();
    if inventory.machine(&request.destination).is_none() {
        return Err("fleet_placement_unavailable: destination is not enrolled".into());
    }
    // Observe only the requested host, never offer a fallback.
    let load = match herdr.list_agents(&request.destination) {
        Ok(agents) => MachineLoad::Available(kiln_adapter_herdr::count_occupancy(&agents)),
        Err(failure) => MachineLoad::Unavailable(failure),
    };
    let controls = service.controls_snapshot();
    if controls.dispatch_pause.paused || controls.is_draining(&request.destination) {
        return Err(
            "fleet_placement_unavailable: fleet dispatch is paused or destination is draining"
                .into(),
        );
    }
    let mut admission = DestinationAdmission {
        inventory,
        occupancy: Occupancy {
            machines: [(request.destination.clone(), load)].into(),
        },
        kinds: &service.config.dispatch.kinds,
    };
    store.fleet_prepare_move(request, expected, now_ms, |work, attempt, old| {
        let source = super::planned_machine(inventory, old, attempt, work)
            .ok_or("fleet_placement_source: no enrolled host matches the old reservation")?;
        let destination = admission.reserve(&request.destination, &work.repository)?;
        let workspace = super::interactive_plan(work, attempt, destination)
            .map_err(|error| format!("fleet_placement_source: {error}"))?
            .workspace
            .identity;
        Ok((source.label.clone(), workspace))
    })
}

pub fn preview(
    service: &Service,
    store: &mut Implementer,
    request: SplitRequest,
    now_ms: u64,
) -> Result<SplitPlan, String> {
    Ok(prepare(service, store, &request, None, now_ms)?.plan)
}

pub fn submit(
    service: &Service,
    store: &mut Implementer,
    request: SplitSubmit,
    now_ms: u64,
) -> Result<SplitStatus, String> {
    let units = SplitRequest {
        units: request.units,
    };
    let ordered = units
        .ordered()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if let Some(existing) = store.fleet_split_status(&request.plan, now_ms)? {
        if !existing.plan.matches_units(&ordered) {
            return Err(
                "fleet_placement_changed: submitted units do not match the reserved plan".into(),
            );
        }
        return Ok(existing);
    }
    prepare(service, store, &units, Some(&request.plan), now_ms)
}

pub fn status(
    _service: &Service,
    store: &mut Implementer,
    identity: &ContentId,
    now_ms: u64,
) -> Result<SplitStatus, String> {
    store
        .fleet_split_status(identity, now_ms)?
        .ok_or_else(|| "fleet_placement_unknown: no such reserved split".into())
}

fn prepare(
    service: &Service,
    store: &mut Implementer,
    request: &SplitRequest,
    expected: Option<&ContentId>,
    now_ms: u64,
) -> Result<SplitStatus, String> {
    let ordered = request
        .ordered()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let herdr = service
        .herdr_client()
        .filter(|_| service.config.dispatch.enabled)
        .ok_or("fleet_placement_unavailable: the Herdr dispatch owner is not configured")?;
    let controls = service.controls_snapshot();
    if controls.dispatch_pause.paused {
        return Err("fleet_placement_unavailable: fleet dispatch is paused".into());
    }
    let inventory = herdr.inventory();
    for unit in &ordered {
        if inventory.machine(&unit.destination).is_none() || controls.is_draining(&unit.destination)
        {
            return Err(format!(
                "fleet_placement_unavailable: destination {} is not enrolled or is draining",
                unit.destination
            ));
        }
    }
    let foreign = if let Some(swarm) = &service.swarm {
        let articles = swarm
            .refresh(now_ms)
            .map_err(|_| "fleet_placement_unavailable: foreign claim observations unavailable")?;
        crate::swarm::foreign_claims(&articles, swarm.own_address(), now_ms)
    } else {
        Vec::new()
    };
    let occupancy = filter_draining_occupancy(&herdr.occupancy(), &service.controls_snapshot());
    let mut admission = DestinationAdmission {
        inventory,
        occupancy,
        kinds: &service.config.dispatch.kinds,
    };
    let context = ClaimContext {
        now_ms,
        attempts: &[],
        foreign: &foreign,
        coordinator: &service.config.machine,
    };
    store.fleet_prepare_split(
        request,
        expected,
        context,
        service.config.policy.claim_lease_ms,
        |unit, work, attempt| {
            let machine = admission.reserve(&unit.destination, &work.repository)?;
            super::interactive_plan(work, attempt, machine)
                .map_err(|error| format!("fleet_placement_source: {error}"))
        },
    )
}

struct DestinationAdmission<'a> {
    inventory: &'a HerdrInventory,
    occupancy: Occupancy,
    kinds: &'a [kiln_adapter_herdr::AgentKind],
}

impl<'a> DestinationAdmission<'a> {
    fn reserve(
        &mut self,
        destination: &str,
        repository: &str,
    ) -> Result<&'a kiln_adapter_herdr::HerdrMachine, String> {
        let unavailable = || {
            format!(
                "fleet_placement_unavailable: destination {destination} has no reachable compatible free slot or repository checkout"
            )
        };
        let machine = self
            .inventory
            .machine(destination)
            .filter(|machine| machine.checkout(repository).is_some())
            .ok_or_else(unavailable)?;
        let load = self
            .occupancy
            .machines
            .get_mut(destination)
            .ok_or_else(unavailable)?;
        let chosen = place(
            self.inventory,
            &Occupancy {
                machines: [(destination.to_owned(), load.clone())].into(),
            },
            self.kinds,
        )
        .ok_or_else(unavailable)?;
        // The preview reserves capacity only within this snapshot. Native
        // dispatch independently checks current capacity before launch.
        let MachineLoad::Available(counts) = load else {
            return Err(unavailable());
        };
        *counts.entry(chosen.kind).or_default() += 1;
        Ok(machine)
    }
}

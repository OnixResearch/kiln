//! Shell for cron schedules: durable reservation of one due occurrence.

use super::{digest, key};
use crate::automation::{ScheduleProfile, SourceRecord, filters};
use crate::engine::Engine;
use crate::schedule::Reservation;

impl Engine {
    // r[impl kiln.schedules.no_catch_up]
    // r[impl kiln.schedules.min_interval]
    /// Reserve one due cron occurrence. Missed occurrences (older than one
    /// minimum interval, e.g. after downtime) and occurrences closer than the
    /// minimum interval to the last fired run advance without a run. A fire
    /// binds the exact revision the schedule reference names at the due time.
    /// The next occurrence and the round-robin cursor persist before dispatch.
    pub(super) fn reserve_cron(
        &mut self,
        schedule: &ScheduleProfile,
        due: u64,
        now: u64,
        next: usize,
    ) -> Result<(), String> {
        let cron = filters::cron(schedule)?.ok_or("cron schedule missing")?;
        let settings_key = key("schedule", &schedule.id);
        let last_fired = self
            .state
            .automation
            .settings
            .get(&settings_key)
            .ok_or("schedule state missing")?
            .last_fired_ms;
        let (status, occurrence, next_due) = match cron
            .decide(due, last_fired, now, schedule.min_interval_ms)
            .map_err(|e| e.to_string())?
        {
            Reservation::NotDue => return Err("cron schedule selected before it was due".into()),
            Reservation::Missed {
                occurrence_ms,
                next_due_ms,
            } => ("missed", occurrence_ms, next_due_ms),
            Reservation::TooClose {
                occurrence_ms,
                next_due_ms,
                ..
            } => ("schedule_min_interval", occurrence_ms, next_due_ms),
            Reservation::Fire {
                occurrence_ms,
                next_due_ms,
            } => ("reserved", occurrence_ms, next_due_ms),
        };
        let revision = if status == "reserved" {
            self.profile
                .projects
                .iter()
                .find(|project| project.id == schedule.project_id)
                .and_then(|project| git2::Repository::open(&project.repository_path).ok())
                .and_then(|repository| {
                    repository
                        .revparse_single(&schedule.reference)
                        .and_then(|object| object.peel_to_commit())
                        .ok()
                        .map(|commit| commit.id().to_string())
                })
        } else {
            None
        };
        let setting = self
            .state
            .automation
            .settings
            .get_mut(&settings_key)
            .ok_or("schedule state missing")?;
        setting.next_due_ms = Some(next_due);
        if status != "reserved" {
            setting.last_status = Some(status.into());
            self.state.automation.next_registration = next;
            return self.persist();
        }
        // An unresolvable reference is a durable rejection, never a retry.
        let dispatch = revision.is_some();
        if dispatch {
            setting.last_fired_ms = Some(occurrence);
        } else {
            setting.last_status = Some("rejected".into());
        }
        let reservation = digest(&("schedule", &schedule.id, occurrence))?;
        self.state
            .automation
            .sources
            .entry(reservation.clone())
            .or_insert(SourceRecord {
                id: reservation.clone(),
                kind: "schedule".into(),
                registration_id: schedule.id.clone(),
                received_at_ms: now,
                status: if dispatch { "reserved" } else { "rejected" }.into(),
                run_id: None,
                error_code: (!dispatch).then(|| "schedule_source_unavailable".into()),
                project_id: schedule.project_id.clone(),
                reference: schedule.reference.clone(),
                pull_request: None,
                source_revision: revision,
                candidate_identity: None,
                affected: None,
                changed: None,
                untrusted: None,
                filter: None,
            });
        self.state.automation.next_registration = next;
        self.persist()?;
        if dispatch {
            self.dispatch_automation_source(&reservation)?;
        }
        Ok(())
    }
}

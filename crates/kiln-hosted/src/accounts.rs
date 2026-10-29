use crate::Actor;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_MEMBERS: i64 = 4096;
const MAX_INVITATIONS: i64 = 16384;
const MAX_PROJECTS: usize = 32;
const INVITATION_LIFETIME_MS: u64 = 7 * 24 * 60 * 60 * 1000;

fn storage(_: rusqlite::Error) -> String {
    "account storage failure".into()
}
fn peer_valid(peer: &str) -> bool {
    peer.len() == 64
        && peer
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn role_valid(role: &str) -> bool {
    matches!(role, "owner" | "admin" | "member" | "viewer" | "billing")
}
fn assignable(actor: &Actor, role: &str) -> Result<(), String> {
    if !role_valid(role)
        || !(actor.role == "owner" || (actor.role == "admin" && !matches!(role, "owner" | "admin")))
    {
        return Err("role assignment denied".into());
    }
    Ok(())
}
fn projects_valid(projects: &[String]) -> bool {
    projects.len() <= MAX_PROJECTS
        && projects
            .iter()
            .all(|p| p.len() <= 64 && crate::identifier(p))
        && projects
            .iter()
            .zip(projects.iter().skip(1))
            .all(|(left, right)| left < right)
}
fn timestamp(now: u64) -> Result<i64, String> {
    i64::try_from(now).map_err(|_| "timestamp overflow".into())
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_bootstrap(singleton INTEGER PRIMARY KEY CHECK(singleton=1),peer TEXT NOT NULL,organization TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS hosted_organizations(organization TEXT PRIMARY KEY,created_by TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS hosted_memberships(organization TEXT NOT NULL REFERENCES hosted_organizations(organization),peer TEXT NOT NULL,role TEXT NOT NULL,projects TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),active INTEGER NOT NULL CHECK(active IN(0,1)),changed_by TEXT NOT NULL,PRIMARY KEY(organization,peer));
    CREATE TABLE IF NOT EXISTS hosted_invitations(organization TEXT NOT NULL,id TEXT NOT NULL,peer TEXT NOT NULL,role TEXT NOT NULL,projects TEXT NOT NULL,expires INTEGER NOT NULL,inviter TEXT NOT NULL,inviter_revision INTEGER NOT NULL,consumed INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(organization,id));
    CREATE TABLE IF NOT EXISTS hosted_account_events(sequence INTEGER PRIMARY KEY,organization TEXT NOT NULL,actor TEXT NOT NULL,subject TEXT NOT NULL,operation TEXT NOT NULL,revision INTEGER NOT NULL);") .map_err(storage)
}

/// Startup-only: the configured server peer owns the initial organization once.
/// Reopening never restores a revoked membership or silently changes bootstrap authority.
pub fn bootstrap(db: &mut Connection, peer: &str, org: &str) -> Result<(), String> {
    if !peer_valid(peer) || !crate::identifier(org) {
        return Err("invalid bootstrap identity".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let prior: Option<(String, String)> = tx
        .query_row(
            "SELECT peer,organization FROM hosted_bootstrap WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some((p, o)) = prior {
        if p != peer || o != org {
            return Err("bootstrap authority differs from durable configuration".into());
        }
    } else {
        let existing: i64 = tx
            .query_row("SELECT COUNT(*) FROM hosted_organizations", [], |r| {
                r.get(0)
            })
            .map_err(storage)?;
        if existing != 0 {
            return Err("cannot bootstrap existing organization state".into());
        }
        tx.execute(
            "INSERT INTO hosted_bootstrap VALUES(1,?1,?2)",
            params![peer, org],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO hosted_organizations VALUES(?1,?2)",
            params![org, peer],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO hosted_memberships VALUES(?1,?2,'owner','[]',1,1,?2)",
            params![org, peer],
        )
        .map_err(storage)?;
        tx.execute("INSERT INTO hosted_account_events(organization,actor,subject,operation,revision) VALUES(?1,?2,?2,'bootstrap',1)",params![org,peer]).map_err(storage)?;
    }
    tx.commit().map_err(storage)
}

// r[impl kiln.hosted.accounts]
pub fn authorize(db: &Connection, peer: &str, organization: &str) -> Result<Actor, String> {
    if !peer_valid(peer) || !crate::identifier(organization) {
        return Err("membership denied".into());
    }
    let role: Option<String> = db
        .query_row(
            "SELECT role FROM hosted_memberships WHERE organization=?1 AND peer=?2 AND active=1",
            params![organization, peer],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    let role = role.filter(|r| role_valid(r)).ok_or("membership denied")?;
    Ok(Actor {
        peer: peer.into(),
        organization: organization.into(),
        role,
    })
}

/// Project checks always consult durable current membership, never the RPC's cached role.
pub fn require_project(
    db: &Connection,
    actor: &Actor,
    project: &str,
    roles: &[&str],
) -> Result<Actor, String> {
    if !crate::identifier(project) {
        return Err("invalid project".into());
    }
    let current = authorize(db, &actor.peer, &actor.organization)?;
    crate::require(&current, roles)?;
    if !matches!(current.role.as_str(), "owner" | "admin") {
        let projects: String = db.query_row("SELECT projects FROM hosted_memberships WHERE organization=?1 AND peer=?2 AND active=1",params![current.organization,current.peer],|r| r.get(0)).map_err(storage)?;
        let projects: Vec<String> =
            serde_json::from_str(&projects).map_err(|_| "invalid stored project scopes")?;
        if !projects_valid(&projects) || !projects.iter().any(|p| p == project) {
            return Err("project scope denied".into());
        }
    }
    Ok(current)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    peer: String,
    role: String,
    #[serde(default)]
    projects: Vec<String>,
    expires_at_ms: u64,
    nonce: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Acceptance {
    invitation: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipChange {
    peer: String,
    expected_revision: i64,
    role: String,
    #[serde(default)]
    projects: Vec<String>,
    active: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationRevocation {
    invitation: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberList {
    #[serde(default)]
    after: Option<String>,
}

pub fn accept_invitation(
    db: &mut Connection,
    peer: &str,
    org: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    let input: Acceptance =
        serde_json::from_value(input).map_err(|_| "invalid invitation acceptance")?;
    if !peer_valid(peer) || !crate::identifier(org) || input.invitation.len() > 128 {
        return Err("invalid invitation identity".into());
    }
    let now = timestamp(now_ms)?;
    // Nest inside the service's request transaction when accepting through RPC.
    let tx = db.savepoint().map_err(storage)?;
    let row: Option<(String,String,String,i64,String,i64,i64)> = tx.query_row("SELECT peer,role,projects,expires,inviter,inviter_revision,consumed FROM hosted_invitations WHERE organization=?1 AND id=?2",params![org,input.invitation],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(storage)?;
    let (intended, role, projects, expires, inviter, inviter_revision, consumed) =
        row.ok_or("invitation unavailable")?;
    if intended != peer || consumed != 0 || expires <= now {
        return Err("invitation unavailable".into());
    }
    let inviter_actor = authorize(&tx, &inviter, org)?;
    assignable(&inviter_actor, &role)?;
    let current_revision: i64 = tx
        .query_row(
            "SELECT revision FROM hosted_memberships WHERE organization=?1 AND peer=?2",
            params![org, inviter],
            |r| r.get(0),
        )
        .map_err(storage)?;
    if current_revision != inviter_revision {
        return Err("invitation issuer authority changed".into());
    }
    let prior: Option<(i64, i64)> = tx
        .query_row(
            "SELECT revision,active FROM hosted_memberships WHERE organization=?1 AND peer=?2",
            params![org, peer],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    if prior.is_some_and(|(_, active)| active == 1) {
        return Err("peer already enrolled".into());
    }
    let revision = prior
        .map_or(Some(1), |(revision, _)| revision.checked_add(1))
        .ok_or("membership revision overflow")?;
    if prior.is_none() {
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM hosted_memberships WHERE organization=?1",
                [org],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if count >= MAX_MEMBERS {
            return Err("organization membership limit".into());
        }
    }
    tx.execute("INSERT INTO hosted_memberships VALUES(?1,?2,?3,?4,?5,1,?6) ON CONFLICT(organization,peer) DO UPDATE SET role=excluded.role,projects=excluded.projects,revision=excluded.revision,active=1,changed_by=excluded.changed_by",params![org,peer,role,projects,revision,inviter]).map_err(storage)?;
    tx.execute(
        "UPDATE hosted_invitations SET consumed=1 WHERE organization=?1 AND id=?2 AND consumed=0",
        params![org, input.invitation],
    )
    .map_err(storage)?;
    tx.execute("INSERT INTO hosted_account_events(organization,actor,subject,operation,revision) VALUES(?1,?2,?2,'accept',?3)",params![org,peer,revision]).map_err(storage)?;
    tx.commit().map_err(storage)?;
    Ok(json!({"peer":peer,"organization":org,"role":role,"revision":revision}))
}

// r[impl kiln.hosted.accounts]
pub fn handle(
    db: &mut Connection,
    actor: &Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let actor = authorize(&tx, &actor.peer, &actor.organization)?;
    let result = match action {
        "list" => {
            crate::require(&actor, &["owner", "admin"])?;
            let input: MemberList =
                serde_json::from_value(input).map_err(|_| "invalid membership list")?;
            if input.after.as_ref().is_some_and(|p| !peer_valid(p)) {
                return Err("invalid membership page cursor".into());
            }
            let mut statement = tx.prepare("SELECT peer,role,projects,revision,active FROM hosted_memberships WHERE organization=?1 AND peer>?2 ORDER BY peer LIMIT 129").map_err(storage)?;
            let rows = statement
                .query_map(
                    params![actor.organization, input.after.unwrap_or_default()],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, bool>(4)?,
                        ))
                    },
                )
                .map_err(storage)?;
            let mut members = Vec::new();
            for row in rows {
                let (peer, role, projects, revision, active) = row.map_err(storage)?;
                let projects: Value =
                    serde_json::from_str(&projects).map_err(|_| "stored project scopes invalid")?;
                members.push(json!({"peer":peer,"role":role,"projects":projects,"revision":revision,"active":active}));
            }
            let next = if members.len() > 128 {
                members.pop();
                members
                    .last()
                    .map(|m| m.get("peer").unwrap_or(&Value::Null).clone())
            } else {
                None
            };
            json!({"members":members,"next":next})
        }
        "invite" => {
            let invitation: Invitation =
                serde_json::from_value(input).map_err(|_| "invalid invitation")?;
            assignable(&actor, &invitation.role)?;
            if !peer_valid(&invitation.peer)
                || !projects_valid(&invitation.projects)
                || !crate::identifier(&invitation.nonce)
                || invitation.expires_at_ms <= now_ms
                || invitation.expires_at_ms
                    > now_ms
                        .checked_add(INVITATION_LIFETIME_MS)
                        .ok_or("expiry overflow")?
            {
                return Err("invalid invitation scope or expiry".into());
            }
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM hosted_invitations WHERE organization=?1",
                    [&actor.organization],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            if count >= MAX_INVITATIONS {
                return Err("invitation retention limit".into());
            }
            let revision: i64 = tx
                .query_row(
                    "SELECT revision FROM hosted_memberships WHERE organization=?1 AND peer=?2",
                    params![actor.organization, actor.peer],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            let id = crate::identity(&(
                "kiln.hosted.invitation.v1",
                &actor.organization,
                &actor.peer,
                revision,
                &invitation.peer,
                &invitation.role,
                &invitation.projects,
                invitation.expires_at_ms,
                &invitation.nonce,
            ))?;
            tx.execute(
                "INSERT INTO hosted_invitations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0)",
                params![
                    actor.organization,
                    id,
                    invitation.peer,
                    invitation.role,
                    serde_json::to_string(&invitation.projects)
                        .map_err(|_| "scope encoding failed")?,
                    timestamp(invitation.expires_at_ms)?,
                    actor.peer,
                    revision
                ],
            )
            .map_err(storage)?;
            tx.execute("INSERT INTO hosted_account_events(organization,actor,subject,operation,revision) VALUES(?1,?2,?3,'invite',?4)",params![actor.organization,actor.peer,invitation.peer,revision]).map_err(storage)?;
            json!({"invitation":id,"expires_at_ms":invitation.expires_at_ms})
        }
        "revoke_invitation" => {
            crate::require(&actor, &["owner", "admin"])?;
            let revocation: InvitationRevocation =
                serde_json::from_value(input).map_err(|_| "invalid invitation revocation")?;
            let row: Option<(String,String)> = tx.query_row("SELECT peer,role FROM hosted_invitations WHERE organization=?1 AND id=?2 AND consumed=0",params![actor.organization,revocation.invitation],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
            let (peer, role) = row.ok_or("invitation unavailable")?;
            assignable(&actor, &role)?;
            tx.execute(
                "UPDATE hosted_invitations SET consumed=1 WHERE organization=?1 AND id=?2",
                params![actor.organization, revocation.invitation],
            )
            .map_err(storage)?;
            tx.execute("INSERT INTO hosted_account_events(organization,actor,subject,operation,revision) VALUES(?1,?2,?3,'revoke_invitation',0)",params![actor.organization,actor.peer,peer]).map_err(storage)?;
            json!({"revoked":true})
        }
        "set_membership" => {
            let change: MembershipChange =
                serde_json::from_value(input).map_err(|_| "invalid membership change")?;
            assignable(&actor, &change.role)?;
            if !peer_valid(&change.peer) || !projects_valid(&change.projects) {
                return Err("invalid membership scope".into());
            }
            let prior: Option<(String,i64,bool)> = tx.query_row("SELECT role,revision,active FROM hosted_memberships WHERE organization=?1 AND peer=?2",params![actor.organization,change.peer],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
            let (role, revision, active) = prior.ok_or("membership unavailable")?;
            assignable(&actor, &role)?;
            if revision != change.expected_revision || !active {
                return Err("stale membership revision".into());
            }
            if role == "owner" && (!change.active || change.role != "owner") {
                let owners: i64 = tx.query_row("SELECT COUNT(*) FROM hosted_memberships WHERE organization=?1 AND role='owner' AND active=1",[&actor.organization],|r|r.get(0)).map_err(storage)?;
                if owners <= 1 {
                    return Err("cannot remove final organization owner".into());
                }
            }
            let revision = revision
                .checked_add(1)
                .ok_or("membership revision overflow")?;
            tx.execute("UPDATE hosted_memberships SET role=?3,projects=?4,revision=?5,active=?6,changed_by=?7 WHERE organization=?1 AND peer=?2",params![actor.organization,change.peer,change.role,serde_json::to_string(&change.projects).map_err(|_|"scope encoding failed")?,revision,change.active,actor.peer]).map_err(storage)?;
            // Any outstanding invitation to this peer is revoked with membership changes.
            tx.execute(
                "UPDATE hosted_invitations SET consumed=1 WHERE organization=?1 AND peer=?2",
                params![actor.organization, change.peer],
            )
            .map_err(storage)?;
            tx.execute("INSERT INTO hosted_account_events(organization,actor,subject,operation,revision) VALUES(?1,?2,?3,'set_membership',?4)",params![actor.organization,actor.peer,change.peer,revision]).map_err(storage)?;
            json!({"peer":change.peer,"revision":revision,"active":change.active})
        }
        _ => return Err("unknown accounts action".into()),
    };
    tx.commit().map_err(storage)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Connection, Actor, String) {
        let mut db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let owner = "01".repeat(32);
        bootstrap(&mut db, &owner, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let actor = authorize(&db, &owner, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        (db, actor, "02".repeat(32))
    }
    fn invite(db: &mut Connection, owner: &Actor, peer: &str, nonce: &str) -> Value {
        handle(db,owner,"invite",json!({"peer":peer,"role":"member","projects":["app"],"expires_at_ms":5000,"nonce":nonce}),1000).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}")).get("invitation").unwrap_or(&Value::Null).clone()
    }
    // r[verify kiln.hosted.accounts.scenario.membership]
    #[test]
    fn intended_peer_once_scope_and_revocation() {
        let (mut db, owner, peer) = setup();
        let invitation = invite(&mut db, &owner, &peer, "first");
        assert!(
            accept_invitation(
                &mut db,
                &"03".repeat(32),
                "acme",
                json!({"invitation":invitation}),
                1001
            )
            .is_err()
        );
        assert!(
            accept_invitation(
                &mut db,
                &peer,
                "other",
                json!({"invitation":invitation}),
                1001
            )
            .is_err()
        );
        accept_invitation(
            &mut db,
            &peer,
            "acme",
            json!({"invitation":invitation}),
            1001,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(
            accept_invitation(
                &mut db,
                &peer,
                "acme",
                json!({"invitation":invitation}),
                1002
            )
            .is_err()
        );
        let member = authorize(&db, &peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        require_project(&db, &member, "app", &["member"])
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(require_project(&db, &member, "private", &["member"]).is_err());
        assert!(handle(&mut db,&member,"invite",json!({"peer":"04".repeat(32),"role":"owner","expires_at_ms":5000,"nonce":"escalate"}),1002).is_err());
        handle(&mut db,&owner,"set_membership",json!({"peer":peer,"expected_revision":1,"role":"member","projects":["app"],"active":false}),1003).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(authorize(&db, &peer, "acme").is_err());
        assert!(require_project(&db, &member, "app", &["member"]).is_err());
        assert!(
            accept_invitation(
                &mut db,
                &peer,
                "acme",
                json!({"invitation":invitation}),
                1004
            )
            .is_err()
        );
    }
    // r[verify kiln.hosted.accounts.scenario.membership]
    #[test]
    fn final_owner_expiry_and_bootstrap_pin() {
        let (mut db, owner, peer) = setup();
        assert!(
            handle(
                &mut db,
                &owner,
                "set_membership",
                json!({"peer":owner.peer,"expected_revision":1,"role":"viewer","active":true}),
                1
            )
            .is_err()
        );
        let invitation = invite(&mut db, &owner, &peer, "expired");
        assert!(
            accept_invitation(
                &mut db,
                &peer,
                "acme",
                json!({"invitation":invitation}),
                5000
            )
            .is_err()
        );
        bootstrap(&mut db, &owner.peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(bootstrap(&mut db, &peer, "acme").is_err());
        assert!(authorize(&db, &owner.peer, "other").is_err());
    }
    #[test]
    fn memberships_survive_reopen_and_inviter_revision_changes() {
        let dir = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let path = dir.path().join("accounts.sqlite");
        let owner = "01".repeat(32);
        let peer = "02".repeat(32);
        let mut db = Connection::open(&path)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        bootstrap(&mut db, &owner, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let actor = authorize(&db, &owner, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let invitation = invite(&mut db, &actor, &peer, "retained");
        drop(db);
        let mut db = Connection::open(&path)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let actor = authorize(&db, &owner, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        handle(
            &mut db,
            &actor,
            "set_membership",
            json!({"peer":owner,"expected_revision":1,"role":"owner","active":true}),
            1001,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(
            accept_invitation(
                &mut db,
                &peer,
                "acme",
                json!({"invitation":invitation}),
                1002
            )
            .is_err()
        );
    }
    // r[verify kiln.hosted.accounts.scenario.membership]
    #[test]
    fn ownership_transfer_does_not_rebootstrap_revoked_peer() {
        let (mut db, owner, peer) = setup();
        let invitation = handle(
            &mut db,
            &owner,
            "invite",
            json!({"peer":peer,"role":"owner","expires_at_ms":5000,"nonce":"ownership"}),
            1000,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"))
        .get("invitation")
        .unwrap_or(&Value::Null)
        .clone();
        accept_invitation(
            &mut db,
            &peer,
            "acme",
            json!({"invitation":invitation}),
            1001,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let second = authorize(&db, &peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        handle(
            &mut db,
            &second,
            "set_membership",
            json!({"peer":owner.peer,"expected_revision":1,"role":"owner","active":false}),
            1002,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        bootstrap(&mut db, &owner.peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(authorize(&db, &owner.peer, "acme").is_err());
        assert!(handle(&mut db, &owner, "list", json!({}), 1003).is_err());
        assert!(
            handle(
                &mut db,
                &second,
                "set_membership",
                json!({"peer":peer,"expected_revision":1,"role":"member","active":true}),
                1004
            )
            .is_err()
        );
    }
}

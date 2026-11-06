//! Local managed-VMM overlap evidence, separate from whole-attempt reservations.
//! r[impl kiln.hosted.microvm.host_placement]
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub(crate) fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_vm_active(org TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,host TEXT NOT NULL,pid INTEGER,start_ticks TEXT,PRIMARY KEY(org,attempt,job));
      CREATE TABLE IF NOT EXISTS hosted_vm_overlap(org TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,peer_org TEXT NOT NULL,peer_attempt TEXT NOT NULL,peer_job TEXT NOT NULL,PRIMARY KEY(org,attempt,job,peer_org,peer_attempt,peer_job));")
        .map_err(|error| error.to_string())?;
    let mut query = db
        .prepare("PRAGMA table_info(hosted_vm_active)")
        .map_err(|error| error.to_string())?;
    let columns = query
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for (column, kind) in [("pid", "INTEGER"), ("start_ticks", "TEXT")] {
        if !columns.iter().any(|name| name == column) {
            db.execute_batch(&format!(
                "ALTER TABLE hosted_vm_active ADD COLUMN {column} {kind}"
            ))
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Linux start-time binds a live PID to one process lifetime. Stopped/dead
/// processes (including unreaped zombies) must not become observed peers.
fn live_start_ticks(pid: u32) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(format!("/proc/{pid}/stat"))
        .ok()?
        .take(4097)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() > 4096 {
        return None;
    }
    let (_, fields) = text.rsplit_once(')')?;
    let mut fields = fields.split_whitespace();
    if matches!(fields.next()?, "Z" | "X" | "x") {
        return None;
    }
    fields.nth(18).map(str::to_owned)
}

/// Caller supplies an observed live VMM before guest dispatch. Candidate peers
/// are independently checked, not inferred from an uncleared teardown marker.
pub(crate) fn begin(
    db: &Connection,
    org: &str,
    attempt: &str,
    job: &str,
    pid: u32,
) -> Result<(), String> {
    let host = crate::microvm::physical_host_identity();
    let start_ticks = live_start_ticks(pid).ok_or("observed VMM is no longer live")?;
    db.execute_batch("SAVEPOINT vm_overlap")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        db.execute("DELETE FROM hosted_vm_active WHERE NOT EXISTS(SELECT 1 FROM hosted_execution e WHERE e.org=hosted_vm_active.org AND e.attempt=hosted_vm_active.attempt AND e.state IN ('reserved','running','unknown'))", []).map_err(|error| error.to_string())?;
        let mut query = db
            .prepare("SELECT org,attempt,job,pid,start_ticks FROM hosted_vm_active WHERE host=?1")
            .map_err(|error| error.to_string())?;
        let peers = query
            .query_map(params![host], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<u32>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        for peer in peers {
            let (peer_org, peer_attempt, peer_job, peer_pid, peer_start) =
                peer.map_err(|error| error.to_string())?;
            let live = peer_pid.and_then(live_start_ticks);
            if live.is_some() && live == peer_start {
                db.execute(
                    "INSERT OR IGNORE INTO hosted_vm_overlap VALUES(?1,?2,?3,?4,?5,?6)",
                    params![org, attempt, job, peer_org, peer_attempt, peer_job],
                )
                .map_err(|error| error.to_string())?;
                db.execute(
                    "INSERT OR IGNORE INTO hosted_vm_overlap VALUES(?1,?2,?3,?4,?5,?6)",
                    params![peer_org, peer_attempt, peer_job, org, attempt, job],
                )
                .map_err(|error| error.to_string())?;
            }
        }
        db.execute("INSERT INTO hosted_vm_active(org,attempt,job,host,pid,start_ticks) VALUES(?1,?2,?3,?4,?5,?6)", params![org,attempt,job,host,pid,start_ticks]).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = db.execute_batch("ROLLBACK TO vm_overlap");
    }
    db.execute_batch("RELEASE vm_overlap")
        .map_err(|error| error.to_string())?;
    result
}

pub(crate) fn finish(
    db: &Connection,
    org: &str,
    attempt: &str,
    job: &str,
    cleaned: bool,
) -> Result<Value, String> {
    if cleaned {
        db.execute(
            "DELETE FROM hosted_vm_active WHERE org=?1 AND attempt=?2 AND job=?3",
            params![org, attempt, job],
        )
        .map_err(|error| error.to_string())?;
    }
    let mut query = db.prepare("SELECT peer_org,peer_attempt,peer_job FROM hosted_vm_overlap WHERE org=?1 AND attempt=?2 AND job=?3 ORDER BY peer_org,peer_attempt,peer_job").map_err(|error| error.to_string())?;
    let peers = query.query_map(params![org,attempt,job], |row| Ok(json!({"organization":row.get::<_,String>(0)?,"attempt":row.get::<_,String>(1)?,"job":row.get::<_,String>(2)?}))).map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>,_>>().map_err(|error| error.to_string())?;
    Ok(json!(peers))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_only_running_peers_symmetrically_across_attempts() {
        let db = Connection::open_in_memory().unwrap_or_else(|error| unreachable!("{error}"));
        db.execute_batch("CREATE TABLE hosted_execution(org TEXT,attempt TEXT,state TEXT); INSERT INTO hosted_execution VALUES('a','one','running'),('b','two','running'),('a','three','running');").unwrap_or_else(|error| unreachable!("{error}"));
        init(&db).unwrap_or_else(|error| unreachable!("{error}"));
        begin(&db, "a", "one", "first", std::process::id())
            .unwrap_or_else(|error| unreachable!("{error}"));
        begin(&db, "b", "two", "second", std::process::id())
            .unwrap_or_else(|error| unreachable!("{error}"));
        let first =
            finish(&db, "a", "one", "first", true).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            first,
            json!([{"organization":"b","attempt":"two","job":"second"}])
        );
        begin(&db, "a", "three", "third", std::process::id())
            .unwrap_or_else(|error| unreachable!("{error}"));
        let second =
            finish(&db, "b", "two", "second", true).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            second,
            json!([{"organization":"a","attempt":"one","job":"first"},{"organization":"a","attempt":"three","job":"third"}])
        );
        assert_eq!(
            finish(&db, "a", "three", "third", true)
                .unwrap_or_else(|error| unreachable!("{error}")),
            json!([{"organization":"b","attempt":"two","job":"second"}])
        );
    }

    #[test]
    fn stopped_vmm_before_finish_is_not_observed_as_a_peer() {
        let db = Connection::open_in_memory().unwrap_or_else(|error| unreachable!("{error}"));
        db.execute_batch("CREATE TABLE hosted_execution(org TEXT,attempt TEXT,state TEXT); INSERT INTO hosted_execution VALUES('a','old','running'),('a','new','running');").unwrap_or_else(|error| unreachable!("{error}"));
        init(&db).unwrap_or_else(|error| unreachable!("{error}"));
        let mut peer = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let admitted = begin(&db, "a", "old", "stopped", peer.id());
        let _ = peer.kill();
        let _ = peer.wait();
        admitted.unwrap_or_else(|error| unreachable!("{error}"));
        // Deliberately leave the old marker until after the next begin.
        begin(&db, "a", "new", "live", std::process::id())
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            finish(&db, "a", "new", "live", true).unwrap_or_else(|error| unreachable!("{error}")),
            json!([])
        );
        assert_eq!(
            finish(&db, "a", "old", "stopped", true)
                .unwrap_or_else(|error| unreachable!("{error}")),
            json!([])
        );
    }
}

//! Bounded durable queue for authenticated GitHub issue deliveries.
//!
//! The webhook receiver stores exact delivery bytes after authentication; a
//! separate scheduler consumes and admits them under operator policy. Storing a
//! delivery is not admission, execution or acceptance.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, params};

const BUSY_TIMEOUT_MS: u64 = 10_000;
const MAX_BODY_BYTES: usize = 1_048_576;
const MAX_DELIVERIES: usize = 65_536;
const MAX_DELIVERY_ID_BYTES: usize = 64;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS implementation_deliveries (
    delivery_id TEXT PRIMARY KEY,
    event TEXT NOT NULL,
    signature TEXT NOT NULL,
    body BLOB NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedDelivery {
    pub delivery_id: String,
    pub event: String,
    pub signature: String,
    pub body: Vec<u8>,
    pub recorded_at_ms: u64,
}

#[derive(Debug)]
pub struct DeliveryQueue {
    connection: Connection,
    maximum_deliveries: usize,
}

impl DeliveryQueue {
    pub fn open(path: &Path, maximum_deliveries: usize) -> Result<Self, String> {
        if maximum_deliveries == 0 || maximum_deliveries > MAX_DELIVERIES {
            return Err("delivery queue bound is zero or exceeds its maximum".to_string());
        }
        let connection = Connection::open(path).map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection.execute_batch(SCHEMA).map_err(text)?;
        Ok(Self {
            connection,
            maximum_deliveries,
        })
    }

    /// Store one exact authenticated delivery. Repeating a delivery identity is
    /// idempotent. A full queue refuses instead of evicting.
    pub fn record(
        &mut self,
        delivery_id: &str,
        event: &str,
        signature: &str,
        body: &[u8],
        recorded_at_ms: u64,
    ) -> Result<bool, String> {
        if !valid_delivery_id(delivery_id)
            || event.is_empty()
            || event.len() > 64
            || signature.is_empty()
            || signature.len() > 128
            || body.is_empty()
            || body.len() > MAX_BODY_BYTES
        {
            return Err("delivery is malformed or exceeds its byte bound".to_string());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(text)?;
        let exists: bool = transaction
            .query_row(
                "SELECT 1 FROM implementation_deliveries WHERE delivery_id = ?1",
                params![delivery_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            return Ok(false);
        }
        let count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM implementation_deliveries",
                [],
                |row| row.get(0),
            )
            .map_err(text)?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= self.maximum_deliveries {
            return Err("delivery queue is full".to_string());
        }
        transaction
            .execute(
                "INSERT INTO implementation_deliveries
                 (delivery_id, event, signature, body, recorded_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    delivery_id,
                    event,
                    signature,
                    body,
                    i64::try_from(recorded_at_ms).map_err(text)?
                ],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)?;
        Ok(true)
    }

    /// Oldest queued deliveries, bounded by `limit`.
    pub fn list(&self, limit: usize) -> Result<Vec<QueuedDelivery>, String> {
        if limit == 0 || limit > self.maximum_deliveries {
            return Err("delivery list bound is zero or exceeds the queue bound".to_string());
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT delivery_id, event, signature, body, recorded_at_ms
                 FROM implementation_deliveries
                 ORDER BY recorded_at_ms ASC, delivery_id ASC LIMIT ?1",
            )
            .map_err(text)?;
        let rows = statement
            .query_map(params![i64::try_from(limit).map_err(text)?], |row| {
                Ok(QueuedDelivery {
                    delivery_id: row.get(0)?,
                    event: row.get(1)?,
                    signature: row.get(2)?,
                    body: row.get(3)?,
                    recorded_at_ms: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                })
            })
            .map_err(text)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(text)
    }

    pub fn remove(&mut self, delivery_id: &str) -> Result<bool, String> {
        let removed = self
            .connection
            .execute(
                "DELETE FROM implementation_deliveries WHERE delivery_id = ?1",
                params![delivery_id],
            )
            .map_err(text)?;
        Ok(removed == 1)
    }

    pub fn len(&self) -> Result<usize, String> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM implementation_deliveries",
                [],
                |row| row.get(0),
            )
            .map_err(text)?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }

    pub fn is_empty(&self) -> Result<bool, String> {
        Ok(self.len()? == 0)
    }
}

fn valid_delivery_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        && value.len() <= MAX_DELIVERY_ID_BYTES
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> (tempfile::TempDir, DeliveryQueue) {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue = DeliveryQueue::open(&directory.path().join("queue.db"), 2)
            .unwrap_or_else(|_| unreachable!());
        (directory, queue)
    }

    #[test]
    fn repeated_delivery_is_idempotent_and_removable() {
        let (_directory, mut queue) = queue();
        let id = "00000000-0000-0000-0000-000000000001";
        assert!(
            queue
                .record(id, "issues", "sha256=00", b"body", 1)
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(
            !queue
                .record(id, "issues", "sha256=00", b"body", 1)
                .unwrap_or_else(|_| unreachable!())
        );
        assert_eq!(queue.len().unwrap_or_else(|_| unreachable!()), 1);
        let listed = queue.list(1).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            listed.first().map(|row| row.body.as_slice()),
            Some(b"body".as_slice())
        );
        assert!(queue.remove(id).unwrap_or_else(|_| unreachable!()));
        assert!(queue.is_empty().unwrap_or_else(|_| unreachable!()));
    }

    #[test]
    fn full_queue_and_bad_delivery_fail_closed() {
        let (_directory, mut queue) = queue();
        assert!(
            queue
                .record(
                    "00000000-0000-0000-0000-000000000001",
                    "issues",
                    "sha256=00",
                    b"a",
                    1
                )
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(
            queue
                .record(
                    "00000000-0000-0000-0000-000000000002",
                    "issues",
                    "sha256=00",
                    b"b",
                    2
                )
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(
            queue
                .record(
                    "00000000-0000-0000-0000-000000000003",
                    "issues",
                    "sha256=00",
                    b"c",
                    3
                )
                .is_err()
        );
        assert!(
            queue
                .record("not-a-delivery", "issues", "sha256=00", b"body", 4)
                .is_err()
        );
        assert!(
            queue
                .record(
                    "00000000-0000-0000-0000-000000000004",
                    "issues",
                    "sha256=00",
                    b"",
                    4
                )
                .is_err()
        );
    }
}

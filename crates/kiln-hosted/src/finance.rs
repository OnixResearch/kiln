//! Durable integer-money admission. Only trusted executor entry points may meter usage.
use hmac::{Hmac, Mac};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;

const MAX_MONEY: u64 = 1_000_000_000_000;
const MAX_UNITS: u64 = 1_000_000_000;
const MAX_CONCURRENT: u64 = 1024;
const MAX_ROWS: usize = 100;
const MAX_EVIDENCE: usize = 4096;
const STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
const STRIPE_VERSION: &str = "2024-06-20";
const WEBHOOK_TOLERANCE_SECONDS: u64 = 300;
type Result<T, E = String> = std::result::Result<T, E>;
fn sql(e: rusqlite::Error) -> String {
    format!("finance storage: {e}")
}
fn num(v: &Value, k: &str, max: u64) -> Result<u64> {
    let n = v
        .get(k)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{k} must be an unsigned integer"))?;
    if n > max {
        return Err(format!("{k} exceeds finite maximum"));
    }
    Ok(n)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    let s = v
        .get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{k} must be text"))?;
    if !crate::identifier(s) || s.len() > 256 {
        return Err(format!("invalid {k}"));
    }
    Ok(s)
}
fn currency(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_lowercase())
}
fn integer(n: u64) -> Result<i64> {
    i64::try_from(n).map_err(|_| "integer overflow".into())
}
fn bounded_evidence(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > MAX_EVIDENCE {
        Err("missing or oversized trusted evidence".into())
    } else {
        Ok(())
    }
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(|| "integer overflow".into())
}
fn authority(a: &crate::Actor) -> Result<()> {
    crate::require(a, &["owner", "billing"])
}

pub fn init(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS finance_prices (
      org TEXT NOT NULL, id TEXT NOT NULL, class TEXT NOT NULL, currency TEXT NOT NULL,
      unit_minor INTEGER NOT NULL CHECK(unit_minor>0), unit_ms INTEGER NOT NULL CHECK(unit_ms>0),
      max_units INTEGER NOT NULL CHECK(max_units>0), admitted_by TEXT NOT NULL, created_ms INTEGER NOT NULL,
      PRIMARY KEY(org,id));
    CREATE TABLE IF NOT EXISTS finance_accounts (
      org TEXT PRIMARY KEY, currency TEXT NOT NULL, revision INTEGER NOT NULL,
      cap_minor INTEGER NOT NULL, request_max_minor INTEGER NOT NULL, max_concurrent INTEGER NOT NULL,
      spent_minor INTEGER NOT NULL DEFAULT 0, reserved_minor INTEGER NOT NULL DEFAULT 0,
      credits_minor INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE IF NOT EXISTS finance_changes (
      org TEXT NOT NULL, request_id TEXT NOT NULL, kind TEXT NOT NULL, fingerprint TEXT NOT NULL,
      result TEXT NOT NULL, actor TEXT NOT NULL, created_ms INTEGER NOT NULL, PRIMARY KEY(org,request_id));
    CREATE TABLE IF NOT EXISTS finance_reservations (
      org TEXT NOT NULL, attempt TEXT NOT NULL, id TEXT NOT NULL, fingerprint TEXT NOT NULL,
      price_id TEXT NOT NULL, class TEXT NOT NULL, binding TEXT NOT NULL, max_units INTEGER NOT NULL,
      maximum_minor INTEGER NOT NULL, state TEXT NOT NULL, actor TEXT NOT NULL,
      created_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL, PRIMARY KEY(org,attempt), UNIQUE(org,id));
    CREATE TABLE IF NOT EXISTS finance_usage (
      org TEXT NOT NULL, receipt TEXT NOT NULL, attempt TEXT NOT NULL, price_id TEXT NOT NULL,
      fingerprint TEXT NOT NULL, units INTEGER NOT NULL, amount_minor INTEGER NOT NULL,
      execution_outcome TEXT NOT NULL, evidence TEXT NOT NULL, created_ms INTEGER NOT NULL,
      PRIMARY KEY(org,receipt), UNIQUE(org,attempt));
    CREATE TABLE IF NOT EXISTS finance_observations (
      org TEXT NOT NULL, id TEXT NOT NULL, attempt TEXT NOT NULL, evidence TEXT NOT NULL,
      created_ms INTEGER NOT NULL, PRIMARY KEY(org,id));
    CREATE TABLE IF NOT EXISTS finance_credits (
      org TEXT NOT NULL, id TEXT NOT NULL, amount_minor INTEGER NOT NULL, reason TEXT NOT NULL,
      correction_of TEXT, actor TEXT NOT NULL, created_ms INTEGER NOT NULL, PRIMARY KEY(org,id));
    CREATE TABLE IF NOT EXISTS finance_invoices (
      org TEXT NOT NULL, id TEXT NOT NULL, request_id TEXT NOT NULL, currency TEXT NOT NULL,
      total_minor INTEGER NOT NULL, document TEXT NOT NULL, payment_state TEXT NOT NULL DEFAULT 'unpaid',
      correction_state TEXT NOT NULL DEFAULT 'original', provider_id TEXT, created_ms INTEGER NOT NULL,
      PRIMARY KEY(org,id), UNIQUE(org,request_id));
    CREATE TABLE IF NOT EXISTS finance_invoice_lines (
      org TEXT NOT NULL, invoice_id TEXT NOT NULL, kind TEXT NOT NULL, source_id TEXT NOT NULL,
      amount_minor INTEGER NOT NULL, PRIMARY KEY(org,kind,source_id));
    CREATE TABLE IF NOT EXISTS finance_outbox (
      org TEXT NOT NULL, id TEXT NOT NULL, invoice_id TEXT NOT NULL, step TEXT NOT NULL,
      method TEXT NOT NULL, path TEXT NOT NULL, body TEXT NOT NULL, state TEXT NOT NULL,
      provider_id TEXT, response TEXT, created_ms INTEGER NOT NULL,
      PRIMARY KEY(org,id), UNIQUE(org,invoice_id,step));
    CREATE TABLE IF NOT EXISTS finance_inbox (
      org TEXT NOT NULL, event_id TEXT NOT NULL, fingerprint TEXT NOT NULL, event_type TEXT NOT NULL,
      provider_invoice_id TEXT NOT NULL, created_ms INTEGER NOT NULL, PRIMARY KEY(org,event_id));
    CREATE INDEX IF NOT EXISTS finance_reservation_state ON finance_reservations(org,state);")
        .map_err(sql)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub class: String,
    pub currency: String,
    pub unit_minor: u64,
    pub unit_ms: u64,
    pub max_units: u64,
}
pub fn trusted_price(db: &Connection, org: &str, id: &str) -> Result<Price> {
    db.query_row("SELECT class,currency,unit_minor,unit_ms,max_units FROM finance_prices WHERE org=?1 AND id=?2", params![org,id], |r| Ok(Price { class:r.get(0)?, currency:r.get(1)?, unit_minor:crate::sqlite_u64(r,2)?, unit_ms:crate::sqlite_u64(r,3)?, max_units:crate::sqlite_u64(r,4)? }))
        .optional().map_err(sql)?.ok_or_else(|| "unknown exact admitted price".into())
}
fn account(db: &Connection, org: &str) -> Result<Value> {
    db.query_row("SELECT currency,revision,cap_minor,request_max_minor,max_concurrent,spent_minor,reserved_minor,credits_minor FROM finance_accounts WHERE org=?1", [org], |r| {
        let cap=crate::sqlite_u64(r,2)?; let spent=crate::sqlite_u64(r,5)?; let reserved=crate::sqlite_u64(r,6)?; let credits=crate::sqlite_u64(r,7)?;
        Ok(json!({"currency":r.get::<_,String>(0)?,"revision":crate::sqlite_u64(r,1)?,"cap_minor":cap,"request_max_minor":crate::sqlite_u64(r,3)?,"max_concurrent":crate::sqlite_u64(r,4)?,"spent_minor":spent,"reserved_minor":reserved,"credits_minor":credits,"available_minor":cap.saturating_add(credits).saturating_sub(spent).saturating_sub(reserved)}))
    }).optional().map_err(sql)?.ok_or_else(|| "account is not configured".into())
}
fn replay(
    db: &Connection,
    org: &str,
    request: &str,
    kind: &str,
    fingerprint: &str,
) -> Result<Option<Value>> {
    let row: Option<(String, String, String)> = db
        .query_row(
            "SELECT kind,fingerprint,result FROM finance_changes WHERE org=?1 AND request_id=?2",
            params![org, request],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    match row {
        Some((k, f, result)) if k == kind && f == fingerprint => serde_json::from_str(&result)
            .map(Some)
            .map_err(|e| e.to_string()),
        Some(_) => Err("request identity conflicts with retained financial operation".into()),
        None => Ok(None),
    }
}
fn remember(
    db: &Connection,
    a: &crate::Actor,
    request: &str,
    kind: &str,
    fingerprint: &str,
    result: &Value,
    now: u64,
) -> Result<()> {
    db.execute(
        "INSERT INTO finance_changes VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            a.organization,
            request,
            kind,
            fingerprint,
            result.to_string(),
            a.peer,
            integer(now)?
        ],
    )
    .map_err(sql)?;
    Ok(())
}
fn configure(db: &mut Connection, a: &crate::Actor, v: Value, now: u64) -> Result<Value> {
    let request = text(&v, "request_id")?;
    let expected = num(&v, "expected_revision", u32::MAX.into())?;
    let cap = num(&v, "cap_minor", MAX_MONEY)?;
    let per = num(&v, "request_max_minor", MAX_MONEY)?;
    let concurrent = num(&v, "max_concurrent", MAX_CONCURRENT)?;
    let cur = v
        .get("currency")
        .and_then(Value::as_str)
        .filter(|c| currency(c))
        .ok_or("currency must be lowercase ISO code")?;
    if per == 0 || per > cap || concurrent == 0 {
        return Err("invalid request or concurrency maximum".into());
    }
    let fingerprint = crate::identity(&v)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let current = crate::accounts::authorize(&tx, &a.peer, &a.organization)?;
    let a = &current;
    authority(a)?;
    if let Some(x) = replay(
        &tx,
        &a.organization,
        request,
        "account.configure",
        &fingerprint,
    )? {
        return Ok(x);
    }
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM finance_accounts WHERE org=?1)",
            [&a.organization],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let prior = if exists {
        Some(account(&tx, &a.organization)?)
    } else {
        None
    };
    if let Some(old) = &prior {
        if old.get("revision").unwrap_or(&Value::Null).as_u64() != Some(expected)
            || old.get("currency").unwrap_or(&Value::Null) != cur
        {
            return Err("stale account revision or currency change".into());
        }
        let obligation = add(
            old.get("spent_minor")
                .unwrap_or(&Value::Null)
                .as_u64()
                .ok_or("invalid retained unsigned integer")?,
            old.get("reserved_minor")
                .unwrap_or(&Value::Null)
                .as_u64()
                .ok_or("invalid retained unsigned integer")?,
        )?;
        if obligation
            > add(
                cap,
                old.get("credits_minor")
                    .unwrap_or(&Value::Null)
                    .as_u64()
                    .ok_or("invalid retained unsigned integer")?,
            )?
        {
            return Err("cap below retained obligations".into());
        }
        let active=tx.query_row("SELECT COUNT(*) FROM finance_reservations WHERE org=?1 AND state IN ('reserved','unknown')",[&a.organization],|r|crate::sqlite_u64(r,0)).map_err(sql)?;
        if active > concurrent {
            return Err("concurrency below retained reservations".into());
        }
        tx.execute("UPDATE finance_accounts SET revision=revision+1,cap_minor=?2,request_max_minor=?3,max_concurrent=?4 WHERE org=?1",params![a.organization,integer(cap)?,integer(per)?,integer(concurrent)?]).map_err(sql)?;
    } else {
        if expected != 0 {
            return Err("new account requires revision zero".into());
        }
        tx.execute("INSERT INTO finance_accounts(org,currency,revision,cap_minor,request_max_minor,max_concurrent) VALUES (?1,?2,1,?3,?4,?5)",params![a.organization,cur,integer(cap)?,integer(per)?,integer(concurrent)?]).map_err(sql)?;
    }
    let result = account(&tx, &a.organization)?;
    remember(
        &tx,
        a,
        request,
        "account.configure",
        &fingerprint,
        &result,
        now,
    )?;
    tx.commit().map_err(sql)?;
    Ok(result)
}
fn admit_price(db: &mut Connection, a: &crate::Actor, v: Value, now: u64) -> Result<Value> {
    let p: Price = serde_json::from_value(v).map_err(|e| e.to_string())?;
    if !crate::identifier(&p.class)
        || !currency(&p.currency)
        || p.unit_minor == 0
        || p.unit_minor > MAX_MONEY
        || p.unit_ms == 0
        || p.unit_ms > 86_400_000
        || p.max_units == 0
        || p.max_units > MAX_UNITS
        || p.unit_minor
            .checked_mul(p.max_units)
            .filter(|x| *x <= MAX_MONEY)
            .is_none()
    {
        return Err("invalid bounded integer price".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let current = crate::accounts::authorize(&tx, &a.peer, &a.organization)?;
    let a = &current;
    authority(a)?;
    let id = crate::identity(
        &json!({"schema":"kiln.hosted.price.v1","organization":a.organization,"price":p}),
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO finance_prices VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            a.organization,
            id,
            p.class,
            p.currency,
            integer(p.unit_minor)?,
            integer(p.unit_ms)?,
            integer(p.max_units)?,
            a.peer,
            integer(now)?
        ],
    )
    .map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(json!({"price_id":id,"price":p}))
}

/// Reserve before dispatch. Binding must contain the exact execution identities, not provider IDs.
pub fn reserve(db: &mut Connection, a: &crate::Actor, v: Value, now: u64) -> Result<Value> {
    let attempt = text(&v, "attempt_id")?;
    let price_id = text(&v, "price_id")?;
    let class = text(&v, "class")?;
    let units = num(&v, "max_units", MAX_UNITS)?;
    let binding = v.get("binding").cloned().unwrap_or(Value::Null);
    if binding.to_string().len() > MAX_EVIDENCE {
        return Err("execution binding too large".into());
    }
    let fingerprint = crate::identity(
        &json!({"attempt_id":attempt,"price_id":price_id,"class":class,"max_units":units,"binding":binding}),
    )?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let current = crate::accounts::authorize(&tx, &a.peer, &a.organization)?;
    let a = &current;
    crate::require(a, &["owner", "admin", "member", "billing"])?;
    if let Some(project) = binding.get("project") {
        let project = project.as_str().ok_or("execution project must be text")?;
        crate::accounts::require_project(
            &tx,
            a,
            project,
            &["owner", "admin", "member", "billing"],
        )?;
    }
    let retained: Option<String> = tx
        .query_row(
            "SELECT fingerprint FROM finance_reservations WHERE org=?1 AND attempt=?2",
            params![a.organization, attempt],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if let Some(f) = retained {
        if f != fingerprint {
            return Err("attempt conflicts with exact retained price, maximum or binding".into());
        }
        return reservation(&tx, &a.organization, attempt);
    }
    let price = trusted_price(&tx, &a.organization, price_id)?;
    if units == 0 || units > price.max_units || class != price.class {
        return Err("reservation exceeds exact price class or maximum".into());
    }
    let maximum = price
        .unit_minor
        .checked_mul(units)
        .ok_or("price overflow")?;
    let acct = account(&tx, &a.organization)?;
    if acct.get("currency").unwrap_or(&Value::Null) != &price.currency
        || maximum
            > acct
                .get("request_max_minor")
                .unwrap_or(&Value::Null)
                .as_u64()
                .ok_or("invalid retained unsigned integer")?
    {
        return Err("currency or per-request maximum denied".into());
    }
    let active=tx.query_row("SELECT COUNT(*) FROM finance_reservations WHERE org=?1 AND state IN ('reserved','unknown')",[&a.organization],|r|crate::sqlite_u64(r,0)).map_err(sql)?;
    if active
        >= acct
            .get("max_concurrent")
            .unwrap_or(&Value::Null)
            .as_u64()
            .ok_or("invalid retained unsigned integer")?
        || maximum
            > acct
                .get("available_minor")
                .unwrap_or(&Value::Null)
                .as_u64()
                .ok_or("invalid retained unsigned integer")?
    {
        return Err("account spend cap or concurrency exhausted".into());
    }
    let id = crate::identity(
        &json!({"schema":"kiln.hosted.reservation.v1","organization":a.organization,"fingerprint":fingerprint}),
    )?;
    tx.execute("INSERT INTO finance_reservations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'reserved',?10,?11,?11)",params![a.organization,attempt,id,fingerprint,price_id,class,binding.to_string(),integer(units)?,integer(maximum)?,a.peer,integer(now)?]).map_err(sql)?;
    tx.execute(
        "UPDATE finance_accounts SET reserved_minor=reserved_minor+?2 WHERE org=?1",
        params![a.organization, integer(maximum)?],
    )
    .map_err(sql)?;
    let result = reservation(&tx, &a.organization, attempt)?;
    tx.commit().map_err(sql)?;
    Ok(result)
}
fn reservation(db: &Connection, org: &str, attempt: &str) -> Result<Value> {
    db.query_row("SELECT id,price_id,class,binding,max_units,maximum_minor,state FROM finance_reservations WHERE org=?1 AND attempt=?2",params![org,attempt],|r|{
        let binding:String=r.get(3)?;
        let binding:Value=serde_json::from_str(&binding).map_err(|e|rusqlite::Error::FromSqlConversionFailure(3,rusqlite::types::Type::Text,Box::new(e)))?;
        Ok(json!({"reservation_id":r.get::<_,String>(0)?,"attempt_id":attempt,"price_id":r.get::<_,String>(1)?,"class":r.get::<_,String>(2)?,"binding":binding,"max_units":crate::sqlite_u64(r,4)?,"maximum_minor":crate::sqlite_u64(r,5)?,"state":r.get::<_,String>(6)?}))
    }).optional().map_err(sql)?.ok_or_else(||"unknown reservation".into())
}

/// Host/backend metering only. Never deserialize this from a customer settlement RPC.
#[derive(Clone, Debug, Serialize)]
pub struct Settlement {
    pub receipt_id: String,
    pub price_id: String,
    pub units: u64,
    pub execution_outcome: String,
    pub evidence: String,
}
pub fn settle_trusted(
    db: &mut Connection,
    org: &str,
    attempt: &str,
    s: &Settlement,
    now: u64,
) -> Result<Value> {
    if !crate::identifier(&s.receipt_id)
        || s.units > MAX_UNITS
        || !matches!(
            s.execution_outcome.as_str(),
            "succeeded" | "failed" | "cancelled" | "not_dispatched"
        )
        || (s.execution_outcome == "not_dispatched" && s.units != 0)
    {
        return Err("invalid trusted settlement".into());
    }
    bounded_evidence(&s.evidence)?;
    let fingerprint = crate::identity(&json!({"attempt_id":attempt,"settlement":s}))?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let prior: Option<(String, String)> = tx
        .query_row(
            "SELECT fingerprint,attempt FROM finance_usage WHERE org=?1 AND receipt=?2",
            params![org, s.receipt_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((f, a)) = prior {
        if f != fingerprint || a != attempt {
            return Err("usage receipt replay conflicts".into());
        }
        return usage_one(&tx, org, attempt);
    }
    let r = reservation(&tx, org, attempt)?;
    if r.get("state").unwrap_or(&Value::Null) == "settled" {
        return Err("attempt already settled by another receipt".into());
    }
    if r.get("price_id").unwrap_or(&Value::Null) != &s.price_id
        || s.units
            > r.get("max_units")
                .unwrap_or(&Value::Null)
                .as_u64()
                .ok_or("invalid retained unsigned integer")?
    {
        return Err(
            "settlement does not match reserved exact price or maximum; obligation retained".into(),
        );
    }
    let price = trusted_price(&tx, org, &s.price_id)?;
    let charge = s
        .units
        .checked_mul(price.unit_minor)
        .ok_or("usage amount overflow")?;
    tx.execute(
        "INSERT INTO finance_usage VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            org,
            s.receipt_id,
            attempt,
            s.price_id,
            fingerprint,
            integer(s.units)?,
            integer(charge)?,
            s.execution_outcome,
            s.evidence,
            integer(now)?
        ],
    )
    .map_err(sql)?;
    tx.execute("UPDATE finance_accounts SET reserved_minor=reserved_minor-?2,spent_minor=spent_minor+?3 WHERE org=?1 AND reserved_minor>=?2",params![org,r.get("maximum_minor").unwrap_or(&Value::Null).as_i64().ok_or("invalid retained reservation maximum")?,integer(charge)?]).map_err(sql)?;
    tx.execute(
        "UPDATE finance_reservations SET state='settled',updated_ms=?3 WHERE org=?1 AND attempt=?2",
        params![org, attempt, integer(now)?],
    )
    .map_err(sql)?;
    let result = usage_one(&tx, org, attempt)?;
    tx.commit().map_err(sql)?;
    Ok(result)
}
#[derive(Clone, Copy, Debug)]
pub struct ElapsedMeasurement<'a> {
    pub receipt_id: &'a str,
    pub elapsed_ms: u64,
    pub execution_outcome: &'a str,
    pub evidence: &'a str,
}
pub fn settle_elapsed_trusted(
    db: &mut Connection,
    org: &str,
    attempt: &str,
    measurement: &ElapsedMeasurement<'_>,
    now: u64,
) -> Result<Value> {
    let ElapsedMeasurement {
        receipt_id: receipt,
        elapsed_ms,
        execution_outcome: outcome,
        evidence,
    } = *measurement;
    if outcome == "not_dispatched" && elapsed_ms != 0 {
        return Err("not_dispatched requires zero elapsed usage".into());
    }
    let r = reservation(db, org, attempt)?;
    let price_id = r
        .get("price_id")
        .unwrap_or(&Value::Null)
        .as_str()
        .ok_or("missing reserved price")?;
    let p = trusted_price(db, org, price_id)?;
    let units = elapsed_ms / p.unit_ms + u64::from(elapsed_ms % p.unit_ms != 0);
    settle_trusted(
        db,
        org,
        attempt,
        &Settlement {
            receipt_id: receipt.into(),
            price_id: price_id.into(),
            units,
            execution_outcome: outcome.into(),
            evidence: evidence.into(),
        },
        now,
    )
}
pub fn observe_unknown_trusted(
    db: &mut Connection,
    org: &str,
    attempt: &str,
    observation: &str,
    evidence: &str,
    now: u64,
) -> Result<Value> {
    if !crate::identifier(observation) {
        return Err("invalid observation id".into());
    }
    bounded_evidence(evidence)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let prior: Option<(String, String)> = tx
        .query_row(
            "SELECT attempt,evidence FROM finance_observations WHERE org=?1 AND id=?2",
            params![org, observation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((a, e)) = prior {
        if a != attempt || e != evidence {
            return Err("observation conflict".into());
        }
        return reservation(&tx, org, attempt);
    }
    let r = reservation(&tx, org, attempt)?;
    if r.get("state").unwrap_or(&Value::Null) == "settled" {
        return Err("settled attempt cannot become unknown".into());
    }
    tx.execute(
        "INSERT INTO finance_observations VALUES (?1,?2,?3,?4,?5)",
        params![org, observation, attempt, evidence, integer(now)?],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE finance_reservations SET state='unknown',updated_ms=?3 WHERE org=?1 AND attempt=?2",
        params![org, attempt, integer(now)?],
    )
    .map_err(sql)?;
    let result = reservation(&tx, org, attempt)?;
    tx.commit().map_err(sql)?;
    Ok(result)
}
fn usage_one(db: &Connection, org: &str, attempt: &str) -> Result<Value> {
    db.query_row("SELECT receipt,price_id,units,amount_minor,execution_outcome,evidence FROM finance_usage WHERE org=?1 AND attempt=?2",params![org,attempt],|r|Ok(json!({"receipt_id":r.get::<_,String>(0)?,"attempt_id":attempt,"price_id":r.get::<_,String>(1)?,"units":crate::sqlite_u64(r,2)?,"amount_minor":crate::sqlite_u64(r,3)?,"execution_outcome":r.get::<_,String>(4)?,"evidence":r.get::<_,String>(5)?}))).map_err(sql)
}
fn credit(db: &mut Connection, a: &crate::Actor, v: Value, now: u64) -> Result<Value> {
    let id = text(&v, "request_id")?;
    let amount = num(&v, "amount_minor", MAX_MONEY)?;
    let reason = v
        .get("reason")
        .and_then(Value::as_str)
        .ok_or("credit reason required")?;
    bounded_evidence(reason)?;
    if amount == 0 {
        return Err("zero credit denied".into());
    }
    let correction = v.get("correction_of").and_then(Value::as_str);
    let fingerprint = crate::identity(&v)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let current = crate::accounts::authorize(&tx, &a.peer, &a.organization)?;
    let a = &current;
    authority(a)?;
    if let Some(result) = replay(&tx, &a.organization, id, "credit.issue", &fingerprint)? {
        return Ok(result);
    }
    let acct = account(&tx, &a.organization)?;
    if add(
        acct.get("credits_minor")
            .unwrap_or(&Value::Null)
            .as_u64()
            .ok_or("invalid retained unsigned integer")?,
        amount,
    )? > MAX_MONEY
    {
        return Err("credit ledger exceeds finite maximum".into());
    }
    if let Some(invoice) = correction {
        if !crate::identifier(invoice) {
            return Err("invalid correction invoice".into());
        }
        let changed = tx
            .execute(
                "UPDATE finance_invoices SET correction_state='credited' WHERE org=?1 AND id=?2",
                params![a.organization, invoice],
            )
            .map_err(sql)?;
        if changed != 1 {
            return Err("correction invoice not found".into());
        }
    }
    tx.execute(
        "INSERT INTO finance_credits VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            a.organization,
            id,
            integer(amount)?,
            reason,
            correction,
            a.peer,
            integer(now)?
        ],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE finance_accounts SET credits_minor=credits_minor+?2 WHERE org=?1",
        params![a.organization, integer(amount)?],
    )
    .map_err(sql)?;
    let result = json!({"credit_id":id,"amount_minor":amount,"correction_of":correction,"state":"issued","payment_state":"not_a_payment"});
    remember(&tx, a, id, "credit.issue", &fingerprint, &result, now)?;
    tx.commit().map_err(sql)?;
    Ok(result)
}
fn draft(db: &mut Connection, a: &crate::Actor, v: Value, now: u64) -> Result<Value> {
    let request = text(&v, "request_id")?;
    let fingerprint = crate::identity(&v)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let current = crate::accounts::authorize(&tx, &a.peer, &a.organization)?;
    let a = &current;
    authority(a)?;
    if let Some(result) = replay(&tx, &a.organization, request, "invoice.draft", &fingerprint)? {
        return Ok(result);
    }
    let acct = account(&tx, &a.organization)?;
    let mut lines: Vec<Value> = Vec::new();
    {
        let mut stmt=tx.prepare("SELECT 'usage',receipt,amount_minor FROM finance_usage u WHERE org=?1 AND NOT EXISTS (SELECT 1 FROM finance_invoice_lines l WHERE l.org=u.org AND l.kind='usage' AND l.source_id=u.receipt) UNION ALL SELECT 'credit',id,-amount_minor FROM finance_credits c WHERE org=?1 AND NOT EXISTS (SELECT 1 FROM finance_invoice_lines l WHERE l.org=c.org AND l.kind='credit' AND l.source_id=c.id) ORDER BY 1,2 LIMIT ?2").map_err(sql)?;
        let rows=stmt.query_map(params![a.organization,MAX_ROWS as i64],|r|Ok(json!({"kind":r.get::<_,String>(0)?,"source_id":r.get::<_,String>(1)?,"amount_minor":r.get::<_,i64>(2)?}))).map_err(sql)?;
        for row in rows {
            lines.push(row.map_err(sql)?);
        }
    }
    if lines.is_empty() {
        return Err("no uninvoiced ledger entries".into());
    }
    let total = lines.iter().try_fold(0i64, |n, l| {
        n.checked_add(
            l.get("amount_minor")
                .unwrap_or(&Value::Null)
                .as_i64()
                .ok_or("invalid retained signed integer")?,
        )
        .ok_or("invoice integer overflow")
    })?;
    let id = crate::identity(
        &json!({"schema":"kiln.hosted.invoice.v1","organization":a.organization,"request_id":request,"currency":acct.get("currency").unwrap_or(&Value::Null),"lines":lines}),
    )?;
    let document = json!({"schema":"kiln.hosted.invoice.v1","invoice_id":id,"organization":a.organization,"currency":acct.get("currency").unwrap_or(&Value::Null),"lines":lines,"total_minor":total,"created_ms":now,"state":"draft","collection":"not_attempted"});
    tx.execute("INSERT INTO finance_invoices(org,id,request_id,currency,total_minor,document,created_ms) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![a.organization,id,request,acct.get("currency").unwrap_or(&Value::Null).as_str().ok_or("invalid retained currency")?,total,document.to_string(),integer(now)?]).map_err(sql)?;
    for line in &lines {
        tx.execute(
            "INSERT INTO finance_invoice_lines VALUES (?1,?2,?3,?4,?5)",
            params![
                a.organization,
                id,
                line.get("kind")
                    .unwrap_or(&Value::Null)
                    .as_str()
                    .ok_or("invalid retained line kind")?,
                line.get("source_id")
                    .unwrap_or(&Value::Null)
                    .as_str()
                    .ok_or("invalid retained source id")?,
                line.get("amount_minor")
                    .unwrap_or(&Value::Null)
                    .as_i64()
                    .ok_or("invalid retained amount")?
            ],
        )
        .map_err(sql)?;
    }
    remember(
        &tx,
        a,
        request,
        "invoice.draft",
        &fingerprint,
        &document,
        now,
    )?;
    tx.commit().map_err(sql)?;
    Ok(document)
}
fn invoice(db: &Connection, org: &str, id: &str) -> Result<Value> {
    db.query_row("SELECT document,payment_state,correction_state,provider_id FROM finance_invoices WHERE org=?1 AND id=?2",params![org,id],|r| {
        let document:String=r.get(0)?;
        let document:Value=serde_json::from_str(&document).map_err(|e|rusqlite::Error::FromSqlConversionFailure(0,rusqlite::types::Type::Text,Box::new(e)))?;
        Ok(json!({"document":document,"payment_state":r.get::<_,String>(1)?,"correction_state":r.get::<_,String>(2)?,"provider_invoice_id":r.get::<_,Option<String>>(3)?}))
    }).optional().map_err(sql)?.ok_or_else(||"invoice not found".into())
}
fn reconcile(db: &Connection, org: &str) -> Result<Value> {
    let acct = account(db, org)?;
    let reserved:i64=db.query_row("SELECT COALESCE(SUM(maximum_minor),0) FROM finance_reservations WHERE org=?1 AND state IN ('reserved','unknown')",[org],|r|r.get(0)).map_err(sql)?;
    let spent: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(amount_minor),0) FROM finance_usage WHERE org=?1",
            [org],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let credits: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(amount_minor),0) FROM finance_credits WHERE org=?1",
            [org],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if acct.get("reserved_minor").unwrap_or(&Value::Null).as_i64() != Some(reserved)
        || acct.get("spent_minor").unwrap_or(&Value::Null).as_i64() != Some(spent)
        || acct.get("credits_minor").unwrap_or(&Value::Null).as_i64() != Some(credits)
    {
        return Err("financial ledger reconciliation mismatch; admission must be held".into());
    }
    let unknown = db
        .query_row(
            "SELECT COUNT(*) FROM finance_reservations WHERE org=?1 AND state='unknown'",
            [org],
            |r| crate::sqlite_u64(r, 0),
        )
        .map_err(sql)?;
    Ok(
        json!({"state":"matched","account":acct,"unknown_obligations":unknown,"collection":"not_implied"}),
    )
}

pub fn handle(
    db: &mut Connection,
    a: &crate::Actor,
    action: &str,
    v: Value,
    now: u64,
) -> Result<Value> {
    if v.to_string().len() > 65536 {
        return Err("finance input bound exceeded".into());
    }
    let current = crate::accounts::authorize(db, &a.peer, &a.organization)?;
    let a = &current;
    match action {
        "price.admit" => admit_price(db, a, v, now),
        "account.configure" => configure(db, a, v, now),
        "reserve" => reserve(db, a, v, now),
        "account.get" => {
            crate::require(a, &["owner", "admin", "billing", "member", "viewer"])?;
            account(db, &a.organization)
        }
        "reservation.get" => {
            crate::require(a, &["owner", "admin", "billing", "member", "viewer"])?;
            let result = reservation(db, &a.organization, text(&v, "attempt_id")?)?;
            if let Some(project) = result.get("binding").unwrap_or(&Value::Null).get("project") {
                crate::accounts::require_project(
                    db,
                    a,
                    project.as_str().ok_or("invalid retained project")?,
                    &["owner", "admin", "billing", "member", "viewer"],
                )?;
            }
            Ok(result)
        }
        "usage.list" => {
            authority(a)?;
            let after = v.get("after").and_then(Value::as_str).unwrap_or("");
            if after.len() > 256 {
                return Err("invalid usage cursor".into());
            }
            let limit = v
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100)
                .clamp(1, MAX_ROWS as u64);
            let mut stmt=db.prepare("SELECT attempt FROM finance_usage WHERE org=?1 AND attempt>?2 ORDER BY attempt LIMIT ?3").map_err(sql)?;
            let attempts = stmt
                .query_map(params![a.organization, after, integer(limit)?], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(sql)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(sql)?;
            let mut rows = Vec::new();
            for attempt in attempts {
                rows.push(usage_one(db, &a.organization, &attempt)?);
            }
            Ok(
                json!({"usage":rows,"next_after":rows.last().and_then(|r|r.get("attempt_id").unwrap_or(&Value::Null).as_str())}),
            )
        }
        "credit.issue" => credit(db, a, v, now),
        "invoice.draft" => draft(db, a, v, now),
        "invoice.get" | "invoice.export" => {
            authority(a)?;
            invoice(db, &a.organization, text(&v, "invoice_id")?)
        }
        "reconcile" => {
            authority(a)?;
            reconcile(db, &a.organization)
        }
        "outbox.list" => {
            authority(a)?;
            let after = v.get("after").and_then(Value::as_str).unwrap_or("");
            if after.len() > 256 {
                return Err("invalid outbox cursor".into());
            }
            let mut stmt=db.prepare("SELECT id,invoice_id,step,state,provider_id FROM finance_outbox WHERE org=?1 AND id>?2 ORDER BY id LIMIT ?3").map_err(sql)?;
            let rows=stmt.query_map(params![a.organization,after,MAX_ROWS as i64],|r|Ok(json!({"outbox_id":r.get::<_,String>(0)?,"invoice_id":r.get::<_,String>(1)?,"step":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?,"provider_id":r.get::<_,Option<String>>(4)?}))).map_err(sql)?.collect::<std::result::Result<Vec<_>,_>>().map_err(sql)?;
            Ok(json!({"outbox":rows}))
        }
        _ => Err(
            "unsupported finance action; customer-reported usage and payment states are forbidden"
                .into(),
        ),
    }
}

/// Explicit server-side sandbox configuration. Never accepted from customer RPC or logged.
pub struct SandboxConfig {
    pub endpoint: String,
    pub secret_key: String,
    pub webhook_secret: String,
    pub customer_id: String,
}
impl std::fmt::Debug for SandboxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxConfig")
            .field("endpoint", &self.endpoint)
            .field("customer_id", &"[redacted]")
            .field("secret_key", &"[redacted]")
            .field("webhook_secret", &"[redacted]")
            .finish()
    }
}
impl SandboxConfig {
    fn validate(&self) -> Result<()> {
        let token =
            |s: &str| s.len() <= 256 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if self.endpoint != STRIPE_ENDPOINT
            || !self.secret_key.starts_with("sk_test_")
            || self.secret_key.len() < 16
            || !token(&self.secret_key)
            || !self.webhook_secret.starts_with("whsec_")
            || !token(&self.webhook_secret)
            || !self.customer_id.starts_with("cus_")
            || !token(&self.customer_id)
            || self.customer_id.len() > 128
        {
            return Err(
                "explicit pinned Stripe sandbox endpoint/test credentials/customer are required"
                    .into(),
            );
        }
        Ok(())
    }
}
impl Drop for SandboxConfig {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.secret_key.zeroize();
        self.webhook_secret.zeroize();
    }
}
/// A real provider protocol request, not evidence that it has been sent or paid.
/// The embedding shell must obtain exact point-of-risk user approval before transmitting.
pub struct ProviderRequest {
    pub outbox_id: String,
    pub method: String,
    pub url: String,
    pub authorization: String,
    pub stripe_version: &'static str,
    pub idempotency_key: String,
    pub content_type: &'static str,
    pub body: String,
}
impl std::fmt::Debug for ProviderRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRequest")
            .field("outbox_id", &self.outbox_id)
            .field("method", &self.method)
            .field("url", &self.url)
            .field("stripe_version", &self.stripe_version)
            .field("authorization", &"[redacted]")
            .field("body", &"[redacted]")
            .finish_non_exhaustive()
    }
}
impl Drop for ProviderRequest {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.authorization.zeroize();
    }
}
fn form(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            result.push(char::from(b));
        } else {
            result.push_str(&format!("%{b:02X}"));
        }
    }
    result
}
/// Prepare create-draft, then one invoice-item request per immutable line. No collection/finalization.
pub fn prepare_stripe_request(
    db: &mut Connection,
    org: &str,
    invoice_id: &str,
    c: &SandboxConfig,
    now: u64,
) -> Result<ProviderRequest> {
    c.validate()?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let inv = invoice(&tx, org, invoice_id)?;
    let doc = inv
        .get("document")
        .ok_or("missing retained invoice document")?;
    if doc
        .get("total_minor")
        .unwrap_or(&Value::Null)
        .as_i64()
        .unwrap_or(-1)
        < 0
    {
        return Err("negative draft requires provider credit-note workflow; no unsupported financial effect prepared".into());
    }
    let provider = inv
        .get("provider_invoice_id")
        .unwrap_or(&Value::Null)
        .as_str();
    let (step, path, body) = if let Some(provider_id) = provider {
        let pinned: String = tx
            .query_row(
                "SELECT body FROM finance_outbox WHERE org=?1 AND invoice_id=?2 AND step='create'",
                params![org, invoice_id],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if !pinned.starts_with(&format!("customer={}&", form(&c.customer_id))) {
            return Err("sandbox customer differs from retained provider binding".into());
        }
        let lines = doc
            .get("lines")
            .unwrap_or(&Value::Null)
            .as_array()
            .ok_or("invalid retained invoice lines")?;
        let mut next = None;
        for (index, line) in lines.iter().enumerate() {
            let step = format!("line-{index}");
            let state: Option<String> = tx
                .query_row(
                    "SELECT state FROM finance_outbox WHERE org=?1 AND invoice_id=?2 AND step=?3",
                    params![org, invoice_id, step],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            if state.as_deref() != Some("confirmed") {
                let body = format!(
                    "customer={}&invoice={}&amount={}&currency={}&description={}",
                    form(&c.customer_id),
                    form(provider_id),
                    line.get("amount_minor")
                        .unwrap_or(&Value::Null)
                        .as_i64()
                        .ok_or("invalid line amount")?,
                    form(
                        doc.get("currency")
                            .unwrap_or(&Value::Null)
                            .as_str()
                            .ok_or("missing currency")?
                    ),
                    form(&format!(
                        "Kiln {} {}",
                        line.get("kind")
                            .unwrap_or(&Value::Null)
                            .as_str()
                            .unwrap_or(""),
                        line.get("source_id")
                            .unwrap_or(&Value::Null)
                            .as_str()
                            .unwrap_or("")
                    ))
                );
                next = Some((step, "/invoiceitems".to_owned(), body));
                break;
            }
        }
        next.ok_or("all sandbox draft lines confirmed; finalization/collection is not authorized")?
    } else {
        (
            "create".to_owned(),
            "/invoices".to_owned(),
            format!(
                "customer={}&auto_advance=false&collection_method=send_invoice&days_until_due=30&pending_invoice_items_behavior=exclude&metadata%5Bkiln_invoice_id%5D={}&metadata%5Bkiln_organization%5D={}",
                form(&c.customer_id),
                form(invoice_id),
                form(org)
            ),
        )
    };
    let id = crate::identity(
        &json!({"schema":"kiln.hosted.stripe-outbox.v1","org":org,"invoice":invoice_id,"step":step,"endpoint":c.endpoint,"body":body}),
    )?;
    let prior: Option<(String, String, String)> = tx
        .query_row(
            "SELECT id,state,body FROM finance_outbox WHERE org=?1 AND invoice_id=?2 AND step=?3",
            params![org, invoice_id, step],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((old, state, old_body)) = prior {
        if old != id || old_body != body {
            return Err("provider outbox conflicts with pinned credentials/customer/body".into());
        }
        return Err(format!(
            "provider operation already {state}; reconcile its retained outbox before retry"
        ));
    }
    tx.execute("INSERT INTO finance_outbox(org,id,invoice_id,step,method,path,body,state,created_ms) VALUES (?1,?2,?3,?4,'POST',?5,?6,'prepared',?7)",params![org,id,invoice_id,step,path,body,integer(now)?]).map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(ProviderRequest {
        outbox_id: id.clone(),
        method: "POST".into(),
        url: format!("{}{path}", c.endpoint),
        authorization: format!("Bearer {}", c.secret_key),
        stripe_version: STRIPE_VERSION,
        idempotency_key: id,
        content_type: "application/x-www-form-urlencoded",
        body,
    })
}
/// Record the actual bounded provider response. An error/ambiguous response never implies success.
pub fn record_stripe_response(
    db: &mut Connection,
    org: &str,
    outbox_id: &str,
    status: u16,
    body: &[u8],
    _now: u64,
) -> Result<Value> {
    if body.len() > 65536 {
        return Err("provider response exceeds bound; obligation unknown".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let (invoice_id, step, state, prior): (String, String, String, Option<String>) = tx
        .query_row(
            "SELECT invoice_id,step,state,response FROM finance_outbox WHERE org=?1 AND id=?2",
            params![org, outbox_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(sql)?;
    let response =
        String::from_utf8(body.to_vec()).map_err(|_| "provider response is not UTF-8")?;
    if state == "confirmed" {
        if prior.as_deref() == Some(&response) {
            return Ok(json!({"outbox_id":outbox_id,"state":"confirmed"}));
        }
        return Err("confirmed provider response conflicts".into());
    }
    let parsed: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let provider = parsed.get("id").unwrap_or(&Value::Null).as_str();
    let expected_object = if step == "create" {
        "invoice"
    } else {
        "invoiceitem"
    };
    let inv = invoice(&tx, org, &invoice_id)?;
    let create_body: String = tx
        .query_row(
            "SELECT body FROM finance_outbox WHERE org=?1 AND invoice_id=?2 AND step='create'",
            params![org, invoice_id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let customer = create_body
        .split('&')
        .next()
        .and_then(|s| s.strip_prefix("customer="))
        .ok_or("missing pinned provider customer")?;
    let matches_binding = if step == "create" {
        parsed
            .get("metadata")
            .unwrap_or(&Value::Null)
            .get("kiln_invoice_id")
            .unwrap_or(&Value::Null)
            == &invoice_id
            && parsed.get("status").unwrap_or(&Value::Null) == "draft"
    } else {
        let index = step
            .strip_prefix("line-")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or("invalid retained provider step")?;
        let line = inv
            .get("document")
            .unwrap_or(&Value::Null)
            .get("lines")
            .unwrap_or(&Value::Null)
            .get(index)
            .ok_or("missing immutable invoice line")?;
        parsed.get("invoice").unwrap_or(&Value::Null)
            == inv.get("provider_invoice_id").unwrap_or(&Value::Null)
            && parsed.get("amount").unwrap_or(&Value::Null)
                == line.get("amount_minor").unwrap_or(&Value::Null)
            && parsed.get("currency").unwrap_or(&Value::Null)
                == inv
                    .get("document")
                    .unwrap_or(&Value::Null)
                    .get("currency")
                    .unwrap_or(&Value::Null)
    };
    let valid = (200..300).contains(&status)
        && parsed.get("object").unwrap_or(&Value::Null) == expected_object
        && parsed.get("livemode").unwrap_or(&Value::Null) == false
        && parsed.get("customer").unwrap_or(&Value::Null) == customer
        && matches_binding
        && provider.is_some_and(|p| {
            p.len() <= 128
                && crate::identifier(p)
                && p.starts_with(if step == "create" { "in_" } else { "ii_" })
        });
    tx.execute(
        "UPDATE finance_outbox SET state=?3,provider_id=?4,response=?5 WHERE org=?1 AND id=?2",
        params![
            org,
            outbox_id,
            if valid { "confirmed" } else { "unknown" },
            provider,
            response
        ],
    )
    .map_err(sql)?;
    if valid && step == "create" {
        tx.execute("UPDATE finance_invoices SET provider_id=?3 WHERE org=?1 AND id=?2 AND provider_id IS NULL",params![org,invoice_id,provider]).map_err(sql)?;
    }
    tx.commit().map_err(sql)?;
    if !valid {
        return Err(
            "provider response is uncertain/rejected; outbox retained, reconcile before retry"
                .into(),
        );
    }
    Ok(json!({"outbox_id":outbox_id,"state":"confirmed","provider_id":provider}))
}
/// Verify Stripe's native HMAC/timestamp envelope before durable consume-once event admission.
pub fn accept_stripe_webhook(
    db: &mut Connection,
    org: &str,
    c: &SandboxConfig,
    signature: &str,
    body: &[u8],
    now_ms: u64,
) -> Result<Value> {
    c.validate()?;
    if body.len() > 65536 || signature.len() > 4096 {
        return Err("webhook bound exceeded".into());
    }
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in signature.split(',') {
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(
                t.parse::<u64>()
                    .map_err(|_| "invalid signature timestamp")?,
            );
        } else if let Some(s) = part.strip_prefix("v1=") {
            signatures.push(s);
        }
    }
    let timestamp = timestamp.ok_or("missing signature timestamp")?;
    if (now_ms / 1000).abs_diff(timestamp) > WEBHOOK_TOLERANCE_SECONDS {
        return Err("stale webhook signature".into());
    }
    let mut verified = false;
    for hex in signatures {
        if hex.len() != 64 {
            continue;
        }
        let mut bytes = [0u8; 32];
        let mut valid = true;
        for (byte, chunk) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            let digit = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            let mut pair = chunk.iter().copied();
            if let (Some(a), Some(b)) = (pair.next().and_then(digit), pair.next().and_then(digit)) {
                *byte = a * 16 + b;
            } else {
                valid = false;
                break;
            }
        }
        if !valid {
            continue;
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(c.webhook_secret.as_bytes())
            .map_err(|_| "invalid signing secret")?;
        mac.update(format!("{timestamp}.").as_bytes());
        mac.update(body);
        if mac.verify_slice(&bytes).is_ok() {
            verified = true;
            break;
        }
    }
    if !verified {
        return Err("invalid webhook signature".into());
    }
    let event: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let event_id = event
        .get("id")
        .unwrap_or(&Value::Null)
        .as_str()
        .filter(|s| s.starts_with("evt_") && s.len() <= 128)
        .ok_or("invalid provider event ID")?;
    let kind = event
        .get("type")
        .unwrap_or(&Value::Null)
        .as_str()
        .ok_or("missing event type")?;
    let object = event
        .get("data")
        .unwrap_or(&Value::Null)
        .get("object")
        .unwrap_or(&Value::Null);
    let provider_id = object
        .get("id")
        .unwrap_or(&Value::Null)
        .as_str()
        .ok_or("missing provider invoice ID")?;
    if event.get("livemode").unwrap_or(&Value::Null) != false
        || object.get("object").unwrap_or(&Value::Null) != "invoice"
        || object.get("customer").unwrap_or(&Value::Null) != &c.customer_id
    {
        return Err("webhook is not the pinned sandbox customer invoice".into());
    }
    let fingerprint = blake3::hash(body).to_hex().to_string();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let prior: Option<String> = tx
        .query_row(
            "SELECT fingerprint FROM finance_inbox WHERE org=?1 AND event_id=?2",
            params![org, event_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if let Some(p) = prior {
        if p != fingerprint {
            return Err("provider event ID conflicts".into());
        }
        return Ok(json!({"event_id":event_id,"state":"duplicate"}));
    }
    let row: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT id,total_minor,currency FROM finance_invoices WHERE org=?1 AND provider_id=?2",
            params![org, provider_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let (invoice_id, total, cur) =
        row.ok_or("webhook invoice is not bound to this organization")?;
    if object.get("currency").unwrap_or(&Value::Null) != &cur
        || object.get("total").unwrap_or(&Value::Null).as_i64() != Some(total)
    {
        return Err("provider invoice differs from immutable local draft".into());
    }
    let new_state = match kind {
        "invoice.paid"
            if object.get("paid").unwrap_or(&Value::Null) == true
                && object.get("amount_paid").unwrap_or(&Value::Null).as_i64() == Some(total) =>
        {
            "paid"
        }
        "invoice.payment_failed" => "failed",
        "invoice.voided" => "void",
        "invoice.finalized" => "open",
        _ => return Err("unsupported or inconsistent sandbox invoice event".into()),
    };
    tx.execute(
        "INSERT INTO finance_inbox VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            org,
            event_id,
            fingerprint,
            kind,
            provider_id,
            integer(now_ms)?
        ],
    )
    .map_err(sql)?;
    // Delayed failure/finalization may not roll a paid/void invoice backwards.
    tx.execute("UPDATE finance_invoices SET payment_state=?3 WHERE org=?1 AND id=?2 AND (payment_state NOT IN ('paid','void') OR payment_state=?3)",params![org,invoice_id,new_state]).map_err(sql)?;
    let result = invoice(&tx, org, &invoice_id)?;
    tx.commit().map_err(sql)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor(role: &str) -> crate::Actor {
        let c = match role {
            "owner" => "a",
            "member" => "b",
            "billing" => "c",
            "viewer" => "d",
            _ => "e",
        };
        crate::Actor {
            peer: c.repeat(64),
            organization: "org".into(),
            role: role.into(),
        }
    }
    fn setup(db: &mut Connection, cap: u64, concurrency: u64) -> String {
        init(db).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let a = actor("owner");
        crate::accounts::init(db)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        crate::accounts::bootstrap(db, &a.peer, "org")
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        db.execute(
            "INSERT INTO hosted_memberships VALUES('org',?1,'member','[\"p\"]',1,1,?2)",
            params![actor("member").peer, a.peer],
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        configure(db,&a,json!({"request_id":"config","expected_revision":0,"currency":"usd","cap_minor":cap,"request_max_minor":cap,"max_concurrent":concurrency}),1).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        admit_price(
            db,
            &a,
            json!({"class":"linux","currency":"usd","unit_minor":10,"unit_ms":1000,"max_units":10}),
            1,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"))
        .get("price_id")
        .unwrap_or(&Value::Null)
        .as_str()
        .unwrap_or_else(|| unreachable!("test value must be text"))
        .into()
    }
    fn request(attempt: &str, price: &str, units: u64) -> Value {
        json!({"attempt_id":attempt,"price_id":price,"class":"linux","max_units":units,"binding":{"project":"p","pipeline":"plan","job":"build"}})
    }
    #[test]
    fn exact_reservation_unknown_restart_and_cap() {
        let file = tempfile::NamedTempFile::new()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let path = file.path();
        let mut db = Connection::open(path)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let price = setup(&mut db, 100, 2);
        let input = request("a", &price, 6);
        let first = reserve(&mut db, &actor("member"), input.clone(), 2)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            reserve(&mut db, &actor("member"), input, 3)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            first
        );
        assert!(reserve(&mut db, &actor("member"), request("a", &price, 5), 3).is_err());
        assert!(reserve(&mut db, &actor("member"), request("b", &price, 5), 3).is_err());
        observe_unknown_trusted(&mut db, "org", "a", "unknown", "worker disconnected", 4)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        drop(db);
        let mut db = Connection::open(path)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("reserved_minor")
                .unwrap_or(&Value::Null),
            60
        );
        assert!(reserve(&mut db, &actor("member"), request("b", &price, 5), 5).is_err());
        let receipt = settle_elapsed_trusted(
            &mut db,
            "org",
            "a",
            &ElapsedMeasurement {
                receipt_id: "receipt",
                elapsed_ms: 1100,
                execution_outcome: "failed",
                evidence: "trusted host clock",
            },
            6,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(receipt.get("amount_minor").unwrap_or(&Value::Null), 20);
        assert_eq!(
            settle_elapsed_trusted(
                &mut db,
                "org",
                "a",
                &ElapsedMeasurement {
                    receipt_id: "receipt",
                    elapsed_ms: 1100,
                    execution_outcome: "failed",
                    evidence: "trusted host clock"
                },
                7
            )
            .unwrap_or_else(|error| unreachable!("{error:?}")),
            receipt
        );
        assert!(
            settle_elapsed_trusted(
                &mut db,
                "org",
                "a",
                &ElapsedMeasurement {
                    receipt_id: "receipt",
                    elapsed_ms: 2100,
                    execution_outcome: "failed",
                    evidence: "trusted host clock"
                },
                7
            )
            .is_err()
        );
        assert!(
            settle_elapsed_trusted(
                &mut db,
                "org",
                "a",
                &ElapsedMeasurement {
                    receipt_id: "other",
                    elapsed_ms: 1100,
                    execution_outcome: "failed",
                    evidence: "trusted host clock"
                },
                7
            )
            .is_err()
        );
        assert_eq!(
            reconcile(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("account")
                .unwrap_or(&Value::Null)
                .get("reserved_minor")
                .unwrap_or(&Value::Null),
            0
        );
    }
    #[test]
    fn authority_exact_price_invoice_credit_and_dedup() {
        let mut db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let price = setup(&mut db, 100, 1);
        let owner = actor("owner");
        assert!(
            credit(
                &mut db,
                &actor("member"),
                json!({"request_id":"evil","amount_minor":100,"reason":"mint"}),
                2
            )
            .is_err()
        );
        assert!(handle(&mut db, &owner, "usage.settle", json!({"units":0}), 2).is_err());
        reserve(&mut db, &actor("member"), request("a", &price, 8), 2)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(reserve(&mut db, &actor("member"), request("b", &price, 1), 2).is_err());
        assert!(
            settle_trusted(
                &mut db,
                "org",
                "a",
                &Settlement {
                    receipt_id: "r".into(),
                    price_id: "different".into(),
                    units: 1,
                    execution_outcome: "succeeded".into(),
                    evidence: "host".into()
                },
                3
            )
            .is_err()
        );
        settle_elapsed_trusted(
            &mut db,
            "org",
            "a",
            &ElapsedMeasurement {
                receipt_id: "r",
                elapsed_ms: 8000,
                execution_outcome: "succeeded",
                evidence: "host",
            },
            3,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let inv = draft(&mut db, &owner, json!({"request_id":"invoice1"}), 4)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(inv.get("total_minor").unwrap_or(&Value::Null), 80);
        assert_eq!(
            draft(&mut db, &owner, json!({"request_id":"invoice1"}), 5)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            inv
        );
        assert!(draft(&mut db, &owner, json!({"request_id":"invoice2"}), 5).is_err());
        let c = json!({"request_id":"credit","amount_minor":30,"reason":"service correction","correction_of":inv.get("invoice_id").unwrap_or(&Value::Null)});
        credit(&mut db, &owner, c.clone(), 6)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        credit(&mut db, &owner, c, 7)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("credits_minor")
                .unwrap_or(&Value::Null),
            30
        );
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("available_minor")
                .unwrap_or(&Value::Null),
            50
        );
        assert_eq!(
            invoice(
                &db,
                "org",
                inv.get("invoice_id")
                    .unwrap_or(&Value::Null)
                    .as_str()
                    .unwrap_or_else(|| unreachable!("invoice ID"))
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .get("payment_state")
            .unwrap_or(&Value::Null),
            "unpaid"
        );
        assert_eq!(
            draft(
                &mut db,
                &owner,
                json!({"request_id":"correction-invoice"}),
                8
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .get("total_minor")
            .unwrap_or(&Value::Null),
            -30
        );
        let other = crate::Actor {
            organization: "other".into(),
            ..owner
        };
        assert!(
            handle(
                &mut db,
                &other,
                "invoice.get",
                json!({"invoice_id":inv.get("invoice_id").unwrap_or(&Value::Null)}),
                9
            )
            .is_err()
        );
    }
    #[test]
    fn competing_connections_cannot_oversubscribe() {
        let dir = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let path = dir.path().join("finance.sqlite");
        let mut db = Connection::open(&path)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let price = setup(&mut db, 100, 10);
        drop(db);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut threads = Vec::new();
        for n in 0..2 {
            let path = path.clone();
            let price = price.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                let mut db = Connection::open(path).unwrap_or_else(|error| {
                    unreachable!("unexpected finance test failure: {error:?}")
                });
                db.busy_timeout(std::time::Duration::from_secs(5))
                    .unwrap_or_else(|error| {
                        unreachable!("unexpected finance test failure: {error:?}")
                    });
                barrier.wait();
                reserve(
                    &mut db,
                    &actor("member"),
                    request(&format!("a{n}"), &price, 6),
                    2,
                )
                .is_ok()
            }));
        }
        barrier.wait();
        assert_eq!(
            threads
                .into_iter()
                .map(|t| usize::from(t.join().unwrap_or_else(|error| unreachable!("{error:?}"))))
                .sum::<usize>(),
            1
        );
        let db = Connection::open(path)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("reserved_minor")
                .unwrap_or(&Value::Null),
            60
        );
    }
    #[test]
    fn sandbox_outbox_and_signed_inbox_do_not_conflate_draft_and_payment() {
        let mut db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let price = setup(&mut db, 100, 1);
        reserve(&mut db, &actor("member"), request("a", &price, 1), 2)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        settle_elapsed_trusted(
            &mut db,
            "org",
            "a",
            &ElapsedMeasurement {
                receipt_id: "r",
                elapsed_ms: 1000,
                execution_outcome: "succeeded",
                evidence: "host",
            },
            3,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let inv = draft(&mut db, &actor("owner"), json!({"request_id":"inv"}), 4)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let id = inv
            .get("invoice_id")
            .unwrap_or(&Value::Null)
            .as_str()
            .unwrap_or_else(|| unreachable!("test value must be text"));
        let c = SandboxConfig {
            endpoint: STRIPE_ENDPOINT.into(),
            secret_key: "sk_test_not_a_live_credential".into(),
            webhook_secret: "whsec_local_protocol_test".into(),
            customer_id: "cus_local".into(),
        };
        let request = prepare_stripe_request(&mut db, "org", id, &c, 5)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(request.body.contains("auto_advance=false"));
        let config_debug = format!("{c:?}");
        let request_debug = format!("{request:?}");
        for secret in [&c.secret_key, &c.webhook_secret, &c.customer_id] {
            assert!(!config_debug.contains(secret));
        }
        assert!(!request_debug.contains(&request.authorization));
        assert!(!request_debug.contains(&request.body));
        assert!(prepare_stripe_request(&mut db, "org", id, &c, 5).is_err());
        let create=serde_json::to_vec(&json!({"id":"in_local","object":"invoice","livemode":false,"customer":"cus_local","status":"draft","metadata":{"kiln_invoice_id":id}})).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        record_stripe_response(&mut db, "org", &request.outbox_id, 200, &create, 6)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let line = prepare_stripe_request(&mut db, "org", id, &c, 7)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(line.body.contains("amount=10"));
        record_stripe_response(&mut db,"org",&line.outbox_id,200,br#"{"id":"ii_local","object":"invoiceitem","livemode":false,"customer":"cus_local","invoice":"in_local","amount":10,"currency":"usd"}"#,8).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            invoice(&db, "org", id)
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("payment_state")
                .unwrap_or(&Value::Null),
            "unpaid"
        );
        let event=serde_json::to_vec(&json!({"id":"evt_local","livemode":false,"type":"invoice.paid","data":{"object":{"id":"in_local","object":"invoice","customer":"cus_local","currency":"usd","total":10,"amount_paid":10,"paid":true}}})).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(accept_stripe_webhook(&mut db, "org", &c, "t=10,v1=bad", &event, 10000).is_err());
        let mut mac = Hmac::<Sha256>::new_from_slice(c.webhook_secret.as_bytes())
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        mac.update(b"10.");
        mac.update(&event);
        let hex = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let signature = format!("t=10,v1={hex}");
        assert_eq!(
            accept_stripe_webhook(&mut db, "org", &c, &signature, &event, 10000)
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("payment_state")
                .unwrap_or(&Value::Null),
            "paid"
        );
        assert_eq!(
            accept_stripe_webhook(&mut db, "org", &c, &signature, &event, 10000)
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("state")
                .unwrap_or(&Value::Null),
            "duplicate"
        );
        assert!(accept_stripe_webhook(&mut db, "org", &c, &signature, &event, 400000).is_err());
    }
    #[test]
    fn finite_request_limits_stale_caps_and_current_project_scope() {
        let mut db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        let price = setup(&mut db, 100, 2);
        let owner = actor("owner");
        let config = json!({"request_id":"limit","expected_revision":1,"currency":"usd","cap_minor":100,"request_max_minor":20,"max_concurrent":2});
        let limited = configure(&mut db, &owner, config.clone(), 2)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            configure(&mut db, &owner, config, 3).unwrap_or_else(|error| unreachable!("{error:?}")),
            limited
        );
        assert!(configure(&mut db,&owner,json!({"request_id":"stale","expected_revision":1,"currency":"usd","cap_minor":100,"request_max_minor":20,"max_concurrent":2}),3).is_err());
        assert!(
            reserve(
                &mut db,
                &actor("member"),
                request("too-large", &price, 3),
                3
            )
            .is_err()
        );
        assert!(reserve(&mut db, &actor("viewer"), request("viewer", &price, 1), 3).is_err());
        assert!(reserve(&mut db, &actor("member"), request("zero", &price, 0), 3).is_err());
        let mut wrong = request("wrong-project", &price, 1);
        *wrong
            .pointer_mut("/binding/project")
            .unwrap_or_else(|| unreachable!("binding project fixture")) = json!("forbidden");
        assert!(reserve(&mut db, &actor("member"), wrong, 3).is_err());
        reserve(&mut db, &actor("member"), request("good", &price, 2), 3)
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(configure(&mut db,&owner,json!({"request_id":"below-obligation","expected_revision":2,"currency":"usd","cap_minor":10,"request_max_minor":10,"max_concurrent":2}),4).is_err());
        assert!(
            settle_elapsed_trusted(
                &mut db,
                "org",
                "good",
                &ElapsedMeasurement {
                    receipt_id: "over-max",
                    elapsed_ms: 3000,
                    execution_outcome: "succeeded",
                    evidence: "trusted host"
                },
                4
            )
            .is_err()
        );
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("reserved_minor")
                .unwrap_or(&Value::Null),
            20
        );
        db.execute("UPDATE hosted_memberships SET projects='[]',revision=2 WHERE organization='org' AND peer=?1",[actor("member").peer]).unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(
            handle(
                &mut db,
                &actor("member"),
                "reservation.get",
                json!({"attempt_id":"good"}),
                5
            )
            .is_err()
        );
        assert!(reserve(&mut db, &actor("member"), request("good", &price, 2), 5).is_err());
        settle_elapsed_trusted(
            &mut db,
            "org",
            "good",
            &ElapsedMeasurement {
                receipt_id: "not-sent",
                elapsed_ms: 0,
                execution_outcome: "not_dispatched",
                evidence: "dispatch journal proves no effect",
            },
            6,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("available_minor")
                .unwrap_or(&Value::Null),
            100
        );
    }
    #[test]
    fn credits_and_prices_reject_conflicting_or_overflowing_inputs() {
        let mut db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        setup(&mut db, 100, 1);
        let owner = actor("owner");
        assert!(admit_price(&mut db,&owner,json!({"class":"linux","currency":"usd","unit_minor":MAX_MONEY,"unit_ms":1,"max_units":2}),2).is_err());
        assert!(
            admit_price(
                &mut db,
                &actor("member"),
                json!({"class":"linux","currency":"usd","unit_minor":1,"unit_ms":1,"max_units":2}),
                2
            )
            .is_err()
        );
        credit(
            &mut db,
            &owner,
            json!({"request_id":"credit","amount_minor":10,"reason":"service correction"}),
            3,
        )
        .unwrap_or_else(|error| unreachable!("unexpected finance test failure: {error:?}"));
        assert!(
            credit(
                &mut db,
                &owner,
                json!({"request_id":"credit","amount_minor":11,"reason":"service correction"}),
                4
            )
            .is_err()
        );
        assert!(
            credit(
                &mut db,
                &owner,
                json!({"request_id":"overflow","amount_minor":MAX_MONEY,"reason":"too much"}),
                4
            )
            .is_err()
        );
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("credits_minor")
                .unwrap_or(&Value::Null),
            10
        );
        assert!(handle(&mut db, &actor("member"), "usage.list", json!({}), 4).is_err());
    }
    #[test]
    fn financial_mutations_recheck_durable_role_before_replay_or_effect() {
        let mut db = Connection::open_in_memory().unwrap_or_else(|error| unreachable!("{error:?}"));
        let price = setup(&mut db, 100, 2);
        let stale_owner = actor("owner");
        // A different retained owner permits this fixture's original owner to be downgraded.
        db.execute("UPDATE hosted_memberships SET role='owner',revision=2 WHERE organization='org' AND peer=?1",[actor("member").peer]).unwrap_or_else(|error|unreachable!("{error:?}"));
        db.execute("UPDATE hosted_memberships SET role='member',projects='[]',revision=2 WHERE organization='org' AND peer=?1",[&stale_owner.peer]).unwrap_or_else(|error|unreachable!("{error:?}"));
        assert!(configure(&mut db,&stale_owner,json!({"request_id":"config","expected_revision":0,"currency":"usd","cap_minor":100,"request_max_minor":100,"max_concurrent":2}),2).is_err());
        assert!(
            admit_price(
                &mut db,
                &stale_owner,
                json!({"class":"new","currency":"usd","unit_minor":1,"unit_ms":1,"max_units":1}),
                2
            )
            .is_err()
        );
        assert!(
            credit(
                &mut db,
                &stale_owner,
                json!({"request_id":"stale-credit","amount_minor":10,"reason":"unauthorized"}),
                2
            )
            .is_err()
        );
        assert!(
            draft(
                &mut db,
                &stale_owner,
                json!({"request_id":"stale-invoice"}),
                2
            )
            .is_err()
        );
        assert!(
            reserve(
                &mut db,
                &stale_owner,
                request("stale-project", &price, 1),
                2
            )
            .is_err()
        );
        db.execute("UPDATE hosted_memberships SET active=0,revision=3 WHERE organization='org' AND peer=?1",[&stale_owner.peer]).unwrap_or_else(|error|unreachable!("{error:?}"));
        assert!(reserve(&mut db,&stale_owner,json!({"attempt_id":"stale-unbound","price_id":price,"class":"linux","max_units":1}),3).is_err());
        assert_eq!(
            account(&db, "org")
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .get("reserved_minor")
                .unwrap_or(&Value::Null),
            0
        );
    }
}

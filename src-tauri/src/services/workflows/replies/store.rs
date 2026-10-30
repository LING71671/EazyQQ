use super::model::{BubbleReceipt, DeliveryReport, ReplyPlan};
use crate::services::db::Database;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

const LEASE_MS: i64 = 30_000;
const MAX_FULL_REPORTS: usize = 500;
const RECENT_TERMINAL_REPORTS: usize = 200;
const MAX_TOMBSTONES: usize = 50_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryBatch {
    pub batch_id: String,
    pub account_uin: String,
    pub target_type: String,
    pub target_id: String,
    pub source_message_id: String,
    pub input_snapshot: Vec<String>,
    pub authorization: String,
    pub plan: ReplyPlan,
    pub report: DeliveryReport,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn domain(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reply_delivery_batches (
        batch_id TEXT PRIMARY KEY, account_uin TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL,
        body_json TEXT NOT NULL, claimed_by TEXT, lease_until INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS reply_delivery_targets (
        account_uin TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL,
        claimed_by TEXT NOT NULL, lease_until INTEGER NOT NULL,
        PRIMARY KEY(account_uin,target_type,target_id)
    );
    CREATE TABLE IF NOT EXISTS reply_delivery_throttle (
        account_uin TEXT PRIMARY KEY, next_send_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS reply_delivery_tombstones (
        batch_id TEXT PRIMARY KEY, account_uin TEXT NOT NULL, target_type TEXT NOT NULL,
        target_id TEXT NOT NULL, plan_hash TEXT NOT NULL, terminal_status TEXT NOT NULL,
        archived_at INTEGER NOT NULL
    );")
}

pub fn is_completed_batch(db: &Database, id: &str) -> Result<bool, String> {
    db.with_connection(|conn| {
        schema(conn)?;
        let sent:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM reply_delivery_batches WHERE batch_id=?1 AND json_extract(body_json,'$.report.status')='sent') OR EXISTS(SELECT 1 FROM reply_delivery_tombstones WHERE batch_id=?1 AND terminal_status='sent')",[id],|row|row.get(0))?;
        Ok(sent)
    }).map_err(|error|error.to_string())
}

fn retain_bounded_reports(
    conn: &Connection,
    current_id: &str,
    is_new: bool,
) -> rusqlite::Result<()> {
    let full: usize = conn.query_row("SELECT COUNT(*) FROM reply_delivery_batches", [], |row| {
        row.get(0)
    })?;
    let proofs: usize = conn.query_row(
        "SELECT COUNT(*) FROM reply_delivery_tombstones",
        [],
        |row| row.get(0),
    )?;
    if is_new && proofs >= MAX_TOMBSTONES {
        return Err(domain(
            "回执归档凭据已达到50000条，停止创建新的发送批次；未完成批次仍可核对与恢复",
        ));
    }
    // Every receipt must be terminal: mixed partial or ambiguous deliveries remain intact.
    let eligible_sql="lease_until<=?1 AND json_extract(body_json,'$.report.status') IN ('sent','superseded','cancelled') AND json_array_length(body_json,'$.report.receipts')>0 AND NOT EXISTS(SELECT 1 FROM json_each(body_json,'$.report.receipts') WHERE COALESCE(json_extract(value,'$.status'),'') NOT IN ('sent','superseded','cancelled'))";
    let eligible: usize = conn.query_row(
        &format!("SELECT COUNT(*) FROM reply_delivery_batches WHERE {eligible_sql}"),
        [now_ms()],
        |row| row.get(0),
    )?;
    let available: usize = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM reply_delivery_batches WHERE {eligible_sql} AND batch_id<>?2"
        ),
        params![now_ms(), current_id],
        |row| row.get(0),
    )?;
    let required_for_cap = (full + usize::from(is_new)).saturating_sub(MAX_FULL_REPORTS);
    let desired = eligible
        .saturating_sub(RECENT_TERMINAL_REPORTS)
        .max(required_for_cap);
    let count = desired
        .min(available)
        .min(MAX_TOMBSTONES.saturating_sub(proofs));
    if is_new && count < required_for_cap {
        return Err(domain(
            "回复回执达到500条完整记录容量，未完成或未知送达记录不能自动清除",
        ));
    }
    if is_new && count < desired {
        return Err(domain("回执归档凭据剩余容量不足，停止创建新的发送批次；已有批次仍可核对与恢复"));
    }
    let oldest = {
        let mut statement=conn.prepare(&format!("SELECT body_json FROM reply_delivery_batches WHERE {eligible_sql} AND batch_id<>?2 ORDER BY json_extract(body_json,'$.updatedAt') ASC,rowid ASC LIMIT ?3"))?;
        let rows = statement.query_map(params![now_ms(), current_id, count], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for raw in oldest {
        let batch = decode(raw)?;
        let hash = batch.plan.fingerprint().map_err(|error| domain(&error))?;
        conn.execute("INSERT INTO reply_delivery_tombstones(batch_id,account_uin,target_type,target_id,plan_hash,terminal_status,archived_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![batch.batch_id,batch.account_uin,batch.target_type,batch.target_id,hash,batch.report.status,now_ms()])?;
        conn.execute(
            "DELETE FROM reply_delivery_batches WHERE batch_id=?1",
            [batch.batch_id],
        )?;
    }
    conn.execute(
        "DELETE FROM reply_delivery_targets WHERE lease_until<=?1",
        [now_ms()],
    )?;
    Ok(())
}

fn decode(value: String) -> rusqlite::Result<DeliveryBatch> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn encode(value: &DeliveryBatch) -> rusqlite::Result<String> {
    serde_json::to_string(value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn read(conn: &Connection, id: &str) -> rusqlite::Result<Option<DeliveryBatch>> {
    conn.query_row(
        "SELECT body_json FROM reply_delivery_batches WHERE batch_id=?1",
        [id],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .map(decode)
    .transpose()
}

pub(crate) fn get(db: &Database, id: &str) -> Result<Option<DeliveryBatch>, String> {
    recover_interrupted(db)?;
    db.with_connection(|conn| {
        schema(conn)?;
        read(conn, id)
    })
    .map_err(|error| error.to_string())
}

fn recover_interrupted(db: &Database) -> Result<(), String> {
    db.with_connection(|conn| {
        schema(conn)?;
        let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let expired={let mut statement=tx.prepare("SELECT body_json FROM reply_delivery_batches WHERE lease_until<=?1 AND EXISTS(SELECT 1 FROM json_each(body_json,'$.report.receipts') WHERE json_extract(value,'$.status')='sending')")?;
            let rows=statement.query_map([now_ms()],|row|row.get::<_,String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?};
        for raw in expired {
            let mut batch=decode(raw)?;let mut changed=false;
            for receipt in &mut batch.report.receipts {if receipt.status=="sending" {receipt.status="unknown".into();receipt.error=Some("上次发送中断，送达状态未知；核对后才能继续".into());changed=true;}}
            if changed {refresh_status(&mut batch.report);batch.updated_at=now_ms();tx.execute("UPDATE reply_delivery_batches SET body_json=?1,claimed_by=NULL,lease_until=0 WHERE batch_id=?2",params![encode(&batch)?,batch.batch_id])?;}
        }
        tx.commit()
    }).map_err(|error|error.to_string())
}

pub(crate) fn list(db: &Database) -> Result<Vec<DeliveryBatch>, String> {
    recover_interrupted(db)?;
    db.with_connection(|conn| {
        let mut statement = conn.prepare(
            "SELECT body_json FROM reply_delivery_batches ORDER BY (json_extract(body_json,'$.report.status')='unknown') DESC,rowid DESC LIMIT 200",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| decode(row?))
            .collect::<rusqlite::Result<Vec<_>>>()
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn reserve_send_slot(db: &Database, account: &str) -> Result<u64, String> {
    db.with_connection(|conn| {
        let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next=tx.query_row("SELECT next_send_at FROM reply_delivery_throttle WHERE account_uin=?1",[account],|row|row.get::<_,i64>(0)).optional()?.unwrap_or(0).max(now_ms());
        let wait=next.saturating_sub(now_ms());
        if wait>5_000 {return Err(domain("账号发送队列已满，请稍后重试"));}
        tx.execute("INSERT INTO reply_delivery_throttle(account_uin,next_send_at) VALUES(?1,?2) ON CONFLICT(account_uin) DO UPDATE SET next_send_at=excluded.next_send_at",params![account,next+250])?;
        tx.commit()?;Ok(wait as u64)
    }).map_err(|error|error.to_string())
}

pub(crate) fn initial(
    batch_id: String,
    account_uin: String,
    target_type: String,
    target_id: String,
    source_message_id: String,
    input_snapshot: Vec<String>,
    authorization: String,
    plan: ReplyPlan,
) -> DeliveryBatch {
    let receipts = plan
        .bubbles
        .iter()
        .map(|bubble| BubbleReceipt {
            bubble_id: bubble.id.clone(),
            status: "pending".into(),
            message_id: None,
            error: None,
        })
        .collect();
    DeliveryBatch {
        report: DeliveryReport {
            batch_id: batch_id.clone(),
            status: "pending".into(),
            receipts,
            messages: vec![],
        },
        batch_id,
        account_uin,
        target_type,
        target_id,
        source_message_id,
        input_snapshot,
        authorization,
        plan,
        created_at: now_ms(),
        updated_at: now_ms(),
    }
}

pub(crate) fn claim(
    db: &Database,
    candidate: DeliveryBatch,
    owner: &str,
) -> Result<DeliveryBatch, String> {
    db.with_connection(|conn| {
        schema(conn)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let archived:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM reply_delivery_tombstones WHERE batch_id=?1)",[&candidate.batch_id],|row|row.get(0))?;
        if archived {return Err(domain("回复批次已归档，禁止复用编号或重复发送"));}
        let previous=read(&tx,&candidate.batch_id)?;
        let is_new=previous.is_none();
        let mut batch = if let Some(mut existing) = previous {
            if existing.account_uin != candidate.account_uin || existing.target_type != candidate.target_type || existing.target_id != candidate.target_id || existing.source_message_id != candidate.source_message_id || existing.input_snapshot != candidate.input_snapshot || existing.authorization != candidate.authorization {
                return Err(domain("回复批次已绑定其他上下文，不能重新授权或改绑"));
            }
            let (previous_owner, until): (Option<String>,i64) = tx.query_row("SELECT claimed_by,lease_until FROM reply_delivery_batches WHERE batch_id=?1", [&candidate.batch_id], |row| Ok((row.get(0)?,row.get(1)?)))?;
            if previous_owner.is_some() && until > now_ms() { return Err(domain("回复批次正在另一进程发送")); }
            for receipt in &mut existing.report.receipts {
                if receipt.status == "sending" { receipt.status = "unknown".into(); receipt.error = Some("上次发送中断，送达状态未知；核对后才能继续".into()); }
            }
            if existing.plan.fingerprint().map_err(|error|domain(&error))? != candidate.plan.fingerprint().map_err(|error|domain(&error))? {
                if !existing.report.messages.is_empty() || existing.report.receipts.iter().any(|receipt| !matches!(receipt.status.as_str(),"pending"|"failed")) {
                    return Err(domain("部分发送或未知送达的批次不能更改内容"));
                }
                existing.plan = candidate.plan;
                existing.report.receipts = candidate.report.receipts;
            }
            existing
        } else { candidate };
        let unresolved = {
            let mut statement = tx.prepare("SELECT body_json FROM reply_delivery_batches WHERE account_uin=?1 AND target_type=?2 AND target_id=?3 AND batch_id<>?4")?;
            let rows=statement.query_map(params![batch.account_uin,batch.target_type,batch.target_id,batch.batch_id],|row|row.get::<_,String>(0))?;
            let mut blocked=false;
            for row in rows {
                let previous=decode(row?)?;
                if previous.report.receipts.iter().any(|receipt|matches!(receipt.status.as_str(),"unknown"|"sending")) {blocked=true;break;}
            }
            blocked
        };
        if unresolved {return Err(domain("当前会话存在未知送达批次，请先核对该批次回执"));}
        let active = tx.query_row("SELECT claimed_by,lease_until FROM reply_delivery_targets WHERE account_uin=?1 AND target_type=?2 AND target_id=?3", params![batch.account_uin,batch.target_type,batch.target_id], |row| Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?))).optional()?;
        if active.is_some_and(|(holder,until)| holder != owner && until > now_ms()) { return Err(domain("同一会话已有消息批次发送中，请稍后重试")); }
        retain_bounded_reports(&tx,&batch.batch_id,is_new)?;
        batch.updated_at = now_ms();
        refresh_status(&mut batch.report);
        tx.execute("INSERT INTO reply_delivery_targets(account_uin,target_type,target_id,claimed_by,lease_until) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(account_uin,target_type,target_id) DO UPDATE SET claimed_by=excluded.claimed_by,lease_until=excluded.lease_until", params![batch.account_uin,batch.target_type,batch.target_id,owner,now_ms()+LEASE_MS])?;
        tx.execute("INSERT INTO reply_delivery_batches(batch_id,account_uin,target_type,target_id,body_json,claimed_by,lease_until) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(batch_id) DO UPDATE SET body_json=excluded.body_json,claimed_by=excluded.claimed_by,lease_until=excluded.lease_until",params![batch.batch_id,batch.account_uin,batch.target_type,batch.target_id,encode(&batch)?,owner,now_ms()+LEASE_MS])?;
        tx.commit()?;
        Ok(batch)
    }).map_err(|error| error.to_string())
}

pub(crate) fn save(db: &Database, batch: &mut DeliveryBatch, owner: &str) -> Result<(), String> {
    refresh_status(&mut batch.report);
    batch.updated_at = now_ms();
    db.with_connection(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute("UPDATE reply_delivery_batches SET body_json=?1,lease_until=?2 WHERE batch_id=?3 AND claimed_by=?4 AND lease_until>?5", params![encode(batch)?,now_ms()+LEASE_MS,batch.batch_id,owner,now_ms()])?;
        if updated != 1 { return Err(domain("回复批次租约已失效，禁止继续发送")); }
        let target_updated = tx.execute("UPDATE reply_delivery_targets SET lease_until=?1 WHERE account_uin=?2 AND target_type=?3 AND target_id=?4 AND claimed_by=?5 AND lease_until>?6",params![now_ms()+LEASE_MS,batch.account_uin,batch.target_type,batch.target_id,owner,now_ms()])?;
        if target_updated != 1 { return Err(domain("会话发送租约已失效，禁止继续发送")); }
        tx.commit()
    }).map_err(|error| error.to_string())
}

pub(crate) fn release(db: &Database, batch: &DeliveryBatch, owner: &str) {
    if let Err(error) = db.with_connection(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE reply_delivery_batches SET claimed_by=NULL,lease_until=0 WHERE batch_id=?1 AND claimed_by=?2",params![batch.batch_id,owner])?;
        tx.execute("DELETE FROM reply_delivery_targets WHERE account_uin=?1 AND target_type=?2 AND target_id=?3 AND claimed_by=?4",params![batch.account_uin,batch.target_type,batch.target_id,owner])?;
        tx.commit()
    }) { tracing::warn!("Could not release reply batch lease: {error}"); }
}

pub(crate) fn reconcile(
    db: &Database,
    id: &str,
    bubble_id: &str,
    message_id: Option<&str>,
    not_delivered: bool,
) -> Result<DeliveryBatch, String> {
    db.with_connection(|conn| {
        schema(conn)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut batch = read(&tx,id)?.ok_or_else(||domain("回复批次不存在"))?;
        let until: i64 = tx.query_row("SELECT lease_until FROM reply_delivery_batches WHERE batch_id=?1",[id],|row|row.get(0))?;
        if until > now_ms() { return Err(domain("批次仍在发送，不能核对回执")); }
        if message_id.is_some_and(|id|batch.report.receipts.iter().any(|receipt|receipt.bubble_id!=bubble_id && receipt.message_id.as_deref()==Some(id))) {return Err(domain("该平台消息编号已属于批次中的另一条回复"));}
        let receipt = batch.report.receipts.iter_mut().find(|receipt|receipt.bubble_id==bubble_id).ok_or_else(||domain("消息回执不存在"))?;
        if !matches!(receipt.status.as_str(),"unknown"|"sending") { return Err(domain("仅未知送达的消息允许手工核对")); }
        match (message_id,not_delivered) {
            (Some(id),false) if id.parse::<i64>().is_ok_and(|value|value!=0) => { receipt.status="sent".into();receipt.message_id=Some(id.into());receipt.error=None; }
            (None,true) => {receipt.status="pending".into();receipt.message_id=None;receipt.error=None;}
            _ => return Err(domain("须提供已送达消息编号或明确确认未送达")),
        }
        if let Some(id)=message_id {
            let message_key=format!("msg_{}_{}",batch.target_id,id);
            if !batch.report.messages.iter().any(|message|message.id==message_key) {
                let index=batch.plan.bubbles.iter().position(|bubble|bubble.id==bubble_id).ok_or_else(||domain("消息计划不存在"))?;
                batch.report.messages.push(crate::models::MessageItemDto {id:message_key,target_id:batch.target_id.clone(),sender_id:batch.account_uin.clone(),sender_name:"我".into(),content:batch.plan.bubbles[index].content(),is_from_me:true,ai_reply_status:if batch.authorization=="auto" {"auto_replied"} else {"draft_sent"}.into(),timestamp:now_ms()});
            }
        }
        refresh_status(&mut batch.report); batch.updated_at=now_ms();
        tx.execute("UPDATE reply_delivery_batches SET body_json=?1,claimed_by=NULL,lease_until=0 WHERE batch_id=?2",params![encode(&batch)?,id])?;
        tx.commit()?;Ok(batch)
    }).map_err(|error|error.to_string())
}

pub(crate) fn refresh_status(report: &mut DeliveryReport) {
    report.status = if report
        .receipts
        .iter()
        .all(|receipt| receipt.status == "sent")
    {
        "sent"
    } else if report
        .receipts
        .iter()
        .any(|receipt| matches!(receipt.status.as_str(), "unknown" | "sending"))
    {
        "unknown"
    } else if report
        .receipts
        .iter()
        .any(|receipt| receipt.status == "superseded")
    {
        "superseded"
    } else if report
        .receipts
        .iter()
        .any(|receipt| receipt.status == "cancelled")
    {
        "cancelled"
    } else if report
        .receipts
        .iter()
        .any(|receipt| receipt.status == "failed")
    {
        if report
            .receipts
            .iter()
            .any(|receipt| receipt.status == "sent")
        {
            "partial"
        } else {
            "failed"
        }
    } else {
        "pending"
    }
    .into();
}

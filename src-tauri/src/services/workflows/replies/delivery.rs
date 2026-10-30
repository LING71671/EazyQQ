use super::{store, DeliveryBatch, DeliveryReport, ReplyPlan};
use crate::models::MessageItemDto;
use crate::services::db::Database;
use crate::services::protocol::{
    context::{self, ConversationScope},
    OneBotClient,
};
use std::future::Future;

pub struct DeliveryRequest {
    pub batch_id: String,
    pub target_type: String,
    pub target_id: String,
    pub source_message_id: String,
    pub input_snapshot: Vec<String>,
    pub authorization: String,
    pub plan: ReplyPlan,
}

pub fn get_batch(db: &Database, batch_id: &str) -> Result<Option<DeliveryBatch>, String> {
    store::get(db, batch_id)
}
pub fn list_batches(db: &Database) -> Result<Vec<DeliveryBatch>, String> {
    store::list(db)
}

fn scope(batch: &DeliveryBatch) -> ConversationScope {
    ConversationScope {
        account_uin: batch.account_uin.clone(),
        target_type: batch.target_type.clone(),
        target_id: batch.target_id.clone(),
    }
}

fn confirmed_message(batch: &DeliveryBatch, index: usize, id: &str) -> MessageItemDto {
    MessageItemDto {
        id: format!("msg_{}_{}", batch.target_id, id),
        target_id: batch.target_id.clone(),
        sender_id: batch.account_uin.clone(),
        sender_name: "我".into(),
        content: batch.plan.bubbles[index].content(),
        is_from_me: true,
        ai_reply_status: if batch.authorization == "auto" {
            "auto_replied"
        } else {
            "draft_sent"
        }
        .into(),
        timestamp: store::now_ms(),
    }
}

fn save_confirmed_sources(db: &Database, batch: &DeliveryBatch) -> Result<(), String> {
    for receipt in &batch.report.receipts {
        if receipt.status != "sent" {
            continue;
        }
        let id = receipt
            .message_id
            .as_ref()
            .ok_or("已送达回执缺少平台消息编号")?;
        if let Some(message) = batch
            .report
            .messages
            .iter()
            .find(|message| message.id == format!("msg_{}_{}", batch.target_id, id))
        {
            context::save_outgoing_source(db, &scope(batch), message, id, "ai_reply_batch")?;
        }
    }
    Ok(())
}

pub fn reconcile(
    db: &Database,
    batch_id: &str,
    bubble_id: &str,
    delivered_message_id: Option<&str>,
    confirmed_not_delivered: bool,
) -> Result<DeliveryReport, String> {
    let batch = store::reconcile(
        db,
        batch_id,
        bubble_id,
        delivered_message_id,
        confirmed_not_delivered,
    )?;
    save_confirmed_sources(db, &batch)?;
    Ok(batch.report)
}

fn explicit_rejection(error: &str) -> bool {
    error.starts_with("OneBot action failed (")
        && error
            .split("retcode ")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|code| code != 0)
}

/// All targets and authorization come from the caller's bound domain context.
/// The callback is re-evaluated before every bubble, including retries.
pub async fn deliver<F, Fut>(
    db: &Database,
    client: &OneBotClient,
    request: DeliveryRequest,
    mut guard: F,
) -> Result<DeliveryReport, String>
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: Future<Output = Result<bool, String>>,
{
    request.plan.validate()?;
    if request.batch_id.is_empty()
        || request.batch_id.len() > 256
        || request.batch_id.chars().any(char::is_control)
    {
        return Err("回复批次编号无效".into());
    }
    if !matches!(request.authorization.as_str(), "auto" | "draft" | "task") {
        return Err("回复发送授权类型无效".into());
    }
    if request.source_message_id.len() > 512
        || request.source_message_id.chars().any(char::is_control)
        || request.target_id.len() > 128
        || request.target_id.chars().any(char::is_control)
    {
        return Err("回复目标或来源编号无效".into());
    }
    if request.input_snapshot.len() > 256 || request.input_snapshot.iter().any(|id| id.len() > 512)
    {
        return Err("回复上下文快照超过限制".into());
    }
    let bound = ConversationScope::for_client(client, &request.target_type, &request.target_id)?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|error| error.to_string())?;
    let owner = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let candidate = store::initial(
        request.batch_id,
        bound.account_uin,
        bound.target_type,
        bound.target_id,
        request.source_message_id,
        request.input_snapshot,
        request.authorization,
        request.plan,
    );
    let mut batch = store::claim(db, candidate, &owner)?;
    let result = deliver_claimed(db, client, &mut batch, &owner, &mut guard).await;
    store::release(db, &batch, &owner);
    result
}

async fn deliver_claimed<F, Fut>(
    db: &Database,
    client: &OneBotClient,
    batch: &mut DeliveryBatch,
    owner: &str,
    guard: &mut F,
) -> Result<DeliveryReport, String>
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: Future<Output = Result<bool, String>>,
{
    // Persisted acknowledgements are authoritative even when a previous local cache write failed.
    save_confirmed_sources(db, batch)?;
    if batch.report.receipts.iter().any(|receipt| {
        matches!(
            receipt.status.as_str(),
            "unknown" | "sending" | "cancelled" | "superseded"
        )
    }) {
        return Ok(batch.report.clone());
    }
    for index in 0..batch.plan.bubbles.len() {
        if batch.report.receipts[index].status == "sent" {
            continue;
        }
        let sent_ids = batch
            .report
            .messages
            .iter()
            .map(|message| message.id.clone())
            .collect();
        if !guard(sent_ids).await? {
            for receipt in &mut batch.report.receipts[index..] {
                if receipt.status != "sent" {
                    receipt.status = "superseded".into();
                    receipt.error = Some("上下文或发送授权已变化，剩余回复已停止".into());
                }
            }
            store::save(db, batch, owner)?;
            break;
        }
        let wait_ms = store::reserve_send_slot(db, &batch.account_uin)?;
        if wait_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
        }
        batch.report.receipts[index].status = "sending".into();
        batch.report.receipts[index].error = None;
        store::save(db, batch, owner)?;
        let content = batch.plan.bubbles[index].content();
        let sent_ids = batch
            .report
            .messages
            .iter()
            .map(|message| message.id.clone())
            .collect();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.send_msg_guarded(&batch.target_type, &batch.target_id, &content, || {
                guard(sent_ids)
            }),
        )
        .await
        .unwrap_or_else(|_| Err("发送请求超时，送达状态未知；请核对后继续".into()));
        match response {
            Ok(None) => {
                for receipt in &mut batch.report.receipts[index..] {
                    if receipt.status != "sent" {
                        receipt.status = "superseded".into();
                        receipt.error = Some("发送前上下文或授权已变化，剩余回复已停止".into());
                    }
                }
                store::save(db, batch, owner)?;
                break;
            }
            Ok(Some(value)) => {
                let message_id = context::identifier(&value["data"]["message_id"])
                    .filter(|id| id.parse::<i64>().is_ok_and(|value| value != 0))
                    .filter(|id| {
                        !batch
                            .report
                            .receipts
                            .iter()
                            .any(|receipt| receipt.message_id.as_ref() == Some(id))
                    });
                if let Some(id) = message_id {
                    let message = confirmed_message(batch, index, &id);
                    batch.report.receipts[index].status = "sent".into();
                    batch.report.receipts[index].message_id = Some(id.clone());
                    batch.report.messages.push(message);
                    // Receipt and message cache intent commit before any next network write.
                    store::save(db, batch, owner)?;
                    if let Err(error) = save_confirmed_sources(db, batch) {
                        batch.report.receipts[index].error =
                            Some(format!("已确认发送，消息来源缓存待修复: {error}"));
                        store::save(db, batch, owner)?;
                        break;
                    }
                } else {
                    batch.report.receipts[index].status = "unknown".into();
                    batch.report.receipts[index].error =
                        Some("协议未返回可验证的唯一消息编号，送达状态未知；请核对后继续".into());
                    store::save(db, batch, owner)?;
                    break;
                }
            }
            Err(error) => {
                batch.report.receipts[index].status = if explicit_rejection(&error) {
                    "failed"
                } else {
                    "unknown"
                }
                .into();
                batch.report.receipts[index].error = Some(error);
                store::save(db, batch, owner)?;
                break;
            }
        }
        if index + 1 < batch.plan.bubbles.len() {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
    Ok(batch.report.clone())
}

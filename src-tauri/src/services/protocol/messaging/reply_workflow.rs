//! Shared generation, approval and revision for desktop, CLI and MCP adapters.
use crate::models::PendingDraftDto;
use crate::services::protocol::{
    context::{self, ConversationScope},
    pipeline, OneBotClient,
};
use crate::services::workflows::replies::{self, DeliveryReport, DeliveryRequest, ReplyPlan};
use crate::services::{
    ai::{persona_settings, AiService},
    db::Database,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
struct DraftContext {
    scope: ConversationScope,
    snapshot: Vec<String>,
    persona_revision: u64,
    policy: String,
    generation: u64,
    generating: bool,
}

fn key(id: &str) -> String {
    format!("draft_context:{id}")
}

pub fn new_id(prefix: &str) -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "无法生成回复标识")?;
    Ok(format!(
        "{prefix}_{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn read_context(db: &Database, id: &str) -> Result<Option<DraftContext>, String> {
    db.get_setting(&key(id))
        .map_err(|error| error.to_string())?
        .map(|text| serde_json::from_str(&text).map_err(|_| "草稿来源版本损坏，停止发送".into()))
        .transpose()
}

pub fn policy_snapshot(
    db: &Database,
    scope: &ConversationScope,
    automatic: bool,
) -> Result<Option<String>, String> {
    let rule = db
        .get_all_rules()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|rule| {
            let kind = if rule.target_type == "friend" {
                "private"
            } else {
                rule.target_type.as_str()
            };
            rule.target_id == scope.target_id && kind == scope.target_type
        });
    Ok(rule
        .filter(|rule| {
            rule.enabled
                && if automatic {
                    rule.mode == "auto_reply"
                } else {
                    rule.mode != "ignore"
                }
        })
        .map(|rule| {
            json!([
                rule.mode,
                rule.enabled,
                rule.trigger_condition,
                rule.keywords,
                rule.cooldown_seconds
            ])
            .to_string()
        }))
}

fn verified_ids(
    db: &Database,
    scope: &ConversationScope,
    ignored: &[String],
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for message in db
        .get_messages_by_target(&scope.target_id, 101)
        .map_err(|error| error.to_string())?
    {
        if ignored.contains(&message.id) {
            continue;
        }
        if db
            .get_message_source(&message.id)
            .map_err(|error| error.to_string())?
            .as_ref()
            .is_some_and(|source| scope.accepts(source))
        {
            ids.push(message.id);
        }
    }
    Ok(ids)
}

fn task_source_current(db: &Database, source_id: &str) -> Result<bool, String> {
    if !source_id.starts_with("task:") {
        return Ok(true);
    }
    let mut components = source_id.split(':');
    components.next();
    let (Some(task_id), Some(run_id), None) =
        (components.next(), components.next(), components.next())
    else {
        return Ok(false);
    };
    db.with_connection(|connection| {
        use rusqlite::OptionalExtension;
        let record = connection
            .query_row(
                "SELECT task_json FROM assistant_tasks WHERE task_id=?1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let task = record
            .map(|text| {
                serde_json::from_str::<crate::services::workflows::tasks::Task>(&text)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .transpose()?;
        Ok(task.is_some_and(|task| {
            task.state == "completed"
                && task.active_run.as_ref().is_some_and(|run| run.id == run_id)
        }))
    })
    .map_err(|error| error.to_string())
}

pub fn input_snapshot(
    db: &Database,
    scope: &ConversationScope,
    source_id: &str,
    ignored: &[String],
) -> Result<Option<Vec<String>>, String> {
    match context::source_message(db, scope, source_id) {
        Ok(incoming) => context::reply_snapshot_ignoring(db, scope, &incoming, ignored),
        Err(error)
            if db
                .get_message_source(source_id)
                .map_err(|error| error.to_string())?
                .is_some() =>
        {
            Err(error)
        }
        Err(_) => Ok(Some(verified_ids(db, scope, ignored)?)),
    }
}

pub fn save_plan_draft(
    db: &Database,
    client: &OneBotClient,
    draft: &PendingDraftDto,
    snapshot: Vec<String>,
    persona_revision: u64,
) -> Result<(), String> {
    let scope = ConversationScope::for_client(client, &draft.target_type, &draft.target_id)?;
    let metadata = DraftContext {
        policy: policy_snapshot(db, &scope, false)?.unwrap_or_default(),
        scope,
        snapshot,
        persona_revision,
        generation: 1,
        generating: false,
    };
    db.with_connection(|connection| {
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        crate::services::storage::drafts::save_draft(&transaction, draft)?;
        crate::services::storage::settings::set_setting(
            &transaction,
            &key(&draft.id),
            &serde_json::to_string(&metadata).map_err(|_| rusqlite::Error::InvalidQuery)?,
            draft.created_at,
        )?;
        transaction.commit()
    })
    .map_err(|error| error.to_string())
}

pub async fn approve_draft(
    db: &Database,
    client: &OneBotClient,
    draft_id: &str,
    final_content: Option<String>,
    final_bubbles: Option<Vec<String>>,
) -> Result<DeliveryReport, String> {
    let draft = db
        .get_pending_drafts()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|draft| draft.id == draft_id)
        .ok_or("草稿不存在或已完成")?;
    let scope = ConversationScope::for_client(client, &draft.target_type, &draft.target_id)?;
    let existing = replies::get_batch(db, draft_id)?;
    let metadata = read_context(db, draft_id)?;
    if metadata
        .as_ref()
        .is_some_and(|metadata| metadata.scope != scope || metadata.generating)
    {
        return Err("草稿正在修订或账号作用域已变化".into());
    }
    let plan =
        if let Some(bubbles) = final_bubbles {
            ReplyPlan::from_bubble_texts(&bubbles)?
        } else if let Some(content) =
            final_content.filter(|content| content != &draft.generated_content)
        {
            ReplyPlan::from_bubble_texts(&[content])?
        } else {
            draft.reply_plan.clone().map(Ok).unwrap_or_else(|| {
                ReplyPlan::from_bubble_texts(&[draft.generated_content.clone()])
            })?
        };
    let snapshot = if let Some(batch) = &existing {
        batch.input_snapshot.clone()
    } else if let Some(metadata) = &metadata {
        metadata.snapshot.clone()
    } else {
        input_snapshot(db, &scope, &draft.reply_to_msg_id, &[])?
            .ok_or("原消息之后已有新活动，请基于最新消息重新生成")?
    };
    let policy = policy_snapshot(db, &scope, false)?.ok_or("会话已撤销 AI 接管授权，停止发送")?;
    if metadata
        .as_ref()
        .is_some_and(|metadata| metadata.policy != policy)
    {
        return Err("接管规则已变化，请重新生成草稿".into());
    }
    let persona_revision = persona_settings::resolve(db, &scope, None)?.revision;
    if metadata
        .as_ref()
        .is_some_and(|metadata| metadata.persona_revision != persona_revision)
    {
        return Err("人设已变化，请重新生成草稿".into());
    }
    let expected_generation = metadata.as_ref().map(|metadata| metadata.generation);
    let authorization = existing
        .as_ref()
        .map(|batch| batch.authorization.clone())
        .unwrap_or_else(|| "draft".into());
    let source_id = draft.reply_to_msg_id.clone();
    if !task_source_current(db, &source_id)? {
        return Err("原任务已继续、取消或删除，旧候选不能发送".into());
    }
    let request = DeliveryRequest {
        batch_id: draft.id.clone(),
        target_type: scope.target_type.clone(),
        target_id: scope.target_id.clone(),
        source_message_id: source_id.clone(),
        input_snapshot: snapshot.clone(),
        authorization,
        plan,
    };
    let report = replies::deliver(db, client, request, |sent_ids| {
        let scope = scope.clone();
        let source_id = source_id.clone();
        let snapshot = snapshot.clone();
        let policy = policy.clone();
        async move {
            // History reads are account-bound and never trigger login or an external write.
            pipeline::refresh_history(db, client, &scope, 100).await;
            let generation = read_context(db, draft_id)?;
            Ok(
                policy_snapshot(db, &scope, false)?.as_deref() == Some(policy.as_str())
                    && persona_settings::resolve(db, &scope, None)?.revision == persona_revision
                    && generation.as_ref().map(|metadata| metadata.generation)
                        == expected_generation
                    && generation
                        .as_ref()
                        .is_none_or(|metadata| !metadata.generating)
                    && task_source_current(db, &source_id)?
                    && db
                        .get_pending_drafts()
                        .map_err(|error| error.to_string())?
                        .iter()
                        .any(|draft| draft.id == draft_id)
                    && input_snapshot(db, &scope, &source_id, &sent_ids)?.as_ref()
                        == Some(&snapshot),
            )
        }
    })
    .await?;
    if report.status == "sent" {
        db.delete_draft(draft_id)
            .map_err(|error| format!("回复已发送，草稿清理失败；不要重发: {error}"))?;
    }
    Ok(report)
}

pub async fn regenerate(
    db: &Database,
    client: &OneBotClient,
    ai: &AiService,
    draft_id: &str,
    instruction: &str,
) -> Result<PendingDraftDto, String> {
    use rusqlite::OptionalExtension;
    fn require_unattempted(connection: &rusqlite::Connection, id: &str) -> rusqlite::Result<()> {
        let stored = connection
            .query_row(
                "SELECT lease_until,body_json FROM reply_delivery_batches WHERE batch_id=?1",
                [id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((until, json)) = stored {
            let batch: replies::DeliveryBatch =
                serde_json::from_str(&json).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            if until > now
                || !batch.report.messages.is_empty()
                || batch
                    .report
                    .receipts
                    .iter()
                    .any(|receipt| !matches!(receipt.status.as_str(), "pending" | "failed"))
            {
                return Err(rusqlite::Error::InvalidParameterName(
                    "已有发送记录或发送租约的计划不能重新生成".into(),
                ));
            }
        }
        Ok(())
    }
    let draft = db
        .get_pending_drafts()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|draft| draft.id == draft_id)
        .ok_or("草稿不存在")?;
    if draft.reply_to_msg_id.starts_with("task:") {
        return Err("此候选来自独立任务，请在任务工作台继续修订并转入新草稿".into());
    }
    let scope = ConversationScope::for_client(client, &draft.target_type, &draft.target_id)?;
    // Initialize/recover the batch store before the transaction-bound claim below.
    let _ = replies::get_batch(db, draft_id)?;
    let policy = policy_snapshot(db, &scope, false)?.ok_or("会话已撤销 AI 接管授权")?;
    let initial_persona = persona_settings::resolve(db, &scope, None)?;
    let (mut draft, mut metadata) = db
        .with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            require_unattempted(&transaction, draft_id)?;
            let draft = crate::services::storage::drafts::get_pending_drafts(&transaction)?
                .into_iter()
                .find(|draft| draft.id == draft_id)
                .ok_or(rusqlite::Error::InvalidQuery)?;
            let previous =
                crate::services::storage::settings::get_setting(&transaction, &key(draft_id))?;
            let mut metadata = previous
                .map(|json| {
                    serde_json::from_str::<DraftContext>(&json)
                        .map_err(|_| rusqlite::Error::InvalidQuery)
                })
                .transpose()?
                .unwrap_or(DraftContext {
                    scope: scope.clone(),
                    snapshot: vec![],
                    persona_revision: initial_persona.revision,
                    policy: policy.clone(),
                    generation: 0,
                    generating: false,
                });
            if metadata.scope != scope {
                return Err(rusqlite::Error::InvalidQuery);
            }
            metadata.generation = metadata
                .generation
                .checked_add(1)
                .ok_or(rusqlite::Error::InvalidQuery)?;
            metadata.generating = true;
            metadata.policy = policy.clone();
            metadata.persona_revision = initial_persona.revision;
            crate::services::storage::settings::set_setting(
                &transaction,
                &key(draft_id),
                &serde_json::to_string(&metadata).map_err(|_| rusqlite::Error::InvalidQuery)?,
                0,
            )?;
            transaction.commit()?;
            Ok((draft, metadata))
        })
        .map_err(|error| format!("草稿修订未取得独占版本: {error}"))?;
    let generation = metadata.generation;
    let result = async {
        let (history, incoming, member) =
            match context::source_message(db, &scope, &draft.reply_to_msg_id) {
                Ok(message) => {
                    let (history, input) = pipeline::prepare_context(
                        db,
                        client,
                        &message,
                        &draft.target_type,
                        ai.current().max_context_messages,
                    )
                    .await?;
                    (history, input, Some(message.sender_id))
                }
                Err(error)
                    if db
                        .get_message_source(&draft.reply_to_msg_id)
                        .map_err(|error| error.to_string())?
                        .is_some() =>
                {
                    return Err(error)
                }
                Err(_) => (
                    context::build_context_history(
                        db,
                        &scope,
                        None,
                        ai.current().max_context_messages,
                    )?,
                    context::requested_input(&scope, &draft.incoming_message_snippet),
                    None,
                ),
            };
        metadata.snapshot = input_snapshot(db, &scope, &draft.reply_to_msg_id, &[])?
            .ok_or("会话已有新消息，请为最新消息生成草稿")?;
        let persona = persona_settings::resolve(db, &scope, member.as_deref())?;
        if persona.revision != initial_persona.revision
            || policy_snapshot(db, &scope, false)?.as_deref() != Some(metadata.policy.as_str())
            || read_context(db, draft_id)?.is_none_or(|current| current.generation != generation)
        {
            return Err("生成前上下文、人设、授权或修订版本已变化".into());
        }
        let service = ai.for_request(&new_id("draft_revision")?)?;
        let ticket = ai.reserve_inference_sequence();
        let generated = service
            .generate_reply_plan(
                &history,
                &incoming,
                &draft.target_name,
                &persona.text,
                Some((&draft.generated_content, instruction)),
            )
            .await;
        ai.observe_scoped_inference(&service.current(), ticket, &generated);
        let (plan, thinking) = generated?;
        if service.current() != ai.current() {
            return Err("模型配置已变化，旧修订结果未提交".into());
        }
        if read_context(db, draft_id)?.is_none_or(|current| current.generation != generation) {
            return Err("已有更新的修订，本次旧结果已丢弃".into());
        }
        if input_snapshot(db, &scope, &draft.reply_to_msg_id, &[])?.as_ref()
            != Some(&metadata.snapshot)
            || persona_settings::resolve(db, &scope, None)?.revision != persona.revision
            || policy_snapshot(db, &scope, false)?.as_deref() != Some(metadata.policy.as_str())
        {
            return Err("生成期间上下文、人设或授权已变化，请重新生成".into());
        }
        draft.generated_content = plan.preview();
        draft.reply_plan = Some(plan);
        draft.thinking_content = thinking;
        draft.model_used = service.model();
        metadata.generating = false;
        db.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            require_unattempted(&transaction, draft_id)?;
            let current =
                crate::services::storage::settings::get_setting(&transaction, &key(draft_id))?
                    .ok_or(rusqlite::Error::InvalidQuery)?;
            let current: DraftContext =
                serde_json::from_str(&current).map_err(|_| rusqlite::Error::InvalidQuery)?;
            if current.generation != generation || !current.generating {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM pending_drafts WHERE id=?1)",
                [draft_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(rusqlite::Error::InvalidQuery);
            }
            crate::services::storage::drafts::save_draft(&transaction, &draft)?;
            crate::services::storage::settings::set_setting(
                &transaction,
                &key(draft_id),
                &serde_json::to_string(&metadata).map_err(|_| rusqlite::Error::InvalidQuery)?,
                draft.created_at,
            )?;
            transaction.commit()
        })
        .map_err(|error| format!("草稿版本已变化，旧结果未写入: {error}"))?;
        Ok(draft)
    }
    .await;
    if result.is_err() {
        db.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            if let Some(text) =
                crate::services::storage::settings::get_setting(&transaction, &key(draft_id))?
            {
                if let Ok(mut current) = serde_json::from_str::<DraftContext>(&text) {
                    if current.generation == generation {
                        current.generating = false;
                        crate::services::storage::settings::set_setting(
                            &transaction,
                            &key(draft_id),
                            &serde_json::to_string(&current)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            0,
                        )?;
                    }
                }
            }
            transaction.commit()
        })
        .map_err(|error| error.to_string())?;
    }
    result
}

pub fn manage_batches(
    db: &Database,
    client: &OneBotClient,
    request: Value,
) -> Result<Value, String> {
    if request["operation"] == "list" {
        let batches = replies::list_batches(db)?
            .into_iter()
            .filter(|batch| client.expected_account() == Some(batch.account_uin.as_str()))
            .collect::<Vec<_>>();
        return serde_json::to_value(batches).map_err(|error| error.to_string());
    }
    let id = request["batchId"].as_str().ok_or("缺少 batchId")?;
    let batch = replies::get_batch(db, id)?.ok_or("回复批次不存在")?;
    if client.expected_account() != Some(batch.account_uin.as_str()) {
        return Err("回复批次账号不一致".into());
    }
    let report = match request["operation"].as_str().unwrap_or("get") {
        "get" => batch.report,
        "reconcile" => replies::reconcile(
            db,
            id,
            request["bubbleId"].as_str().ok_or("缺少 bubbleId")?,
            request["deliveredMessageId"].as_str(),
            request["confirmedNotDelivered"].as_bool().unwrap_or(false),
        )?,
        _ => return Err("未知回执操作".into()),
    };
    Ok(serde_json::to_value(report).map_err(|error| error.to_string())?)
}

pub async fn suggest(
    db: &Database,
    client: &OneBotClient,
    ai: &AiService,
    target_id: &str,
    target_type: Option<&str>,
    snippet: &str,
    source_id: Option<&str>,
) -> Result<(ReplyPlan, Option<String>), String> {
    let kind = context::target_type(db, target_id, target_type)?;
    let scope = ConversationScope::for_client(client, &kind, target_id)?;
    let (history, incoming, name, member) = if let Some(id) = source_id {
        let message = context::source_message(db, &scope, id)?;
        let (history, input) = pipeline::prepare_context(
            db,
            client,
            &message,
            &kind,
            ai.current().max_context_messages,
        )
        .await?;
        (history, input, message.sender_name, Some(message.sender_id))
    } else {
        (
            context::build_context_history(db, &scope, None, ai.current().max_context_messages)?,
            context::requested_input(&scope, snippet),
            "当前会话".into(),
            None,
        )
    };
    let persona = persona_settings::resolve(db, &scope, member.as_deref())?;
    let service = ai.for_request(&new_id("suggestion")?)?;
    let ticket = ai.reserve_inference_sequence();
    let generated = service
        .generate_reply_plan(&history, &incoming, &name, &persona.text, None)
        .await;
    ai.observe_scoped_inference(&service.current(), ticket, &generated);
    let result = generated?;
    if persona_settings::resolve(db, &scope, None)?.revision != persona.revision
        || ai.current() != service.current()
    {
        return Err("人设或模型配置已变化，候选回复未提交，请重新生成".into());
    }
    Ok(result)
}

pub fn create_task_draft(
    db: &Database,
    client: &OneBotClient,
    _ai: &AiService,
    task: &crate::services::workflows::tasks::Task,
) -> Result<PendingDraftDto, String> {
    if task.state != "completed" || client.expected_account() != Some(task.account_uin.as_str()) {
        return Err("仅当前账号已完成的任务可以转入草稿".into());
    }
    let candidate = task.candidate_reply.clone().ok_or("任务没有候选回复")?;
    let plan = ReplyPlan::parse_candidate(&candidate.to_string())?;
    let scope = ConversationScope::for_client(client, &task.target_type, &task.target_id)?;
    let persona = persona_settings::resolve(db, &scope, None)?;
    let run = task.active_run.as_ref().ok_or("任务没有运行记录")?;
    if run.persona_revision != persona.revision {
        return Err("人设已变化，请继续任务更新候选回复".into());
    }
    let snapshot = input_snapshot(db, &scope, "", &[])?.unwrap_or_default();
    if snapshot != run.context_snapshot {
        return Err("会话已有新消息，请继续任务更新候选回复".into());
    }
    let policy = policy_snapshot(db, &scope, false)?
        .ok_or("会话未授权 AI 草稿审核；请在联系人中开启草稿模式")?;
    let draft = PendingDraftDto {
        id: format!("task_draft_{}_{}", task.id, run.id),
        target_id: task.target_id.clone(),
        target_name: task.title.clone(),
        target_type: task.target_type.clone(),
        reply_to_msg_id: format!("task:{}:{}", task.id, run.id),
        incoming_message_snippet: task.instruction.clone(),
        generated_content: plan.preview(),
        reply_plan: Some(plan),
        thinking_content: None,
        model_used: task.session.model.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
    };
    if replies::is_completed_batch(db, &draft.id)? {
        return Err("这次任务的候选已经发送，不能再次创建同一草稿".into());
    }
    let metadata = DraftContext {
        scope: scope.clone(),
        snapshot,
        persona_revision: persona.revision,
        policy,
        generation: 1,
        generating: false,
    };
    db.with_connection(|connection|{
        let transaction=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let record:String=transaction.query_row("SELECT task_json FROM assistant_tasks WHERE task_id=?1 AND account_uin=?2 AND revision=?3 AND state='completed'",rusqlite::params![task.id,task.account_uin,task.revision],|row|row.get(0))?;
        let current:crate::services::workflows::tasks::Task=serde_json::from_str(&record).map_err(|_|rusqlite::Error::InvalidQuery)?;
        if current.active_run.as_ref().is_none_or(|current_run|current_run.id!=run.id) {return Err(rusqlite::Error::InvalidQuery);}
        if let Some(existing)=crate::services::storage::drafts::get_pending_drafts(&transaction)?.into_iter().find(|existing|existing.id==draft.id) {transaction.commit()?;return Ok(existing);}
        crate::services::storage::drafts::save_draft(&transaction,&draft)?;
        crate::services::storage::settings::set_setting(&transaction,&key(&draft.id),&serde_json::to_string(&metadata).map_err(|_|rusqlite::Error::InvalidQuery)?,draft.created_at)?;
        transaction.commit()?;Ok(draft)
    }).map_err(|error|format!("任务版本已变化或草稿保存失败: {error}"))
}

use crate::{args::Args, context::Services, format::print_json};
use serde_json::json;

pub async fn cmd_tasks(svc: &Services, args: &Args) -> Result<(), String> {
    let mut request = json!({"operation":args.flag("action").unwrap_or("list"),"taskId":args.flag("id"),"targetId":args.flag("target"),"targetType":args.flag("type"),"title":args.flag("title"),"instruction":args.flag("instruction"),"persona":args.flag("persona")});
    if let Some(value) = args.flag("revision") {
        request["expectedRevision"] = json!(value
            .parse::<u64>()
            .map_err(|_| "--revision 必须是正整数")?);
    }
    if let Some(value) = args.flag("selected-file-ids") {
        request["selectedFileIds"] = serde_json::from_str::<Vec<String>>(value)
            .map_err(|_| "--selected-file-ids 必须是文件 ID 的 JSON 数组")?
            .into();
    }
    request["allowTodos"] = json!(args.has("allow-todos"));
    let response = eazyqq_lib::services::workflows::tasks::manage(
        svc.db.clone(),
        svc.onebot.clone(),
        svc.ai.clone(),
        request,
    )
    .await?;
    print_json(&response);
    Ok(())
}

pub fn cmd_personas(svc: &Services, args: &Args) -> Result<(), String> {
    let request = json!({"operation":args.flag("action").unwrap_or("get"),"targetId":args.flag("target"),"targetType":args.flag("type"),"memberUin":args.flag("member"),"text":args.flag("text")});
    if svc.onebot.expected_account().is_none() {
        return Err("请通过 --account 绑定账号".into());
    }
    let response = eazyqq_lib::services::ai::persona_settings::manage(&svc.db, request)?;
    print_json(&response);
    Ok(())
}

pub fn cmd_reply_batches(svc: &Services, args: &Args) -> Result<(), String> {
    let request = json!({"operation":args.flag("action").unwrap_or("list"),"confirmedNotDelivered":args.has("not-delivered"),"batchId":args.flag("id"),"bubbleId":args.flag("bubble"),"deliveredMessageId":args.flag("message-id")});
    let response = eazyqq_lib::services::protocol::reply_workflow::manage_batches(
        &svc.db,
        &svc.onebot,
        request,
    )?;
    print_json(&response);
    Ok(())
}

use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json, truncate};

pub async fn cmd_files(svc: &Services, args: &Args) -> Result<(), String> {
    let group = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号>".to_string())?;

    let records = eazyqq_lib::services::group_files::sync_group_files(&svc.db, &svc.onebot, group)
        .await
        .unwrap_or_default();

    if args.json() {
        print_json(&serde_json::to_value(&records).unwrap_or(serde_json::json!([])));
        return Ok(());
    }

    hr();
    println!("群文件 ({} 个) - 群 {}", records.len(), group);
    hr();
    if records.is_empty() {
        println!("(该群没有文件，或 OneBot 未返回文件列表)");
    }
    for f in &records {
        println!(
            "{}  {:<34} {:>8} KB  [{}]{}",
            f.file_id,
            truncate(&f.file_name, 34),
            f.file_size / 1024,
            f.download_status,
            f.local_path
                .as_ref()
                .map(|p| format!("  -> {}", p))
                .unwrap_or_default()
        );
    }
    hr();
    Ok(())
}

pub async fn cmd_file_download(svc: &Services, args: &Args) -> Result<(), String> {
    let group = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号>".to_string())?;
    let file_id = args
        .flag("file-id")
        .ok_or_else(|| "缺少 --file-id <文件ID>（可用 `eazyqq-cli files` 查看）".to_string())?;

    let path = eazyqq_lib::services::group_files::download_group_file(
        &svc.db,
        &svc.onebot,
        group,
        file_id,
    )
    .await?;

    if args.json() {
        print_json(&serde_json::json!({ "localSavePath": path.display().to_string() }));
    } else {
        println!("已下载 -> {}", path.display());
    }
    Ok(())
}

pub async fn cmd_file_summarize(svc: &Services, args: &Args) -> Result<(), String> {
    let path = args
        .flag("path")
        .ok_or_else(|| "缺少 --path <本地文件路径>".to_string())?;

    let result = eazyqq_lib::services::group_files::summarize_file(&svc.ai, path).await?;

    if args.json() {
        print_json(&serde_json::to_value(&result).unwrap_or(serde_json::json!({})));
        return Ok(());
    }

    hr();
    println!("文档综述 -> {}", result.file_name);
    println!(
        "大小 {} KB | 提取字符 {}",
        result.file_size / 1024,
        result.total_chars
    );
    hr();
    println!("{}", result.summary_text);
    if !result.key_takeaways.is_empty() {
        println!();
        println!("[核心论点]");
        for t in &result.key_takeaways {
            println!("  - {}", t);
        }
    }
    if !result.action_items.is_empty() {
        println!();
        println!("[待办事项]");
        for a in &result.action_items {
            println!("  - {}", a);
        }
    }
    hr();
    Ok(())
}

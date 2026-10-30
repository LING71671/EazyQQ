use crate::{args::Args, context::Services, format::print_json};
use eazyqq_lib::commands::ai::style_packs::manage_for_account;

pub fn cmd_style_packs(svc: &Services, args: &Args) -> Result<(), String> {
    let operation = args.flag("action").unwrap_or("list");
    let pack_json = if operation == "import" {
        let path = args.flag("file").ok_or("缺少 --file <风格包.json>")?;
        let metadata = std::fs::metadata(path).map_err(|_| "无法读取风格包文件")?;
        if !metadata.is_file() || metadata.len() > 65_536 { return Err("风格包文件不能超过64KiB".into()); }
        let file = std::fs::File::open(path).map_err(|_| "无法读取风格包文件")?;
        use std::io::Read;
        let mut content = String::new();
        file.take(65_537).read_to_string(&mut content).map_err(|_| "风格包必须是UTF-8 JSON文件")?;
        Some(content)
    } else { None };
    let status = manage_for_account(
        svc.ai.runtime_dir().parent().ok_or("账号目录不可用")?,
        operation, pack_json.as_deref(), args.flag("id"), args.flag("output"),
    )?;
    if args.json() {
        print_json(&serde_json::to_value(&status).map_err(|_| "风格包状态序列化失败")?);
    } else {
        println!("风格包：{}", if status.enabled { "已启用" } else { "已停用" });
        for pack in status.packs {
            println!("{} {} {}{}", pack.id, pack.name, pack.version,
                if status.active_id.as_deref() == Some(pack.id.as_str()) { "（当前）" } else { "" });
        }
        if let Some(path) = status.exported_path { println!("已导出：{path}"); }
    }
    Ok(())
}

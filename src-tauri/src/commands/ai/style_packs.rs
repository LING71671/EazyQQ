use crate::commands::AppState;
use crate::models::ApiResponse;
use crate::services::ai::style_pack::{self, StylePack};
use serde::Serialize;
use std::path::Path;
use tauri::{command, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePackStatus {
    pub enabled: bool,
    pub active_id: Option<String>,
    pub packs: Vec<StylePack>,
    pub exported_path: Option<String>,
}

pub fn manage_for_account(
    account_dir: &Path,
    operation: &str,
    pack_json: Option<&str>,
    pack_id: Option<&str>,
    destination: Option<&str>,
) -> Result<StylePackStatus, String> {
    let mut exported_path = None;
    match operation {
        "list" => {}
        "import" => { style_pack::import_for_account(account_dir, pack_json.ok_or("请选择风格包JSON文件")?.as_bytes())?; }
        "select" => { style_pack::select_for_account(account_dir, pack_id.ok_or("请选择风格包")?)?; }
        "enable" => { style_pack::set_enabled(account_dir, true)?; }
        "disable" => { style_pack::set_enabled(account_dir, false)?; }
        "remove" => { style_pack::remove_for_account(account_dir)?; }
        "export" => {
            let packs = style_pack::list_for_account(account_dir)?;
            let pack = packs.iter().find(|pack| Some(pack.id.as_str()) == pack_id).ok_or("找不到指定风格包")?;
            let path = Path::new(destination.ok_or("请选择导出位置")?);
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return Err("风格包必须导出为.json文件".into());
            }
            let bytes = serde_json::to_vec_pretty(pack).map_err(|_| "风格包序列化失败")?;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
                .map_err(|_| "无法导出风格包，请选择尚不存在的JSON文件路径")?;
            file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|_| "风格包导出失败")?;
            exported_path = Some(path.to_string_lossy().into_owned());
        }
        _ => return Err("未知风格包操作".into()),
    }
    let selected = style_pack::read_for_account(account_dir)?;
    Ok(StylePackStatus {
        enabled: selected.as_ref().map(|selected| selected.enabled).unwrap_or(false),
        active_id: selected.map(|selected| selected.pack.id),
        packs: style_pack::list_for_account(account_dir)?,
        exported_path,
    })
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.manage_style_packs", skip_all, err)]
pub async fn manage_style_packs(
    state: State<'_, AppState>,
    operation: String,
    pack_json: Option<String>,
    pack_id: Option<String>,
    destination: Option<String>,
) -> Result<ApiResponse<StylePackStatus>, String> {
    let account_dir = state.ai.runtime_dir().parent().ok_or("账号目录不可用")?;
    manage_for_account(account_dir, &operation, pack_json.as_deref(), pack_id.as_deref(), destination.as_deref()).map(ApiResponse::ok)
}

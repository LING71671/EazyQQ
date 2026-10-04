use std::path::Path;
use serde::Serialize;

use super::boot::running_qq_processes;
use super::patch::configured_qq_path;

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

impl Step {
    pub fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, ok: true, detail: detail.into() }
    }

    pub fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, ok: false, detail: detail.into() }
    }
}

pub fn diagnose(napcat_dir: &Path, uin: Option<&str>) -> Vec<Step> {
    let mut steps = Vec::new();

    // 1. Directory
    steps.push(if napcat_dir.is_dir() {
        Step::ok("NapCat 目录", napcat_dir.display().to_string())
    } else {
        Step::fail("NapCat 目录", format!("不存在: {}", napcat_dir.display()))
    });

    // 2. Required files
    let required = [
        ("NapCatWinBootMain.exe", "启动引导器"),
        ("NapCatWinBootHook.dll", "注入钩子"),
        ("napcat.mjs", "NapCat 主体"),
    ];
    let missing: Vec<String> = required
        .iter()
        .filter(|(f, _)| !napcat_dir.join(f).is_file())
        .map(|(f, what)| format!("{}（{}）", f, what))
        .collect();
    steps.push(if missing.is_empty() {
        Step::ok("必需文件", format!("{} 项齐全", required.len()))
    } else {
        Step::fail("必需文件", format!("缺少 {}", missing.join("、")))
    });

    // 3. QQ path
    let qq_path = configured_qq_path(napcat_dir);
    match &qq_path {
        Ok(p) => steps.push(Step::ok("QQ 路径配置", p.display().to_string())),
        Err(e) => steps.push(Step::fail("QQ 路径配置", e.clone())),
    }

    // 4. QQ version
    if let Ok(p) = &qq_path {
        let mut versions: Vec<String> = p
            .parent()
            .map(|d| d.join("versions"))
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        versions.sort();
        steps.push(if versions.is_empty() {
            Step::fail("QQ 版本", "versions/ 下没有版本目录，QQ 安装可能不完整")
        } else {
            Step::ok("QQ 版本", versions.join("、"))
        });
    }

    // 5. QQ running
    let running = running_qq_processes();
    steps.push(if running.is_empty() {
        Step::ok("QQ 进程", "未在运行，NapCat 可以自行引导 QQ 启动")
    } else {
        Step::fail(
            "QQ 进程",
            format!(
                "已有 {} 个 QQ 实例在运行。NapCat 会自行引导一个 QQ 实例启动，两者可能冲突。\
                 实测中 NapCat 拉起的 4 个 QQ 进程约 3 秒后全部退出。若 NapCat 反复启动失败，\
                 请先完全退出 QQ 再试。",
                running.len()
            ),
        )
    });

    // 6. Account configuration
    if let Some(uin) = uin.filter(|u| !u.is_empty()) {
        let napcat_cfg = napcat_dir.join("config").join(format!("napcat_{}.json", uin));
        let onebot_cfg = napcat_dir.join("config").join(format!("onebot11_{}.json", uin));
        let absent: Vec<&str> = [
            (&napcat_cfg, "napcat_<uin>.json"),
            (&onebot_cfg, "onebot11_<uin>.json"),
        ]
        .iter()
        .filter(|(p, _)| !p.is_file())
        .map(|(_, what)| *what)
        .collect();
        steps.push(if absent.is_empty() {
            Step::ok("账号配置", format!("{} 的配置已就绪", uin))
        } else {
            Step::fail("账号配置", format!("缺少 {}", absent.join("、")))
        });
    }

    // 7. WebUI endpoint
    let webui_cfg = napcat_dir.join("config").join("webui.json");
    match std::fs::read_to_string(&webui_cfg)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    {
        Some(v) => {
            let host = v.get("host").and_then(|h| h.as_str()).unwrap_or("?");
            let port = v.get("port").and_then(|p| p.as_u64()).unwrap_or(0);
            let has_token = v
                .get("token")
                .and_then(|t| t.as_str())
                .map(|t| !t.is_empty())
                .unwrap_or(false);
            steps.push(Step::ok(
                "WebUI 配置",
                format!("{}:{}{}", host, port, if has_token { "（含 token）" } else { "（无 token）" }),
            ));
        }
        None => steps.push(Step::fail(
            "WebUI 配置",
            format!("无法读取 {}", webui_cfg.display()),
        )),
    }

    // 8. NapCat log check
    let log_dir = napcat_dir.join("logs");
    let newest = std::fs::read_dir(&log_dir)
        .ok()
        .and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter_map(|m| m.modified().ok())
                .max()
        });
    steps.push(match newest {
        Some(t) => {
            let age = t.elapsed().map(|d| d.as_secs()).unwrap_or(u64::MAX);
            Step::ok("NapCat 自身日志", format!("最近写入于 {} 秒前", age))
        }
        None => Step::fail(
            "NapCat 自身日志",
            format!(
                "{} 下没有任何日志，fileLog 是开启的。NapCat 会自行创建该目录，\
                 没有日志表示尚未完成初始化启动。",
                log_dir.display()
            ),
        ),
    });

    steps
}

pub fn first_blocker(napcat_dir: &Path, uin: Option<&str>) -> Option<Step> {
    diagnose(napcat_dir, uin).into_iter().find(|s| !s.ok)
}

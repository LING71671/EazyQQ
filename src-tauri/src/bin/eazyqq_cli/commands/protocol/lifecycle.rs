use crate::args::Args;
use crate::context::Services;
use std::path::Path;

pub async fn cmd_start(svc: &Services, _args: &Args) -> Result<(), String> {
    let napcat_path = Path::new(svc.napcat.napcat_dir());
    let outcome = eazyqq_lib::services::protocol::boot::start_with_args(
        napcat_path,
        None,
        eazyqq_lib::services::accounts::active().as_deref(),
    );
    if outcome.ok {
        if _args.json() {
            crate::format::print_json(&serde_json::to_value(&outcome).map_err(|e| e.to_string())?);
        } else {
            println!("{}", outcome.detail);
        }
        Ok(())
    } else {
        Err(format!("NapCat 启动失败: {}", outcome.detail))
    }
}

pub fn cmd_stop(args: &Args) -> Result<(), String> {
    let outcome = eazyqq_lib::services::napcat_boot::stop();
    if args.json() {
        crate::format::print_json(&serde_json::to_value(&outcome).map_err(|e| e.to_string())?);
    } else {
        println!("{}", outcome.detail);
    }
    if outcome.ok {
        Ok(())
    } else {
        Err(outcome.detail)
    }
}

pub async fn cmd_restart(svc: &Services, _args: &Args) -> Result<(), String> {
    let napcat_path = Path::new(svc.napcat.napcat_dir());
    let outcome = eazyqq_lib::services::napcat_boot::restart(napcat_path);
    if outcome.ok {
        if _args.json() {
            crate::format::print_json(&serde_json::to_value(&outcome).map_err(|e| e.to_string())?);
        } else {
            println!("{}", outcome.detail);
        }
        Ok(())
    } else {
        Err(format!("NapCat 重启失败: {}", outcome.detail))
    }
}

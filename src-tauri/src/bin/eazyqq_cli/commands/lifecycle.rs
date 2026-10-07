use std::path::Path;
use crate::args::Args;
use crate::context::Services;

pub async fn cmd_start(svc: &Services, _args: &Args) -> Result<(), String> {
    let napcat_path = Path::new(svc.napcat.napcat_dir());
    let outcome = eazyqq_lib::services::napcat_boot::start(napcat_path);
    if outcome.ok {
        println!("NapCat 启动成功: {}", outcome.detail);
        Ok(())
    } else {
        Err(format!("NapCat 启动失败: {}", outcome.detail))
    }
}

pub fn cmd_stop(_args: &Args) -> Result<(), String> {
    let outcome = eazyqq_lib::services::napcat_boot::stop();
    println!("{}", outcome.detail);
    Ok(())
}

pub async fn cmd_restart(svc: &Services, _args: &Args) -> Result<(), String> {
    let napcat_path = Path::new(svc.napcat.napcat_dir());
    let outcome = eazyqq_lib::services::napcat_boot::restart(napcat_path);
    if outcome.ok {
        println!("NapCat 重启成功: {}", outcome.detail);
        Ok(())
    } else {
        Err(format!("NapCat 重启失败: {}", outcome.detail))
    }
}

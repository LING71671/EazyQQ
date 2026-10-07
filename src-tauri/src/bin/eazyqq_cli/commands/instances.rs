use std::path::Path;
use crate::args::Args;
use crate::context::Services;
use crate::format::hr;
use eazyqq_lib::services::identity::instances::{
    get_or_register_instance, load_registry,
};
use eazyqq_lib::services::protocol::boot::{
    start_instance, stop_all_instances, stop_instance,
};

pub async fn cmd_instances(svc: &Services, args: &Args) -> Result<(), String> {
    let sub = args.positional.first().map(|s| s.as_str()).unwrap_or("list");
    let napcat_dir = Path::new(svc.napcat.napcat_dir());

    match sub {
        "list" => cmd_list(svc, args).await,
        "start" => {
            if args.has("all") {
                let reg = load_registry();
                if reg.instances.is_empty() {
                    println!("当前未登记任何多开实例，可使用 `instances add --uin <QQ号>` 添加。");
                    return Ok(());
                }
                println!("正在启动所有登记的实例 ({} 个)...", reg.instances.len());
                for inst in &reg.instances {
                    let outcome = start_instance(napcat_dir, &inst.uin);
                    println!("  [{}] -> {}", inst.uin, outcome.detail);
                }
                Ok(())
            } else if let Some(uin) = args.flag("uin") {
                let outcome = start_instance(napcat_dir, uin);
                if outcome.ok {
                    println!("{}", outcome.detail);
                    Ok(())
                } else {
                    Err(outcome.detail)
                }
            } else {
                Err("缺少 --uin <QQ号> 或 --all".to_string())
            }
        }
        "stop" => {
            if args.has("all") {
                let outcomes = stop_all_instances();
                for o in outcomes {
                    println!("  {}", o.detail);
                }
                Ok(())
            } else if let Some(uin) = args.flag("uin") {
                let outcome = stop_instance(uin);
                println!("{}", outcome.detail);
                Ok(())
            } else {
                Err("缺少 --uin <QQ号> 或 --all".to_string())
            }
        }
        "add" => {
            let uin = args.flag("uin").ok_or_else(|| "缺少 --uin <QQ号>".to_string())?;
            let nick = args.flag("nick");
            let inst = get_or_register_instance(uin, nick, napcat_dir)?;
            println!(
                "已登记账号 {}：分配 HTTP 端口 {}, WS 端口 {}",
                inst.uin, inst.http_port, inst.ws_port
            );
            Ok(())
        }
        other => Err(format!("未知子命令: `instances {}`，支持 list, start, stop, add", other)),
    }
}

async fn cmd_list(svc: &Services, args: &Args) -> Result<(), String> {
    let reg = load_registry();
    if args.has("json") {
        let val = serde_json::to_value(&reg).map_err(|e| e.to_string())?;
        crate::format::print_json(&val);
        return Ok(());
    }

    hr();
    println!("EazyQQ 原生多开账号实例列表 (共 {} 个)", reg.instances.len());
    hr();

    if reg.instances.is_empty() {
        println!("当前未登记任何多开账号。");
        println!("提示：运行 `eazyqq_cli instances add --uin <QQ号>` 登记新分身。");
        hr();
        return Ok(());
    }

    for inst in &reg.instances {
        let client = svc.instance_pool.get_client(&inst.uin).await;
        let online = client.get_login_info().await.is_ok();
        let status_tag = if online {
            "[在线]"
        } else {
            "[离线]"
        };

        let pid_str = inst.child_pid.map(|p| format!("PID {}", p)).unwrap_or_else(|| "-".to_string());
        let nick_str = inst.nickname.as_deref().unwrap_or("-");

        println!(
            "{:<6} 账号: {:<11} 昵称: {:<10} 端口(HTTP/WS): {:>4}/{:<4}  {}",
            status_tag, inst.uin, nick_str, inst.http_port, inst.ws_port, pid_str
        );
    }
    hr();
    Ok(())
}

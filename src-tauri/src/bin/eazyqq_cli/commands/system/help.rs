pub fn cmd_help() {
    println!("EazyQQ CLI {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: eazyqq_cli [--account QQ] <command> [options] [--json]");
    for command in super::schema::COMMANDS {
        println!("  {}", command);
    }
    println!("accounts list|add|select|qr|start|stop|login|configure|forget");
    println!("Batch: accounts start|stop|login --uins QQ,QQ or --all [--dry-run]");
    println!("Run schema --json for the machine contract; see docs/api/CLI.md for examples");
}

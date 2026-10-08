#![windows_subsystem = "windows"]

fn main() {
    if let Some(code) = eazyqq_lib::services::protocol::ownership::run_supervisor_if_requested() {
        std::process::exit(code);
    }
    eazyqq_lib::run();
}

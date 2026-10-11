#![windows_subsystem = "windows"]

fn main() {
    if let Some(code) = eazyqq_lib::services::infra::updates::run_helper_if_requested() {
        eazyqq_lib::services::infra::telemetry::flush();
        std::process::exit(code);
    }
    if let Some(code) = eazyqq_lib::services::protocol::ownership::run_supervisor_if_requested() {
        std::process::exit(code);
    }
    let _telemetry = eazyqq_lib::services::infra::telemetry::initialize("desktop");
    eazyqq_lib::run();
}

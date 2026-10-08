#[path = "auth/accounts.rs"]
pub mod accounts;
#[path = "runtime/boot.rs"]
pub mod boot;
#[path = "health/diagnostics.rs"]
pub mod diagnostics;
#[path = "messaging/events.rs"]
pub mod events;
#[path = "health/health.rs"]
pub mod health;
#[path = "runtime/instance_pool.rs"]
pub mod instance_pool;
#[path = "messaging/listener.rs"]
pub mod listener;
#[path = "transport/onebot.rs"]
pub mod onebot;
#[path = "runtime/ownership.rs"]
pub mod ownership;
#[path = "runtime/patch.rs"]
pub mod patch;
#[path = "messaging/pipeline.rs"]
pub mod pipeline;
#[path = "auth/service.rs"]
pub mod service;
#[path = "auth/session.rs"]
pub mod session;
#[path = "runtime/workers.rs"]
pub mod workers;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

pub use boot::{
    consecutive_failures, locate_napcat_dir, note_healthy, note_unready, restart, start,
    start_instance, stop, stop_all_instances, stop_instance, BootOutcome,
};
pub use diagnostics::{diagnose, first_blocker, Step};
pub use events::{handle_onebot_event, MessageOutcome};
pub use instance_pool::InstancePool;
pub use listener::start_onebot_ws_listener;
pub use onebot::{ObFriendInfo, ObGroupFileInfo, ObGroupInfo, OneBotClient};
pub use patch::{configured_qq_path, sync_qqnt_patch};
pub use service::NapCatService;

// Backward-compatible module facades
pub mod napcat {
    pub use super::service::NapCatService;
}

pub mod napcat_boot {
    pub use super::boot::{
        consecutive_failures, locate_napcat_dir, note_healthy, note_unready, restart, start,
        start_instance, stop, stop_all_instances, stop_instance, BootOutcome,
    };
    pub use super::diagnostics::{diagnose, first_blocker, Step};
    pub use super::patch::{configured_qq_path, sync_qqnt_patch};
}

pub mod ws_listener {
    pub use super::events::{handle_onebot_event, MessageOutcome};
    pub use super::listener::start_onebot_ws_listener;
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod http_tests;

#[path = "runtime/layout.rs"]
pub mod layout;

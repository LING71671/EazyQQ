pub mod boot;
pub mod diagnostics;
pub mod events;
pub mod listener;
pub mod onebot;
pub mod patch;
pub mod pipeline;
pub mod service;

#[cfg(test)]
mod tests;

pub use boot::{
    consecutive_failures, note_healthy, note_unready, restart, start, stop, BootOutcome,
};
pub use diagnostics::{diagnose, first_blocker, Step};
pub use events::{handle_onebot_event, MessageOutcome};
pub use listener::start_onebot_ws_listener;
pub use onebot::{ObFriendInfo, ObGroupFileInfo, ObGroupInfo, OneBotClient};
pub use patch::{configured_qq_path, sync_qqnt_patch};
pub use service::NapCatService;

// Backward-compatible module facades
pub mod napcat {
    pub use super::service::NapCatService;
}

pub mod napcat_boot {
    pub use super::boot::{consecutive_failures, note_healthy, note_unready, restart, start, stop, BootOutcome};
    pub use super::diagnostics::{diagnose, first_blocker, Step};
    pub use super::patch::{configured_qq_path, sync_qqnt_patch};
}

pub mod ws_listener {
    pub use super::events::{handle_onebot_event, MessageOutcome};
    pub use super::listener::start_onebot_ws_listener;
}

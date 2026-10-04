//! Services layer: 7 domain-driven modular subsystems with clean facades.

pub mod ai;
pub mod identity;
pub mod infra;
pub mod protocol;
pub mod security;
pub mod storage;
pub mod workflows;

// --- Backwards-compatible facades & re-exports ---

pub use identity::accounts;
pub use identity::contacts;

pub use infra::config;
pub use infra::diagnostics;
pub use infra::logging;

pub use protocol::napcat;
pub use protocol::napcat_boot;
pub use protocol::onebot;
pub use protocol::ws_listener;

pub use security::cooldown;
pub use security::policy;
pub use security::trigger;

pub use storage::db;

pub use workflows::archive;
pub use workflows::chain;
pub use workflows::document;
pub use workflows::group_files;
pub use workflows::scheduler;
pub use workflows::summarizer;

use std::sync::Arc;
use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::napcat::NapCatService;
use crate::services::onebot::OneBotClient;

pub struct AppState {
    pub db: Arc<Database>,
    pub napcat: Arc<NapCatService>,
    pub onebot: Arc<OneBotClient>,
    pub ai: Arc<AiService>,
}

pub mod auth;
pub mod contacts;
pub mod chat;
pub mod files;
pub mod summary;
pub mod system;
pub mod window;

pub use auth::*;
pub use contacts::*;
pub use chat::*;
pub use files::*;
pub use summary::*;
pub use system::*;
pub use window::*;

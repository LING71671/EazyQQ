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

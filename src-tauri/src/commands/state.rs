use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::napcat::NapCatService;
use crate::services::onebot::OneBotClient;
use std::sync::Arc;

pub struct AppState {
    pub worker_lease: Option<Arc<crate::services::infra::persistence::FileLock>>,
    pub db: Arc<Database>,
    pub napcat: Arc<NapCatService>,
    pub onebot: Arc<OneBotClient>,
    pub ai: Arc<AiService>,
}

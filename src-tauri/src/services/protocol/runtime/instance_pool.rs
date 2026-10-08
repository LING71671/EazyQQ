use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::identity::instances::load_registry;
use crate::services::protocol::onebot::OneBotClient;

#[derive(Default)]
pub struct InstancePool {
    clients: RwLock<HashMap<String, Arc<OneBotClient>>>,
}

impl InstancePool {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_client(&self, uin: &str) -> Arc<OneBotClient> {
        {
            let lock = self.clients.read().await;
            if let Some(client) = lock.get(uin) {
                return client.clone();
            }
        }

        let reg = load_registry();
        let port = reg
            .instances
            .iter()
            .find(|i| i.uin == uin)
            .map(|i| i.http_port)
            .unwrap_or(0);

        let endpoint = format!("http://127.0.0.1:{}", port);
        let client = Arc::new(OneBotClient::for_account(endpoint, Some(uin.to_string())));

        let mut lock = self.clients.write().await;
        lock.insert(uin.to_string(), client.clone());
        client
    }

    pub async fn resolve_client(&self, explicit_uin: Option<&str>) -> Arc<OneBotClient> {
        let uin = explicit_uin
            .map(|s| s.to_string())
            .or_else(|| crate::services::identity::accounts::active())
            .unwrap_or_default();

        self.get_client(&uin).await
    }
}

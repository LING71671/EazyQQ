use std::sync::Arc;
use std::time::Duration;
use futures_util::StreamExt;
use serde_json::Value;
use tauri::AppHandle;
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

use crate::services::ai::AiService;
use crate::services::db::Database;
use super::events::handle_onebot_event;
use super::onebot::OneBotClient;

pub fn start_onebot_ws_listener(
    app_handle: AppHandle,
    ws_url: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) {
    tauri::async_runtime::spawn(async move {
        info!("Starting OneBot WebSocket listener targeting {}", ws_url);

        loop {
            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!("Successfully connected to OneBot WebSocket ({})", ws_url);
                    crate::services::chain::record_ok(
                        crate::services::chain::Link::OneBotWs,
                        "已连接，正在接收消息",
                    );
                    let (_, mut read) = ws_stream.split();

                    while let Some(msg_res) = read.next().await {
                        match msg_res {
                            Ok(msg) => {
                                if msg.is_text() {
                                    if let Ok(text) = msg.into_text() {
                                        if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                            let outcome = handle_onebot_event(
                                                Some(&app_handle),
                                                &json,
                                                &db,
                                                &onebot,
                                                &ai,
                                                false,
                                            )
                                            .await;
                                            crate::services::chain::record_ok(
                                                crate::services::chain::Link::OneBotWs,
                                                format!("已连接，最近事件: {}", outcome.action),
                                            );
                                            tracing::debug!("ws event: {}", outcome.summary());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("WebSocket stream error: {}, will reconnect...", e);
                                crate::services::chain::record_error(
                                    crate::services::chain::Link::OneBotWs,
                                    format!("连接中断: {}", e),
                                );
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("OneBot WebSocket connect failed ({}), retrying in 3s", e);
                    let is_logged_in = crate::services::accounts::active().is_some();
                    if is_logged_in {
                        crate::services::chain::record_error(
                            crate::services::chain::Link::OneBotWs,
                            format!("无法连接 {}: {}", ws_url, e),
                        );
                    } else {
                        crate::services::chain::record_unknown(
                            crate::services::chain::Link::OneBotWs,
                            format!("待扫码登录后就绪 ({})", ws_url),
                        );
                    }
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    });
}

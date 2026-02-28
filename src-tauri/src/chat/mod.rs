//! Модуль чата — подключение к стриминговым платформам.
//!
//! Содержит:
//! - `message` — единый формат `ChatMessage` для всех платформ
//! - `twitch` — Twitch IRC WebSocket клиент
//! - `kick` — Kick Pusher WebSocket клиент
//! - `auth` — Twitch OAuth авторизация
//! - `badges` — загрузка бейджей через Twitch API

pub mod auth;
pub mod badges;
pub mod kick;
pub mod message;
pub mod twitch;

use std::sync::OnceLock;

/// Глобальный HTTP клиент для Twitch API (connection pooling, таймаут 15с).
/// Создаётся один раз при первом обращении.
static TWITCH_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Получить shared HTTP клиент для Twitch API запросов.
pub fn twitch_http_client() -> &'static reqwest::Client {
    TWITCH_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client")
    })
}

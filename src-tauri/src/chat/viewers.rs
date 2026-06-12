//! Счётчик зрителей — фоновый опрос Twitch Helix и Kick API.
//!
//! Для каждого подключённого канала запускается отдельная задача,
//! которая раз в 60 секунд запрашивает число зрителей и отправляет
//! во frontend событие `viewer-count` с payload `{ platform, viewers }`.
//!
//! - Twitch: Helix `Get Streams` — работает только при OAuth авторизации.
//! - Kick: публичный API `/api/v2/channels/{slug}` (поле `livestream.viewer_count`).
//! - `viewers: null` — канал оффлайн или данные недоступны.
//! - Если настройка `show_viewer_count` выключена — API не опрашивается.

use log::{info, warn};
use serde::Deserialize;
use tauri::{Emitter, Manager};

use super::auth::get_client_id;

/// Интервал опроса (секунды).
const POLL_INTERVAL_SECS: u64 = 60;

/// Payload события `viewer-count`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ViewerCountPayload {
    pub platform: String,
    /// Число зрителей; None — оффлайн или недоступно.
    pub viewers: Option<u64>,
}

/// Проверить, включён ли счётчик зрителей в настройках.
fn is_enabled(app_handle: &tauri::AppHandle) -> bool {
    app_handle
        .try_state::<crate::config::ConfigState>()
        .and_then(|cs| cs.settings.try_lock().ok().map(|s| s.show_viewer_count))
        .unwrap_or(true)
}

/// Отправить число зрителей во frontend.
fn emit_viewers(app_handle: &tauri::AppHandle, platform: &str, viewers: Option<u64>) {
    let _ = app_handle.emit(
        "viewer-count",
        ViewerCountPayload {
            platform: platform.to_string(),
            viewers,
        },
    );
}

// ──────────────────────────────────────────────────────────
// Twitch
// ──────────────────────────────────────────────────────────

/// Ответ Helix Get Streams.
#[derive(Deserialize)]
struct StreamsResponse {
    data: Vec<StreamData>,
}

#[derive(Deserialize)]
struct StreamData {
    viewer_count: u64,
}

/// Запросить число зрителей Twitch канала через Helix API.
/// Возвращает None если канал оффлайн.
async fn fetch_twitch_viewers(token: &str, channel: &str) -> Result<Option<u64>, String> {
    let client = super::twitch_http_client();
    let resp = client
        .get("https://api.twitch.tv/helix/streams")
        .query(&[("user_login", channel)])
        .header("Authorization", format!("Bearer {}", token))
        .header("Client-Id", get_client_id())
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Helix streams: HTTP {}", resp.status()));
    }

    let streams: StreamsResponse = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;

    // Пустой data — стрим оффлайн
    Ok(streams.data.first().map(|s| s.viewer_count))
}

/// Запустить фоновый опрос зрителей Twitch.
/// Токен читается из TwitchAuth на каждом тике — подхватывает login/logout на лету.
pub fn spawn_twitch_poller(
    app_handle: tauri::AppHandle,
    channel: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Viewer poller (Twitch) запущен для #{}", channel);
        loop {
            if is_enabled(&app_handle) {
                let token = {
                    let auth: tauri::State<super::auth::TwitchAuth> = app_handle.state();
                    let t = auth.access_token.lock().await.clone();
                    t
                };
                match token {
                    Some(token) => match fetch_twitch_viewers(&token, &channel).await {
                        Ok(viewers) => emit_viewers(&app_handle, "twitch", viewers),
                        Err(e) => warn!("Viewer poller (Twitch): {}", e),
                    },
                    // Анонимное подключение — счётчик недоступен
                    None => emit_viewers(&app_handle, "twitch", None),
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    })
}

// ──────────────────────────────────────────────────────────
// Kick
// ──────────────────────────────────────────────────────────

/// Ответ Kick API v2 (только нужные поля).
#[derive(Deserialize)]
struct KickChannelViewers {
    livestream: Option<KickLivestream>,
}

#[derive(Deserialize)]
struct KickLivestream {
    #[serde(default)]
    viewer_count: Option<u64>,
    /// Некоторые версии API используют поле `viewers`
    #[serde(default)]
    viewers: Option<u64>,
}

/// Запросить число зрителей Kick канала.
/// Возвращает None если канал оффлайн.
async fn fetch_kick_viewers(client: &reqwest::Client, slug: &str) -> Result<Option<u64>, String> {
    let url = format!("https://kick.com/api/v2/channels/{}", slug);
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Referer", "https://kick.com/")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Kick API: HTTP {}", resp.status()));
    }

    let channel: KickChannelViewers = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;

    Ok(channel
        .livestream
        .and_then(|ls| ls.viewer_count.or(ls.viewers)))
}

/// Запустить фоновый опрос зрителей Kick.
pub fn spawn_kick_poller(
    app_handle: tauri::AppHandle,
    slug: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Viewer poller (Kick) запущен для {}", slug);

        // Клиент с cookie jar (обход Cloudflare, как в fetch_chatroom_id)
        let client = match reqwest::Client::builder()
            .user_agent(super::kick::BROWSER_USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .cookie_store(true)
            .use_rustls_tls()
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Viewer poller (Kick): не удалось создать HTTP клиент: {}", e);
                return;
            }
        };

        // Одноразовый визит на kick.com для получения Cloudflare cookies
        let _ = client.get("https://kick.com/").send().await;

        loop {
            if is_enabled(&app_handle) {
                match fetch_kick_viewers(&client, &slug).await {
                    Ok(viewers) => emit_viewers(&app_handle, "kick", viewers),
                    Err(e) => warn!("Viewer poller (Kick): {}", e),
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    })
}

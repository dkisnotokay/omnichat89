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

/// Отправить число зрителей во frontend и OBS overlay.
fn emit_viewers(app_handle: &tauri::AppHandle, platform: &str, viewers: Option<u64>) {
    info!("Viewer count: {} = {:?}", platform, viewers);
    let _ = app_handle.emit(
        "viewer-count",
        ViewerCountPayload {
            platform: platform.to_string(),
            viewers,
        },
    );
    // В overlay через SSE command-канал: viewers:<platform>:<число|null>
    if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
        let value = viewers.map_or("null".to_string(), |v| v.to_string());
        let _ = overlay.command_tx.send(format!("viewers:{}:{}", platform, value));
    }
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

/// Публичный Client-ID сайта twitch.tv (используется самим сайтом и
/// аналогичными приложениями). Позволяет узнать число зрителей БЕЗ авторизации.
const TWITCH_GQL_CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

/// Запросить число зрителей через внутренний GQL API Twitch (без авторизации).
/// Возвращает None если канал оффлайн или не существует.
async fn fetch_twitch_viewers_gql(channel: &str) -> Result<Option<u64>, String> {
    let client = super::twitch_http_client();
    let body = serde_json::json!({
        "query": "query($login: String!) { user(login: $login) { stream { viewersCount } } }",
        "variables": { "login": channel }
    });

    let resp = client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", TWITCH_GQL_CLIENT_ID)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("GQL HTTP error: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("GQL: HTTP {} — {}", status, snippet));
    }

    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("GQL JSON error: {}", e))?;

    let stream = &v["data"]["user"]["stream"];
    if stream.is_null() {
        return Ok(None); // оффлайн или канал не найден
    }
    Ok(stream["viewersCount"].as_u64())
}

/// Запустить фоновый опрос зрителей Twitch.
///
/// С OAuth-токеном — официальный Helix API; без токена (или при ошибке
/// Helix) — внутренний GQL API без авторизации. Токен читается из
/// TwitchAuth на каждом тике — подхватывает login/logout на лету.
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

                // Helix (официальный) при наличии токена, иначе/при ошибке — GQL
                let result = match token {
                    Some(token) => match fetch_twitch_viewers(&token, &channel).await {
                        Ok(v) => Ok(v),
                        Err(e) => {
                            warn!("Viewer poller (Twitch): Helix не сработал ({}), пробуем GQL", e);
                            fetch_twitch_viewers_gql(&channel).await
                        }
                    },
                    None => fetch_twitch_viewers_gql(&channel).await,
                };

                match result {
                    Ok(viewers) => emit_viewers(&app_handle, "twitch", viewers),
                    Err(e) => warn!("Viewer poller (Twitch): {}", e),
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

    // livestream: null → канал оффлайн; Some без числа → неожиданный формат API
    if let Some(ref ls) = channel.livestream {
        if ls.viewer_count.is_none() && ls.viewers.is_none() {
            warn!("Kick API: livestream без viewer_count/viewers — формат изменился?");
        }
    }

    Ok(channel
        .livestream
        .and_then(|ls| ls.viewer_count.or(ls.viewers)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ручная сетевая проверка Kick API:
    /// `cargo test probe_kick_viewers -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn probe_kick_viewers() {
        let client = reqwest::Client::builder()
            .user_agent(crate::chat::kick::BROWSER_USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .cookie_store(true)
            .use_rustls_tls()
            .build()
            .unwrap();
        let _ = client.get("https://kick.com/").send().await;
        let extra = std::env::var("PROBE_KICK").unwrap_or_default();
        let mut slugs = vec!["xqc", "classybeef", "garydavid"];
        if !extra.is_empty() {
            slugs.push(&extra);
        }
        for slug in slugs {
            let res = fetch_kick_viewers(&client, slug).await;
            println!("{}: {:?}", slug, res);
        }
    }

    /// Ручная сетевая проверка Twitch GQL (анонимный счётчик):
    /// `cargo test probe_twitch_gql -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn probe_twitch_gql() {
        for login in [
            "monstercat", "xqc", "dk_okay", "stray228", "jesusavgn", "evelone192",
            "buster", "zubarefff", "riotgames", "eslcs", "rainbow6", "papich",
        ] {
            let res = fetch_twitch_viewers_gql(login).await;
            println!("{}: {:?}", login, res);
        }
    }
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

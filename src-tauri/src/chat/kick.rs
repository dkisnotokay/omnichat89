//! Kick Pusher WebSocket клиент.
//!
//! Подключается к чату Kick через Pusher WebSocket протокол.
//! Не требует авторизации — чат Kick публичный.
//!
//! Протокол: Pusher WebSocket (wss://ws-us2.pusher.com)
//! Для получения chatroom_id используется Kick REST API.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde::Deserialize;
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::message::{Badge, ChatMessage, EmoteRef, Platform};
use tauri::Manager;

/// URL Pusher WebSocket сервера для Kick.
const KICK_PUSHER_URL: &str =
    "wss://ws-us2.pusher.com/app/32cbd69e4b950bf97679?protocol=7&client=js&version=8.4.0-rc2&flash=false";

/// User-Agent для HTTP запросов к Kick API (обход Cloudflare).
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Состояние подключения к Kick чату.
/// Хранится в Tauri State для управления из команд.
pub struct KickState {
    /// Флаг для остановки цикла чтения сообщений (AtomicBool — без lock contention)
    pub should_stop: Arc<AtomicBool>,
    /// Текущий канал (None если не подключен)
    pub current_channel: Arc<Mutex<Option<String>>>,
    /// Handle фоновой задачи для принудительной остановки
    pub task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Default for KickState {
    fn default() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
            current_channel: Arc::new(Mutex::new(None)),
            task_handle: Arc::new(Mutex::new(None)),
        }
    }
}

// ──────────────────────────────────────────────────────────
// Структуры для парсинга ответов Kick API
// ──────────────────────────────────────────────────────────

/// Ответ GET /api/v2/channels/{slug}
#[derive(Debug, Deserialize)]
struct KickChannelResponse {
    chatroom: KickChatroom,
}

/// Chatroom внутри ответа канала
#[derive(Debug, Deserialize)]
struct KickChatroom {
    id: u64,
}

// ──────────────────────────────────────────────────────────
// Структуры для парсинга Pusher сообщений
// ──────────────────────────────────────────────────────────

/// Конверт Pusher протокола (каждое WebSocket сообщение).
#[derive(Debug, Deserialize)]
struct PusherMessage {
    event: String,
    #[serde(default)]
    data: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    channel: Option<String>,
}

/// Данные чат-сообщения Kick (внутри data поля PusherMessage).
#[derive(Debug, Deserialize)]
struct KickChatMessageData {
    id: String,
    content: String,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    created_at: Option<String>,
    sender: KickSender,
    /// Метаданные ответа (reply) — содержит original_sender и original_message
    metadata: Option<KickReplyMetadata>,
}

/// Метаданные ответа (reply) на сообщение Kick.
#[derive(Debug, Deserialize)]
struct KickReplyMetadata {
    original_sender: Option<KickOriginalSender>,
    original_message: Option<KickOriginalMessage>,
}

/// Автор оригинального сообщения (на которое отвечают).
#[derive(Debug, Deserialize)]
struct KickOriginalSender {
    username: Option<String>,
}

/// Оригинальное сообщение (на которое отвечают).
#[derive(Debug, Deserialize)]
struct KickOriginalMessage {
    content: Option<String>,
}

/// Отправитель сообщения Kick.
#[derive(Debug, Deserialize)]
struct KickSender {
    username: String,
    slug: String,
    identity: Option<KickIdentity>,
}

/// Идентичность (визуальные атрибуты) отправителя.
#[derive(Debug, Deserialize)]
struct KickIdentity {
    color: Option<String>,
    badges: Option<Vec<KickBadge>>,
}

/// Бейдж пользователя Kick.
#[derive(Debug, Deserialize)]
struct KickBadge {
    #[serde(rename = "type")]
    badge_type: String,
    text: String,
}

/// Событие удаления сообщения Kick.
#[derive(Debug, Deserialize)]
struct KickMessageDeletedData {
    id: Option<String>,
    message: Option<KickDeletedMessageInner>,
}

/// Внутренний объект удалённого сообщения.
#[derive(Debug, Deserialize)]
struct KickDeletedMessageInner {
    id: Option<String>,
}

/// Событие бана пользователя Kick.
#[derive(Debug, Deserialize)]
struct KickUserBannedData {
    user: Option<KickBannedUser>,
}

/// Данные забаненного пользователя.
#[derive(Debug, Deserialize)]
struct KickBannedUser {
    slug: Option<String>,
    username: Option<String>,
}

/// Событие подписки Kick.
#[derive(Debug, Deserialize)]
struct KickSubscriptionData {
    username: Option<String>,
    months: Option<u32>,
}

/// Событие подарочных подписок Kick.
#[derive(Debug, Deserialize)]
struct KickGiftedSubscriptionsData {
    gifter_username: Option<String>,
    gifted_usernames: Option<Vec<String>>,
}

// ──────────────────────────────────────────────────────────
// Публичные функции
// ──────────────────────────────────────────────────────────

/// Получает chatroom_id для канала Kick.
///
/// Стратегия (с fallback):
/// 1. API v2 с cookie jar (обход Cloudflare)
/// 2. API v1 (альтернативный эндпоинт)
/// 3. Парсинг HTML страницы канала
///
/// # Аргументы
/// * `slug` — имя канала Kick (slug, например "xqc")
///
/// # Возвращает
/// `chatroom_id` (u64) или ошибку.
pub async fn fetch_chatroom_id(slug: &str) -> Result<u64, String> {
    // Валидация slug: только a-z, 0-9, _, - (до 50 символов)
    let slug_lower = slug.to_lowercase();
    if slug_lower.is_empty()
        || slug_lower.len() > 50
        || !slug_lower.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("Невалидное имя канала Kick: '{}'", slug));
    }

    info!("Kick: получение chatroom_id для канала '{}'", slug);

    // Клиент с cookie jar + rustls (другой TLS fingerprint для обхода Cloudflare)
    let client = reqwest::Client::builder()
        .user_agent(BROWSER_USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .cookie_store(true)
        .use_rustls_tls()
        .build()
        .map_err(|e| format!("Ошибка создания HTTP клиента: {}", e))?;

    // Шаг 1: Получаем Cloudflare cookies, посетив kick.com
    info!("Kick: получение cookies от kick.com...");
    let _ = client
        .get("https://kick.com/")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "none")
        .send()
        .await;

    // Шаг 2: Пробуем API v2 с cookies
    let api_url = format!("https://kick.com/api/v2/channels/{}", slug);
    info!("Kick API v2 запрос: {}", api_url);

    match try_kick_api(&client, &api_url, slug).await {
        Ok(id) => return Ok(id),
        Err(e) => {
            warn!("Kick API v2 не удался: {}", e);
        }
    }

    // Шаг 3: Пробуем API v1
    let api_v1_url = format!("https://kick.com/api/v1/channels/{}", slug);
    info!("Kick API v1 запрос: {}", api_v1_url);

    match try_kick_api(&client, &api_v1_url, slug).await {
        Ok(id) => return Ok(id),
        Err(e) => {
            warn!("Kick API v1 не удался: {}", e);
        }
    }

    // Шаг 4: Fallback — парсим HTML страницы канала
    info!("Kick: fallback — парсинг HTML страницы канала '{}'", slug);
    match try_kick_html(&client, slug).await {
        Ok(id) => return Ok(id),
        Err(e) => {
            warn!("Kick HTML fallback не удался: {}", e);
        }
    }

    Err(format!(
        "Канал '{}' не найден на Kick",
        slug
    ))
}

/// Пробует получить chatroom_id через Kick API.
async fn try_kick_api(
    client: &reqwest::Client,
    url: &str,
    slug: &str,
) -> Result<u64, String> {
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Referer", "https://kick.com/")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Таймаут (канал '{}')", slug)
            } else {
                format!("Ошибка запроса: {}", e)
            }
        })?;

    let status = response.status();
    info!("Kick API ответ: {} для '{}'", status, slug);

    if status.as_u16() == 404 || status.as_u16() == 403 {
        return Err(format!("Канал '{}' не найден на Kick", slug));
    }

    if !status.is_success() {
        return Err(format!("Kick API ошибка: HTTP {}", status));
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Ошибка чтения: {}", e))?;

    let channel: KickChannelResponse = serde_json::from_str(&body_text)
        .map_err(|e| format!("Ошибка парсинга: {}", e))?;

    info!(
        "Kick chatroom_id для '{}': {} (через API)",
        slug, channel.chatroom.id
    );
    Ok(channel.chatroom.id)
}

/// Fallback: извлекает chatroom_id из HTML страницы канала.
/// Ищет паттерн "chatroom":{"id":NNN в HTML/JS коде страницы.
async fn try_kick_html(
    client: &reqwest::Client,
    slug: &str,
) -> Result<u64, String> {
    let page_url = format!("https://kick.com/{}", slug);

    let response = client
        .get(&page_url)
        .header("Accept", "text/html,application/xhtml+xml")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "none")
        .send()
        .await
        .map_err(|e| format!("Ошибка загрузки страницы: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} для страницы канала", response.status()));
    }

    // Ограничение размера ответа (5MB max, defense-in-depth)
    let content_length = response.content_length().unwrap_or(0);
    if content_length > 5 * 1024 * 1024 {
        return Err(format!("Ответ слишком большой: {} байт", content_length));
    }

    let html = response
        .text()
        .await
        .map_err(|e| format!("Ошибка чтения HTML: {}", e))?;

    if html.len() > 5 * 1024 * 1024 {
        return Err("HTML страница слишком большая".to_string());
    }

    // Ищем паттерн "chatroom":{"id":NNN
    // Может быть в __NEXT_DATA__ или inline скриптах
    let patterns = [
        "\"chatroom\":{\"id\":",
        "\"chatroom_id\":",
        "chatroom\":{\"id\":",
    ];

    for pattern in &patterns {
        if let Some(start) = html.find(pattern) {
            let after = &html[start + pattern.len()..];
            // Извлекаем число
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(id) = num_str.parse::<u64>() {
                if id > 0 {
                    info!(
                        "Kick chatroom_id для '{}': {} (из HTML)",
                        slug, id
                    );
                    return Ok(id);
                }
            }
        }
    }

    Err("chatroom_id не найден в HTML".to_string())
}

/// Подключается к чату Kick через Pusher WebSocket и слушает сообщения.
///
/// Отправляет каждое сообщение чата во frontend через Tauri event `chat-message`.
/// Использует Pusher протокол: subscribe, ping/pong.
///
/// # Аргументы
/// * `channel` — имя канала Kick (slug)
/// * `chatroom_id` — ID chatroom из API
/// * `app_handle` — хэндл Tauri для отправки событий
/// * `should_stop` — флаг для остановки
pub async fn connect_and_listen(
    channel: String,
    chatroom_id: u64,
    app_handle: tauri::AppHandle,
    should_stop: Arc<AtomicBool>,
) {
    // Сбрасываем флаг остановки
    should_stop.store(false, Ordering::Relaxed);

    let channel_lower = channel.to_lowercase();
    info!(
        "Подключение к Kick каналу: {} (chatroom_id: {})",
        channel_lower, chatroom_id
    );

    // Подключаемся к Pusher WebSocket
    let ws_stream = match connect_async(KICK_PUSHER_URL).await {
        Ok((stream, _)) => {
            info!("WebSocket подключен к Kick Pusher");
            stream
        }
        Err(e) => {
            error!("Ошибка подключения к Kick Pusher: {}", e);
            let _ = app_handle.emit(
                "kick-chat-error",
                format!("Ошибка подключения: {}", e),
            );
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Ждём connection_established от Pusher
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(10));
    tokio::pin!(timeout);

    let connected = loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(pusher_msg) = serde_json::from_str::<PusherMessage>(&text) {
                            if pusher_msg.event == "pusher:connection_established" {
                                info!("Kick Pusher: connection_established");
                                break true;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("Ошибка чтения Pusher WebSocket: {}", e);
                        let _ = app_handle.emit("kick-chat-error", format!("Ошибка WebSocket: {}", e));
                        return;
                    }
                    None => {
                        error!("Kick Pusher WebSocket поток завершился до connection_established");
                        let _ = app_handle.emit("kick-chat-error", "Соединение прервано");
                        return;
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                error!("Таймаут ожидания connection_established от Kick Pusher");
                let _ = app_handle.emit("kick-chat-error", "Таймаут подключения к Kick");
                break false;
            }
        }
    };

    if !connected {
        return;
    }

    // Подписываемся на канал чата
    let subscribe_msg = serde_json::json!({
        "event": "pusher:subscribe",
        "data": {
            "channel": format!("chatrooms.{}.v2", chatroom_id)
        }
    });

    if let Err(e) = write
        .send(Message::Text(subscribe_msg.to_string().into()))
        .await
    {
        error!("Ошибка отправки subscribe: {}", e);
        let _ = app_handle.emit("kick-chat-error", format!("Ошибка подписки: {}", e));
        return;
    }

    info!("Kick: подписались на chatrooms.{}.v2", chatroom_id);

    // Подписываемся на канал без .v2 (модерационные события могут приходить сюда)
    let subscribe_msg2 = serde_json::json!({
        "event": "pusher:subscribe",
        "data": {
            "channel": format!("chatrooms.{}", chatroom_id)
        }
    });

    if let Err(e) = write
        .send(Message::Text(subscribe_msg2.to_string().into()))
        .await
    {
        warn!("Kick: ошибка подписки на chatrooms.{}: {}", chatroom_id, e);
    } else {
        info!("Kick: подписались на chatrooms.{}", chatroom_id);
    }

    // Уведомляем frontend о подключении
    let _ = app_handle.emit("kick-chat-connected", &channel_lower);

    // Основной цикл чтения сообщений
    loop {
        // Проверяем флаг остановки (AtomicBool — без блокировки)
        if should_stop.load(Ordering::Relaxed) {
            info!("Остановка чтения Kick чата по запросу");
            break;
        }

        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_pusher_message(&text, &channel_lower, &app_handle, &mut write).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        warn!("Kick Pusher соединение закрыто сервером");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("Ошибка чтения Kick Pusher WebSocket: {}", e);
                        break;
                    }
                    None => {
                        warn!("Kick Pusher поток завершился");
                        break;
                    }
                    _ => {} // Игнорируем Binary, Ping, Pong
                }
            }
            // Таймаут 1 секунда для проверки флага остановки
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
        }
    }

    // Уведомляем frontend об отключении
    let _ = app_handle.emit("kick-chat-disconnected", &channel_lower);
    info!("Отключились от Kick канала {}", channel_lower);
}

// ──────────────────────────────────────────────────────────
// Внутренние функции
// ──────────────────────────────────────────────────────────

/// Получает текущий язык из настроек приложения.
fn get_language(app_handle: &tauri::AppHandle) -> String {
    app_handle
        .try_state::<crate::config::ConfigState>()
        .and_then(|config_state| {
            config_state
                .settings
                .try_lock()
                .ok()
                .map(|s| s.language.clone())
        })
        .unwrap_or_else(|| "ru".to_string())
}

/// Обрабатывает одно Pusher сообщение.
async fn handle_pusher_message<S>(
    raw: &str,
    channel: &str,
    app_handle: &tauri::AppHandle,
    write: &mut futures_util::stream::SplitSink<S, Message>,
) where
    S: futures_util::Sink<Message> + Unpin,
{
    let pusher_msg: PusherMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(_) => return,
    };

    match pusher_msg.event.as_str() {
        // Keepalive: Pusher пингует нас
        "pusher:ping" => {
            let pong = serde_json::json!({"event": "pusher:pong", "data": {}});
            let _ = write
                .send(Message::Text(pong.to_string().into()))
                .await;
        }

        // Новое сообщение в чате
        "App\\Events\\ChatMessageEvent" | "App\\Events\\ChatMessageSentEvent" => {
            // data — JSON строка внутри JSON (Pusher протокол)
            let data_str = match pusher_msg.data.as_str() {
                Some(s) => s.to_string(),
                None => match serde_json::to_string(&pusher_msg.data) {
                    Ok(s) => s,
                    Err(_) => return,
                },
            };

            if let Some(chat_msg) = parse_kick_message(&data_str, channel) {
                let _ = app_handle.emit("chat-message", &chat_msg);
                // Отправляем в OBS overlay через broadcast
                if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
                    let _ = overlay.chat_tx.send(chat_msg.clone());
                }
                crate::tts::try_enqueue(app_handle, &chat_msg);
            }
        }

        // Удаление сообщения
        "App\\Events\\MessageDeletedEvent" => {
            let data_str = match pusher_msg.data.as_str() {
                Some(s) => s.to_string(),
                None => match serde_json::to_string(&pusher_msg.data) {
                    Ok(s) => s,
                    Err(_) => return,
                },
            };

            if let Ok(deleted) = serde_json::from_str::<KickMessageDeletedData>(&data_str) {
                // message.id — ID удалённого сообщения, deleted.id — ID самого события удаления
                let msg_id = deleted
                    .message.and_then(|m| m.id)
                    .or(deleted.id);
                if let Some(id) = msg_id {
                    info!("Kick: удаление сообщения {}", id);
                    let _ = app_handle.emit("chat-msg-deleted", id.clone());
                    // Отправляем в OBS overlay
                    if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
                        let _ = overlay.command_tx.send(format!("delete:{}", id));
                    }
                }
            }
        }

        // Очистка чата (бан пользователя)
        "App\\Events\\UserBannedEvent" => {
            let data_str = match pusher_msg.data.as_str() {
                Some(s) => s.to_string(),
                None => match serde_json::to_string(&pusher_msg.data) {
                    Ok(s) => s,
                    Err(_) => return,
                },
            };

            if let Ok(banned) = serde_json::from_str::<KickUserBannedData>(&data_str) {
                let username = banned
                    .user
                    .and_then(|u| u.slug.or(u.username));
                if let Some(name) = username {
                    info!("Kick: пользователь забанен — {}", name);
                    let _ = app_handle.emit("chat-user-cleared", &name);
                    // Отправляем в OBS overlay
                    if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
                        let _ = overlay.command_tx.send(format!("clear_user:{}", name));
                    }
                } else {
                    info!("Kick: пользователь забанен (без имени)");
                }
            }
        }

        // Очистка всего чата
        "App\\Events\\ChatroomClearEvent" => {
            info!("Kick: полная очистка чата");
            let _ = app_handle.emit("chat-cleared", "");
            // Отправляем в OBS overlay
            if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
                let _ = overlay.command_tx.send("clear".to_string());
            }
        }

        // Подписка на канал
        "App\\Events\\SubscriptionEvent" => {
            let data_str = match pusher_msg.data.as_str() {
                Some(s) => s.to_string(),
                None => match serde_json::to_string(&pusher_msg.data) {
                    Ok(s) => s,
                    Err(_) => return,
                },
            };

            if let Ok(sub) = serde_json::from_str::<KickSubscriptionData>(&data_str) {
                let username = sub.username.unwrap_or_else(|| "???".to_string());
                let months = sub.months.unwrap_or(1);
                let lang = get_language(app_handle);
                let is_ru = lang == "ru";
                let (event_type, system_message) = if months > 1 {
                    (
                        "resub",
                        if is_ru {
                            format!("{} переподписался на {} мес.!", username, months)
                        } else {
                            format!("{} resubscribed for {} months!", username, months)
                        },
                    )
                } else {
                    (
                        "sub",
                        if is_ru {
                            format!("{} подписался на канал!", username)
                        } else {
                            format!("{} subscribed!", username)
                        },
                    )
                };
                info!("Kick: {} — {}", event_type, system_message);

                let chat_msg = ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    platform: Platform::Kick,
                    username: username.to_lowercase(),
                    display_name: username,
                    color: None,
                    badges: Vec::new(),
                    message: String::new(),
                    emotes: Vec::new(),
                    timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                    channel: channel.to_string(),
                    reply_to: None,
                    reply_text: None,
                    event_type: Some(event_type.to_string()),
                    system_message: Some(system_message),
                };
                let _ = app_handle.emit("chat-message", &chat_msg);
                if let Some(overlay) =
                    app_handle.try_state::<crate::overlay::OverlayState>()
                {
                    let _ = overlay.chat_tx.send(chat_msg.clone());
                }
                crate::tts::try_enqueue(app_handle, &chat_msg);
            }
        }

        // Подарочные подписки
        "App\\Events\\GiftedSubscriptionsEvent" => {
            let data_str = match pusher_msg.data.as_str() {
                Some(s) => s.to_string(),
                None => match serde_json::to_string(&pusher_msg.data) {
                    Ok(s) => s,
                    Err(_) => return,
                },
            };

            if let Ok(gift) = serde_json::from_str::<KickGiftedSubscriptionsData>(&data_str) {
                let gifter = gift
                    .gifter_username
                    .unwrap_or_else(|| "???".to_string());
                let count = gift.gifted_usernames.map(|v| v.len()).unwrap_or(1);
                let lang = get_language(app_handle);
                let system_message = if lang == "ru" {
                    format!("{} подарил {} подписок!", gifter, count)
                } else {
                    format!("{} gifted {} subs!", gifter, count)
                };
                info!("Kick: submysterygift — {}", system_message);

                let chat_msg = ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    platform: Platform::Kick,
                    username: gifter.to_lowercase(),
                    display_name: gifter,
                    color: None,
                    badges: Vec::new(),
                    message: String::new(),
                    emotes: Vec::new(),
                    timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                    channel: channel.to_string(),
                    reply_to: None,
                    reply_text: None,
                    event_type: Some("submysterygift".to_string()),
                    system_message: Some(system_message),
                };
                let _ = app_handle.emit("chat-message", &chat_msg);
                if let Some(overlay) =
                    app_handle.try_state::<crate::overlay::OverlayState>()
                {
                    let _ = overlay.chat_tx.send(chat_msg.clone());
                }
                crate::tts::try_enqueue(app_handle, &chat_msg);
            }
        }

        // Подтверждение подписки
        "pusher_internal:subscription_succeeded" => {
            info!("Kick: подписка на канал подтверждена");
        }

        // Остальные события игнорируем
        _ => {}
    }
}

/// Парсит данные сообщения Kick в единый формат `ChatMessage`.
///
/// # Аргументы
/// * `data_str` — JSON строка с данными сообщения
/// * `channel` — имя канала Kick
fn parse_kick_message(data_str: &str, channel: &str) -> Option<ChatMessage> {
    let data: KickChatMessageData = serde_json::from_str(data_str).ok()?;

    // Пропускаем системные сообщения
    if let Some(ref t) = data.msg_type {
        if t != "message" && t != "reply" {
            return None;
        }
    }

    let username = data.sender.slug.clone();
    let display_name = data.sender.username.clone();

    // Цвет ника
    let color = data
        .sender
        .identity
        .as_ref()
        .and_then(|i| i.color.clone())
        .filter(|c| !c.is_empty());

    // Бейджи
    let badges = data
        .sender
        .identity
        .as_ref()
        .and_then(|i| i.badges.as_ref())
        .map(|badges| {
            badges
                .iter()
                .map(|b| Badge {
                    id: b.badge_type.clone(),
                    version: "1".to_string(),
                    image_url: String::new(),
                    title: b.text.clone(),
                })
                .collect::<Vec<Badge>>()
        })
        .unwrap_or_default();

    // Извлекаем эмоуты из Kick сообщения
    // Kick использует формат [emote:ID:code] в тексте — нужно извлечь
    let (clean_message, emotes) = extract_kick_emotes(&data.content);

    // Временная метка
    let timestamp = data
        .created_at
        .as_ref()
        .and_then(|ts| {
            // Формат: "2024-01-01T00:00:00.000000Z"
            chrono_parse_timestamp(ts)
        })
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    // Извлекаем reply info из metadata (если это ответ на сообщение)
    let (reply_to, reply_text) = if data.msg_type.as_deref() == Some("reply") {
        if let Some(ref meta) = data.metadata {
            let author = meta
                .original_sender
                .as_ref()
                .and_then(|s| s.username.clone());
            let text = meta
                .original_message
                .as_ref()
                .and_then(|m| m.content.clone());
            (author, text)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Some(ChatMessage {
        id: data.id,
        platform: Platform::Kick,
        username,
        display_name,
        color,
        badges,
        message: clean_message,
        emotes,
        timestamp,
        channel: channel.to_string(),
        reply_to,
        reply_text,
        event_type: None,
        system_message: None,
    })
}

/// Извлекает эмоуты из текста Kick сообщения.
///
/// Kick использует формат `[emote:123456:EmoteName]` в тексте.
/// Функция возвращает очищенный текст и список эмоутов с позициями.
fn extract_kick_emotes(content: &str) -> (String, Vec<EmoteRef>) {
    let mut clean = String::new();
    let mut emotes = Vec::new();
    let mut chars = content.char_indices().peekable();

    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '[' {
            // Попробуем найти [emote:ID:code]
            if let Some(end_rel) = content[byte_pos..].find(']') {
                let bracket_content = &content[byte_pos + 1..byte_pos + end_rel];
                let parts: Vec<&str> = bracket_content.splitn(3, ':').collect();

                if parts.len() == 3 && parts[0] == "emote" {
                    let emote_id = parts[1];
                    let emote_code = parts[2];

                    // Валидация emote ID: только цифры (defense-in-depth)
                    if !emote_id.chars().all(|c| c.is_ascii_digit()) {
                        clean.push('[');
                        continue;
                    }

                    // Позиция в chars (Unicode-корректная)
                    let char_start = clean.chars().count();
                    clean.push_str(emote_code);
                    let char_end = clean.chars().count().saturating_sub(1);

                    // URL эмоута Kick
                    let url = format!(
                        "https://files.kick.com/emotes/{}/fullsize",
                        emote_id
                    );

                    emotes.push(EmoteRef {
                        id: emote_id.to_string(),
                        code: emote_code.to_string(),
                        url,
                        start: char_start,
                        end: char_end,
                    });

                    // Пропускаем до ']' включительно
                    let skip_to = byte_pos + end_rel + 1;
                    while let Some(&(next_pos, _)) = chars.peek() {
                        if next_pos >= skip_to {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                }
            }
            // Не эмоут — копируем как есть
            clean.push('[');
        } else {
            clean.push(ch);
        }
    }

    (clean, emotes)
}

/// Парсит ISO 8601 timestamp в Unix миллисекунды.
/// Простой парсер без внешних зависимостей.
fn chrono_parse_timestamp(ts: &str) -> Option<i64> {
    // Формат: "2024-01-15T12:30:45.123456Z" или "2024-01-15T12:30:45Z"
    let ts = ts.trim_end_matches('Z');
    let parts: Vec<&str> = ts.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }

    let year: i64 = date_parts[0].parse().ok()?;
    let month: i64 = date_parts[1].parse().ok()?;
    let day: i64 = date_parts[2].parse().ok()?;

    let time_and_frac: Vec<&str> = parts[1].splitn(2, '.').collect();
    let time_parts: Vec<&str> = time_and_frac[0].split(':').collect();
    if time_parts.len() != 3 {
        return None;
    }

    let hour: i64 = time_parts[0].parse().ok()?;
    let min: i64 = time_parts[1].parse().ok()?;
    let sec: i64 = time_parts[2].parse().ok()?;

    // Упрощённый расчёт Unix timestamp (не учитывает високосные секунды)
    let days = days_from_civil(year, month, day);
    let timestamp_secs = days * 86400 + hour * 3600 + min * 60 + sec;

    Some(timestamp_secs * 1000) // в миллисекундах
}

/// Конвертирует дату (year, month, day) в количество дней с Unix epoch.
/// Алгоритм из Howard Hinnant's date algorithms.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let d = d as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

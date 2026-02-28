//! Twitch IRC WebSocket клиент.
//!
//! Подключается анонимно (justinfan) к Twitch IRC через WebSocket.
//! Не требует авторизации для чтения сообщений чата.
//!
//! Протокол: IRC over WebSocket (wss://irc-ws.chat.twitch.tv:443)
//! Документация: https://dev.twitch.tv/docs/irc/

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::message::{Badge, ChatMessage, EmoteRef, Platform};
use tauri::Manager;

/// URL Twitch IRC WebSocket сервера.
const TWITCH_IRC_URL: &str = "wss://irc-ws.chat.twitch.tv:443";

/// Состояние подключения к Twitch IRC.
/// Хранится в Tauri State для управления из команд.
pub struct TwitchState {
    /// Флаг для остановки цикла чтения сообщений (AtomicBool — без lock contention)
    pub should_stop: Arc<AtomicBool>,
    /// Текущий канал (None если не подключен)
    pub current_channel: Arc<Mutex<Option<String>>>,
    /// Handle фоновой задачи для принудительной остановки
    pub task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Default for TwitchState {
    fn default() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
            current_channel: Arc::new(Mutex::new(None)),
            task_handle: Arc::new(Mutex::new(None)),
        }
    }
}

// Валидация каналов удалена — проверка через ROOMSTATE при подключении к IRC.
// GQL API с публичным Client-ID ненадёжен.

/// Подключается к каналу Twitch и начинает слушать сообщения.
///
/// Отправляет каждое сообщение чата во frontend через Tauri event `chat-message`.
/// Автоматически отвечает на PING от сервера (keepalive).
///
/// # Аргументы
/// * `channel` — имя канала Twitch (без #)
/// * `app_handle` — хэндл Tauri для отправки событий
/// * `should_stop` — флаг для остановки
/// * `auth_token` — OAuth токен (None для анонимного подключения)
/// * `user_login` — логин пользователя (нужен при авторизованном подключении)
pub async fn connect_and_listen(
    channel: String,
    app_handle: tauri::AppHandle,
    should_stop: Arc<AtomicBool>,
    auth_token: Option<String>,
    user_login: Option<String>,
) {
    // Сбрасываем флаг остановки
    should_stop.store(false, Ordering::Relaxed);

    let channel_lower = channel.to_lowercase();

    // Валидация имени канала: только a-z, 0-9, _ (до 25 символов)
    if channel_lower.is_empty()
        || channel_lower.len() > 25
        || !channel_lower.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        error!("Невалидное имя канала Twitch: '{}'", channel_lower);
        let _ = app_handle.emit("chat-error", format!("Невалидное имя канала: '{}'", channel_lower));
        return;
    }

    info!("Подключение к Twitch каналу: #{}", channel_lower);

    // Подключаемся к WebSocket
    let ws_stream = match connect_async(TWITCH_IRC_URL).await {
        Ok((stream, _)) => {
            info!("WebSocket подключен к Twitch IRC");
            stream
        }
        Err(e) => {
            error!("Ошибка подключения к Twitch IRC: {}", e);
            let _ = app_handle.emit("chat-error", format!("Ошибка подключения: {}", e));
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Запрашиваем теги IRC v3 (цвета, бейджи, эмоуты)
    let cap_msg = Message::Text("CAP REQ :twitch.tv/tags twitch.tv/commands\r\n".into());
    if let Err(e) = write.send(cap_msg).await {
        error!("Ошибка отправки CAP REQ: {}", e);
        return;
    }

    // Авторизация: OAuth токен или анонимный логин
    if let (Some(token), Some(login)) = (&auth_token, &user_login) {
        info!("Авторизованное подключение: {}", login);
        let pass_msg = Message::Text(format!("PASS oauth:{}\r\n", token).into());
        if let Err(e) = write.send(pass_msg).await {
            error!("Ошибка отправки PASS: {}", e);
            return;
        }
        let nick_msg = Message::Text(format!("NICK {}\r\n", login).into());
        if let Err(e) = write.send(nick_msg).await {
            error!("Ошибка отправки NICK: {}", e);
            return;
        }
    } else {
        info!("Анонимное подключение (justinfan)");
        let nick_msg = Message::Text("NICK justinfan12345\r\n".into());
        if let Err(e) = write.send(nick_msg).await {
            error!("Ошибка отправки NICK: {}", e);
            return;
        }
    }

    // Присоединяемся к каналу
    let join_msg = Message::Text(format!("JOIN #{}\r\n", channel_lower).into());
    if let Err(e) = write.send(join_msg).await {
        error!("Ошибка отправки JOIN: {}", e);
        return;
    }

    // Ждём ROOMSTATE — Twitch отправляет его только для существующих каналов.
    // Если за 5 сек не получили — канал не существует.
    info!("Ожидание ROOMSTATE от #{}", channel_lower);
    let roomstate_timeout = tokio::time::sleep(std::time::Duration::from_secs(5));
    tokio::pin!(roomstate_timeout);
    let mut got_roomstate = false;

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        for line in text.lines() {
                            if line.starts_with("PING") {
                                let pong = Message::Text("PONG :tmi.twitch.tv\r\n".into());
                                let _ = write.send(pong).await;
                            } else if line.contains("ROOMSTATE") {
                                info!("ROOMSTATE получен для #{}", channel_lower);
                                got_roomstate = true;
                            }
                        }
                        if got_roomstate {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        error!("Ошибка чтения WebSocket при ожидании ROOMSTATE: {}", e);
                        let _ = app_handle.emit("chat-error", format!("Ошибка: {}", e));
                        return;
                    }
                    None => {
                        error!("Twitch IRC поток завершился при ожидании ROOMSTATE");
                        let _ = app_handle.emit("chat-error", "Соединение прервано");
                        return;
                    }
                    _ => {}
                }
            }
            _ = &mut roomstate_timeout => {
                warn!("Таймаут ROOMSTATE для #{} — канал не существует", channel_lower);
                let _ = app_handle.emit(
                    "chat-error",
                    format!("Канал '{}' не найден на Twitch", channel_lower),
                );
                return;
            }
        }
    }

    // Уведомляем frontend о подключении
    let _ = app_handle.emit("chat-connected", &channel_lower);
    info!("Присоединились к каналу #{}", channel_lower);

    // Основной цикл чтения сообщений
    loop {
        // Проверяем флаг остановки (AtomicBool — без блокировки)
        if should_stop.load(Ordering::Relaxed) {
            info!("Остановка чтения чата по запросу");
            break;
        }

        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Обрабатываем каждую строку IRC (может быть несколько в одном сообщении)
                        for line in text.lines() {
                            if line.starts_with("PING") {
                                // Отвечаем на PING для поддержания соединения
                                let pong = Message::Text("PONG :tmi.twitch.tv\r\n".into());
                                let _ = write.send(pong).await;
                            } else if line.contains("PRIVMSG") {
                                // Парсим сообщение чата
                                if let Some(chat_msg) = parse_privmsg(line, &channel_lower) {
                                    dispatch_chat_message(&app_handle, &chat_msg).await;
                                }
                            } else if line.contains("USERNOTICE") {
                                // Системные события: подписки, рейды, подарки и т.д.
                                let lang = get_language(&app_handle);
                                if let Some(chat_msg) = parse_usernotice(line, &channel_lower, &lang) {
                                    dispatch_chat_message(&app_handle, &chat_msg).await;
                                }
                            } else if line.contains("CLEARCHAT") {
                                // Модерация: очистка сообщений пользователя или всего чата
                                handle_clearchat(line, &app_handle);
                            } else if line.contains("CLEARMSG") {
                                // Модерация: удаление конкретного сообщения
                                handle_clearmsg(line, &app_handle);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        warn!("Twitch IRC соединение закрыто сервером");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("Ошибка чтения WebSocket: {}", e);
                        break;
                    }
                    None => {
                        warn!("Twitch IRC поток завершился");
                        break;
                    }
                    _ => {} // Игнорируем Binary, Ping, Pong, Frame
                }
            }
            // Таймаут 1 секунда для проверки флага остановки
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
        }
    }

    // Уведомляем frontend об отключении
    let _ = app_handle.emit("chat-disconnected", &channel_lower);
    info!("Отключились от канала #{}", channel_lower);
}

/// Парсит IRC PRIVMSG в структуру `ChatMessage`.
///
/// Формат IRC сообщения с тегами:
/// `@badge-info=...;badges=...;color=#FF0000;display-name=User;emotes=...;id=...
///  :user!user@user.tmi.twitch.tv PRIVMSG #channel :message text`
///
/// # Аргументы
/// * `raw` — сырая IRC строка
/// * `channel` — имя канала (для заполнения поля channel)
fn parse_privmsg(raw: &str, channel: &str) -> Option<ChatMessage> {
    // Разделяем теги и основное сообщение
    // Формат: @tags :prefix PRIVMSG #channel :message
    let (tags_str, rest) = if raw.starts_with('@') {
        let space_idx = raw.find(' ')?;
        (&raw[1..space_idx], &raw[space_idx + 1..])
    } else {
        ("", raw)
    };

    // Находим PRIVMSG
    let privmsg_idx = rest.find("PRIVMSG")?;

    // Извлекаем username из prefix (:username!username@username.tmi.twitch.tv)
    let prefix = &rest[..privmsg_idx].trim();
    let username = if prefix.starts_with(':') {
        prefix[1..].split('!').next().unwrap_or("unknown")
    } else {
        "unknown"
    };

    // Извлекаем текст сообщения (после второго ':')
    let after_privmsg = &rest[privmsg_idx..];
    let msg_start = after_privmsg.find(':').map(|i| i + 1)?;
    let raw_message = after_privmsg[msg_start..].trim_end().to_string();

    // Обработка /me (ACTION): IRC отправляет как \x01ACTION текст\x01
    let (message, is_action) = if raw_message.starts_with("\x01ACTION ") && raw_message.ends_with('\x01') {
        let text = raw_message[8..raw_message.len() - 1].to_string();
        (text, true)
    } else {
        (raw_message, false)
    };

    // Парсим теги IRC
    let tags = parse_tags(tags_str);

    // Извлекаем display-name
    let display_name = tags
        .iter()
        .find(|(k, _)| *k == "display-name")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| username.to_string());

    // Извлекаем цвет ника
    let color = tags
        .iter()
        .find(|(k, _)| *k == "color")
        .and_then(|(_, v)| if v.is_empty() { None } else { Some(v.to_string()) });

    // Извлекаем бейджи
    let badges = parse_badges(&tags);

    // Извлекаем эмоуты
    let emotes = parse_emotes(&tags, &message);

    // Временная метка
    let timestamp = tags
        .iter()
        .find(|(k, _)| *k == "tmi-sent-ts")
        .and_then(|(_, v)| v.parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    // ID сообщения
    let id = tags
        .iter()
        .find(|(k, _)| *k == "id")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Извлекаем данные ответа (reply)
    // Twitch IRC теги: reply-parent-display-name, reply-parent-msg-body
    let reply_to = tags
        .iter()
        .find(|(k, _)| *k == "reply-parent-display-name")
        .map(|(_, v)| v.to_string())
        .filter(|v| !v.is_empty());

    let reply_text = tags
        .iter()
        .find(|(k, _)| *k == "reply-parent-msg-body")
        .map(|(_, v)| {
            // Twitch экранирует пробелы как \s в тегах
            v.replace("\\s", " ")
                .replace("\\n", " ")
                .replace("\\r", "")
                .replace("\\\\", "\\")
        })
        .filter(|v| !v.is_empty());

    // Детектируем выделенное сообщение (Channel Points) или /me (ACTION)
    let event_type = if is_action {
        Some("action".to_string())
    } else {
        tags.iter()
            .find(|(k, _)| *k == "msg-id")
            .and_then(|(_, v)| {
                if *v == "highlighted-message" {
                    Some("highlighted".to_string())
                } else {
                    None
                }
            })
    };

    Some(ChatMessage {
        id,
        platform: Platform::Twitch,
        username: username.to_string(),
        display_name,
        color,
        badges,
        message,
        emotes,
        timestamp,
        channel: channel.to_string(),
        reply_to,
        reply_text,
        event_type,
        system_message: None,
    })
}

/// Парсит строку тегов IRC в вектор пар (ключ, значение).
///
/// Формат: `key1=value1;key2=value2;key3=value3`
fn parse_tags(tags_str: &str) -> Vec<(&str, &str)> {
    if tags_str.is_empty() {
        return Vec::new();
    }
    tags_str
        .split(';')
        .filter_map(|tag| {
            let mut parts = tag.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((key, value))
        })
        .collect()
}

/// Отправляет ChatMessage во frontend, OBS overlay (с resolved badge URLs) и TTS.
async fn dispatch_chat_message(app_handle: &tauri::AppHandle, chat_msg: &ChatMessage) {
    let _ = app_handle.emit("chat-message", chat_msg);
    // Отправляем в OBS overlay через broadcast (с resolved badge URLs)
    if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
        let mut msg_for_overlay = chat_msg.clone();
        {
            let bmap = overlay.badge_map.lock().await;
            for badge in &mut msg_for_overlay.badges {
                if badge.image_url.is_empty() {
                    let key = format!("{}/{}", badge.id, badge.version);
                    if let Some(entry) = bmap.get(&key) {
                        badge.image_url = entry.image_url.clone();
                    }
                }
            }
        }
        let _ = overlay.chat_tx.send(msg_for_overlay);
    }
    crate::tts::try_enqueue(app_handle, chat_msg);
}

/// Получает текущий язык из настроек приложения.
fn get_language(app_handle: &tauri::AppHandle) -> String {
    app_handle
        .try_state::<crate::config::ConfigState>()
        .and_then(|config_state| {
            config_state.settings.try_lock().ok().map(|s| s.language.clone())
        })
        .unwrap_or_else(|| "ru".to_string())
}

/// Парсит IRC USERNOTICE в структуру `ChatMessage`.
///
/// USERNOTICE — системные события: подписки, рейды, подарки и т.д.
/// Тег `msg-id` определяет тип события, `system-msg` содержит текст.
/// Пользователь может добавить свой текст (после последнего `:`).
fn parse_usernotice(raw: &str, channel: &str, lang: &str) -> Option<ChatMessage> {
    // Парсим теги
    let (tags_str, rest) = if raw.starts_with('@') {
        let space_idx = raw.find(' ')?;
        (&raw[1..space_idx], &raw[space_idx + 1..])
    } else {
        ("", raw)
    };

    let tags = parse_tags(tags_str);

    // Тип события
    let msg_id = tags
        .iter()
        .find(|(k, _)| *k == "msg-id")
        .map(|(_, v)| *v)
        .unwrap_or("");

    let event_type = match msg_id {
        "sub" | "resub" | "subgift" | "submysterygift" | "raid" | "announcement"
        | "viewermilestone" => msg_id.to_string(),
        _ => return None, // Неизвестный USERNOTICE, пропускаем
    };

    info!("USERNOTICE: {} в #{}", event_type, channel);

    // Системный текст (локализованный)
    let system_message = build_system_message(&tags, msg_id, lang);

    // Username из тега login (USERNOTICE использует login, не prefix)
    let username = tags
        .iter()
        .find(|(k, _)| *k == "login")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let display_name = tags
        .iter()
        .find(|(k, _)| *k == "display-name")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| username.clone());

    let color = tags
        .iter()
        .find(|(k, _)| *k == "color")
        .and_then(|(_, v)| if v.is_empty() { None } else { Some(v.to_string()) });

    let badges = parse_badges(&tags);

    // Пользовательское сообщение (после USERNOTICE #channel :text)
    let user_message = rest
        .find("USERNOTICE")
        .and_then(|idx| {
            let after = &rest[idx..];
            // Находим #channel, потом ищем : после него
            after.find('#').and_then(|hash_idx| {
                after[hash_idx..].find(':').map(|colon_idx| {
                    after[hash_idx + colon_idx + 1..].trim_end().to_string()
                })
            })
        })
        .filter(|s| !s.is_empty());

    let emotes = if let Some(ref msg) = user_message {
        parse_emotes(&tags, msg)
    } else {
        Vec::new()
    };

    let timestamp = tags
        .iter()
        .find(|(k, _)| *k == "tmi-sent-ts")
        .and_then(|(_, v)| v.parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    let id = tags
        .iter()
        .find(|(k, _)| *k == "id")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    Some(ChatMessage {
        id,
        platform: Platform::Twitch,
        username: username.to_string(),
        display_name,
        color,
        badges,
        message: user_message.unwrap_or_default(),
        emotes,
        timestamp,
        channel: channel.to_string(),
        reply_to: None,
        reply_text: None,
        event_type: Some(event_type),
        system_message,
    })
}

/// Формирует локализованный системный текст из тегов USERNOTICE.
fn build_system_message(tags: &[(&str, &str)], msg_id: &str, lang: &str) -> Option<String> {
    let display_name = tags
        .iter()
        .find(|(k, _)| *k == "display-name")
        .map(|(_, v)| *v)
        .unwrap_or("???");

    let is_ru = lang == "ru";

    match msg_id {
        "sub" => {
            let plan = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-sub-plan")
                .map(|(_, v)| *v)
                .unwrap_or("1000");
            let tier = match plan {
                "Prime" => "Twitch Prime",
                "2000" => "Tier 2",
                "3000" => "Tier 3",
                _ => "Tier 1",
            };
            if is_ru {
                Some(format!("{} подписался на канал! ({})", display_name, tier))
            } else {
                Some(format!("{} subscribed! ({})", display_name, tier))
            }
        }
        "resub" => {
            let months = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-cumulative-months")
                .and_then(|(_, v)| v.parse::<u32>().ok())
                .unwrap_or(1);
            if is_ru {
                Some(format!(
                    "{} переподписался на {} мес.!",
                    display_name, months
                ))
            } else {
                Some(format!(
                    "{} resubscribed for {} months!",
                    display_name, months
                ))
            }
        }
        "subgift" => {
            let recipient = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-recipient-display-name")
                .map(|(_, v)| *v)
                .unwrap_or("???");
            if is_ru {
                Some(format!(
                    "{} подарил подписку {}!",
                    display_name, recipient
                ))
            } else {
                Some(format!(
                    "{} gifted a sub to {}!",
                    display_name, recipient
                ))
            }
        }
        "submysterygift" => {
            let count = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-mass-gift-count")
                .and_then(|(_, v)| v.parse::<u32>().ok())
                .unwrap_or(1);
            if is_ru {
                Some(format!("{} подарил {} подписок!", display_name, count))
            } else {
                Some(format!("{} gifted {} subs!", display_name, count))
            }
        }
        "raid" => {
            let viewers = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-viewerCount")
                .and_then(|(_, v)| v.parse::<u32>().ok())
                .unwrap_or(0);
            if is_ru {
                Some(format!(
                    "{} рейдит с {} зрителями!",
                    display_name, viewers
                ))
            } else {
                Some(format!(
                    "{} raided with {} viewers!",
                    display_name, viewers
                ))
            }
        }
        "announcement" => {
            if is_ru {
                Some(format!("Объявление от {}", display_name))
            } else {
                Some(format!("Announcement from {}", display_name))
            }
        }
        "viewermilestone" => {
            let count = tags
                .iter()
                .find(|(k, _)| *k == "msg-param-value")
                .and_then(|(_, v)| v.parse::<u32>().ok())
                .unwrap_or(0);
            if is_ru {
                Some(format!(
                    "{} смотрит {} стримов подряд!",
                    display_name, count
                ))
            } else {
                Some(format!(
                    "{} watched {} streams in a row!",
                    display_name, count
                ))
            }
        }
        _ => None,
    }
}

/// Извлекает бейджи из IRC тегов.
///
/// Формат тега badges: `moderator/1,subscriber/12,premium/1`
fn parse_badges(tags: &[(&str, &str)]) -> Vec<Badge> {
    let badges_str = tags
        .iter()
        .find(|(k, _)| *k == "badges")
        .map(|(_, v)| *v)
        .unwrap_or("");

    if badges_str.is_empty() {
        return Vec::new();
    }

    badges_str
        .split(',')
        .filter_map(|badge| {
            let mut parts = badge.splitn(2, '/');
            let id = parts.next()?;
            let version = parts.next().unwrap_or("1");

            // URL бейджей — пустой, будет заполнен из badge map (после OAuth)
            // На фронтенде используется emoji-фоллбэк если URL пустой
            let title = match id {
                "broadcaster" => "Стример",
                "moderator" => "Модератор",
                "vip" => "VIP",
                "subscriber" => "Подписчик",
                "premium" => "Twitch Prime",
                "bits" => "Bits",
                _ => id,
            };

            Some(Badge {
                id: id.to_string(),
                version: version.to_string(),
                image_url: String::new(),
                title: title.to_string(),
            })
        })
        .collect()
}

/// Извлекает эмоуты из IRC тегов.
///
/// Формат тега emotes: `emote_id:start-end,start-end/emote_id:start-end`
/// Пример: `25:0-4,12-16/1902:6-10` означает Kappa на позициях 0-4 и 12-16
fn parse_emotes(tags: &[(&str, &str)], message: &str) -> Vec<EmoteRef> {
    let emotes_str = tags
        .iter()
        .find(|(k, _)| *k == "emotes")
        .map(|(_, v)| *v)
        .unwrap_or("");

    if emotes_str.is_empty() {
        return Vec::new();
    }

    // Twitch передаёт позиции эмоутов как индексы символов (не байтов).
    // Для корректной работы с кириллицей и другими многобайтовыми символами
    // конвертируем в вектор символов.
    let chars: Vec<char> = message.chars().collect();

    let mut result = Vec::new();

    for emote_group in emotes_str.split('/') {
        let mut parts = emote_group.splitn(2, ':');
        let emote_id = match parts.next() {
            Some(id) if !id.is_empty() => id,
            _ => continue,
        };
        let positions = match parts.next() {
            Some(p) => p,
            None => continue,
        };

        for pos in positions.split(',') {
            let mut range = pos.splitn(2, '-');
            let start: usize = match range.next().and_then(|s| s.parse().ok()) {
                Some(s) => s,
                None => continue,
            };
            let end: usize = match range.next().and_then(|s| s.parse().ok()) {
                Some(e) => e,
                None => continue,
            };

            // Извлекаем код эмоута из текста сообщения (через char-индексы)
            let code = if end < chars.len() {
                chars[start..=end].iter().collect::<String>()
            } else {
                format!("emote_{}", emote_id)
            };

            // URL эмоута Twitch (CDN)
            let url = format!(
                "https://static-cdn.jtvnw.net/emoticons/v2/{}/default/dark/2.0",
                emote_id
            );

            result.push(EmoteRef {
                id: emote_id.to_string(),
                code,
                url,
                start,
                end,
            });
        }
    }

    // Сортируем по позиции начала (для правильной замены в тексте)
    result.sort_by_key(|e| e.start);
    result
}

/// Обрабатывает IRC CLEARCHAT — бан/таймаут пользователя или очистка всего чата.
///
/// Форматы:
/// - `@... :tmi.twitch.tv CLEARCHAT #channel :username` — очистка сообщений пользователя
/// - `@... :tmi.twitch.tv CLEARCHAT #channel` — очистка всего чата
///
/// Тег `target-user-id` присутствует при удалении конкретного пользователя.
fn handle_clearchat(raw: &str, app_handle: &tauri::AppHandle) {
    // Парсим теги
    let tags_str = if raw.starts_with('@') {
        raw.find(' ').map(|i| &raw[1..i]).unwrap_or("")
    } else {
        ""
    };
    let tags = parse_tags(tags_str);

    // Извлекаем target username (после последнего ':')
    let target_user = raw
        .rfind(':')
        .and_then(|i| {
            let user = raw[i + 1..].trim();
            // Убеждаемся что это не часть prefix (:tmi.twitch.tv)
            if user.contains('.') || user.is_empty() {
                None
            } else {
                Some(user.to_string())
            }
        });

    // ban-duration тег: если 0 или отсутствует — перманентный бан
    let _ban_duration = tags
        .iter()
        .find(|(k, _)| *k == "ban-duration")
        .and_then(|(_, v)| v.parse::<u64>().ok());

    if let Some(username) = target_user {
        // Удаление сообщений конкретного пользователя
        info!("CLEARCHAT: удаление сообщений пользователя {}", username);
        let _ = app_handle.emit("chat-user-cleared", &username);
        // Отправляем в OBS overlay
        if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
            let _ = overlay.command_tx.send(format!("clear_user:{}", username));
        }
    } else {
        // Очистка всего чата
        info!("CLEARCHAT: полная очистка чата");
        let _ = app_handle.emit("chat-cleared", "");
        // Отправляем в OBS overlay
        if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
            let _ = overlay.command_tx.send("clear".to_string());
        }
    }
}

/// Обрабатывает IRC CLEARMSG — удаление конкретного сообщения.
///
/// Формат: `@login=user;target-msg-id=abc-123 :tmi.twitch.tv CLEARMSG #channel :message text`
///
/// Тег `target-msg-id` содержит ID удаляемого сообщения.
fn handle_clearmsg(raw: &str, app_handle: &tauri::AppHandle) {
    let tags_str = if raw.starts_with('@') {
        raw.find(' ').map(|i| &raw[1..i]).unwrap_or("")
    } else {
        ""
    };
    let tags = parse_tags(tags_str);

    if let Some((_, msg_id)) = tags.iter().find(|(k, _)| *k == "target-msg-id") {
        if !msg_id.is_empty() {
            info!("CLEARMSG: удаление сообщения {}", msg_id);
            let _ = app_handle.emit("chat-msg-deleted", msg_id.to_string());
            // Отправляем в OBS overlay
            if let Some(overlay) = app_handle.try_state::<crate::overlay::OverlayState>() {
                let _ = overlay.command_tx.send(format!("delete:{}", msg_id));
            }
        }
    }
}

//! Единый формат сообщения чата для всех платформ.
//!
//! `ChatMessage` — это структура, которую frontend получает через Tauri events.
//! Все платформы (Twitch, Kick и т.д.) преобразуют свои сообщения в этот формат.

use serde::{Deserialize, Serialize};

/// Платформа, откуда пришло сообщение.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Kick,
}

/// Бейдж (иконка) пользователя (broadcaster, moderator, subscriber и т.д.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Badge {
    /// Идентификатор бейджа (set_id, например "moderator", "subscriber")
    pub id: String,
    /// Версия бейджа (например "1", "12" для уровня подписки)
    pub version: String,
    /// URL иконки бейджа (заполняется из badge map при наличии OAuth)
    pub image_url: String,
    /// Название для tooltip
    pub title: String,
}

/// Ссылка на эмоут в тексте сообщения.
/// Указывает позиции начала и конца в тексте, где находится эмоут.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmoteRef {
    /// Уникальный ID эмоута
    pub id: String,
    /// Код эмоута (текст, который заменяется картинкой)
    pub code: String,
    /// URL картинки эмоута
    pub url: String,
    /// Начальная позиция в тексте (включительно)
    pub start: usize,
    /// Конечная позиция в тексте (включительно)
    pub end: usize,
}

/// Единое сообщение чата, общее для всех платформ.
/// Отправляется из Rust во frontend через Tauri event `chat-message`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Уникальный ID сообщения (UUID v4)
    pub id: String,
    /// Платформа источника (Twitch, Kick)
    pub platform: Platform,
    /// Логин пользователя (lowercase)
    pub username: String,
    /// Отображаемое имя пользователя (может содержать заглавные буквы)
    pub display_name: String,
    /// Цвет ника в формате hex (#FF0000), None если не задан
    pub color: Option<String>,
    /// Бейджи пользователя (модератор, подписчик и т.д.)
    pub badges: Vec<Badge>,
    /// Текст сообщения
    pub message: String,
    /// Эмоуты в тексте (с позициями для замены)
    pub emotes: Vec<EmoteRef>,
    /// Временная метка сообщения (Unix timestamp в миллисекундах)
    pub timestamp: i64,
    /// Имя канала, в котором написано сообщение
    pub channel: String,
    /// Ответ на сообщение (reply) — имя автора исходного сообщения
    pub reply_to: Option<String>,
    /// Текст исходного сообщения, на которое отвечают
    pub reply_text: Option<String>,
    /// Тип системного события (None для обычных сообщений).
    /// Значения: "sub", "resub", "subgift", "submysterygift", "raid",
    ///           "highlighted", "announcement", "viewermilestone"
    pub event_type: Option<String>,
    /// Системный текст события (например "Username подписался на канал!").
    /// Отображается отдельно от пользовательского сообщения.
    pub system_message: Option<String>,
}

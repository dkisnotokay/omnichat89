//! Настройки TTS озвучки.
//!
//! `TtsSettings` синхронизируются из frontend при каждом изменении.
//! Хранятся в localStorage на frontend и передаются в Rust через команду `tts_update_settings`.

use serde::{Deserialize, Serialize};

/// Настройки TTS озвучки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSettings {
    // --- Основные ---
    /// Включена ли озвучка
    pub enabled: bool,
    /// Имя голоса Edge TTS (например "ru-RU-DmitryNeural") или "random"
    pub voice: String,
    /// Скорость речи в процентах (-50..+100)
    pub rate: i32,
    /// Громкость в процентах (-50..0)
    pub volume: i32,
    /// Максимальный размер очереди сообщений
    pub max_queue_size: usize,
    /// Пауза между сообщениями (мс)
    pub pause_ms: u64,

    // --- Фильтры: что озвучивать ---
    /// Озвучивать все сообщения (когда true — роли ниже игнорируются)
    pub read_all: bool,
    /// Озвучивать ответы (reply)
    pub read_replies: bool,
    /// Озвучивать выделенные сообщения
    pub read_highlighted: bool,
    /// Озвучивать сообщения подписчиков
    pub read_subscribers: bool,
    /// Озвучивать сообщения VIP
    pub read_vip: bool,
    /// Озвучивать сообщения модераторов
    pub read_moderators: bool,

    // --- Фильтры контента ---
    /// Озвучивать имена ("Username сказал:")
    pub read_usernames: bool,
    /// Озвучивать ссылки
    pub read_links: bool,
    /// Озвучивать эмоуты (текстовые коды)
    pub read_emotes: bool,
    /// Максимальная длина текста (символов, 0 = без лимита)
    pub max_message_length: usize,

    // --- Ключевые слова (триггеры) ---
    /// Использовать ключевые слова для активации
    pub use_keywords: bool,
    /// Список ключевых слов: ["!say", "!s"]
    pub keywords: Vec<String>,
    /// Удалять ключевые слова из текста перед озвучкой
    pub strip_keywords: bool,

    // --- Фильтры ---
    /// Символы/слова для удаления из текста перед озвучкой: ["@"]
    pub ignore_symbols: Vec<String>,
    /// Слова-блокировщики: сообщение с ними не озвучивается
    pub word_filter: Vec<String>,
    /// Чёрный список юзернеймов (не озвучивать)
    pub blacklist: Vec<String>,
    /// Белый список юзернеймов (всегда озвучивать, bypass фильтров)
    pub whitelist: Vec<String>,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            voice: "ru-RU-DmitryNeural".to_string(),
            rate: 0,
            volume: 0,
            max_queue_size: 20,
            pause_ms: 300,

            read_all: true,
            read_replies: true,
            read_highlighted: false,
            read_subscribers: false,
            read_vip: false,
            read_moderators: false,

            read_usernames: true,
            read_links: false,
            read_emotes: false,
            max_message_length: 200,

            use_keywords: false,
            keywords: vec![],
            strip_keywords: true,

            ignore_symbols: vec!["@".to_string()],
            word_filter: vec![],
            blacklist: vec![
                "nightbot".to_string(),
                "moobot".to_string(),
                "streamelements".to_string(),
            ],
            whitelist: vec![],
        }
    }
}

/// Доступные голоса Edge TTS для выбора.
pub const AVAILABLE_VOICES: &[(&str, &str)] = &[
    ("ru-RU-DmitryNeural", "Дмитрий"),
    ("ru-RU-SvetlanaNeural", "Светлана"),
    ("en-US-ChristopherNeural", "Christopher (EN)"),
    ("en-US-JennyNeural", "Jenny (EN)"),
    ("en-US-GuyNeural", "Guy (EN)"),
    ("en-US-AriaNeural", "Aria (EN)"),
];

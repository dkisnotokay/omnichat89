//! Персистенция настроек приложения.
//!
//! `AppSettings` — единый конфиг, зеркало фронтенд `AppSettings`.
//! Сохраняется в `%APPDATA%/com.omnichat.app/config.json`.
//! Rust — источник правды, фронтенд синхронизируется через invoke/events.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

/// Все настройки приложения.
/// Поля совпадают с фронтенд `AppSettings` из settings.ts.
/// `rename_all = "camelCase"` — JSON ключи как в TypeScript (fontSize, showTimestamp, ...).
/// `default` — при десериализации старого config.json новые поля получают дефолты.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    // --- Display ---
    pub font_size: u32,
    pub show_timestamp: bool,
    pub show_badges: bool,
    pub show_platform_icon: bool,
    pub show_system_events: bool,
    pub max_messages: u32,
    pub always_on_top: bool,
    pub bg_opacity: u32,
    pub app_bg_opacity: u32,
    pub text_color: String,
    pub bg_color: String,

    // --- Language ---
    pub language: String,

    // --- OBS Overlay ---
    pub overlay_port: u16,
    /// Секретный токен для аутентификации OBS overlay запросов.
    /// Генерируется автоматически при первом запуске.
    pub overlay_secret: String,

    // --- Последние подключённые каналы (для авто-реконнекта) ---
    pub last_twitch_channel: String,
    pub last_kick_channel: String,

    // --- TTS Core ---
    pub tts_enabled: bool,
    pub tts_voice: String,
    pub tts_rate: i32,
    pub tts_volume: u32,
    pub tts_max_queue_size: u32,
    pub tts_pause_ms: u64,

    // --- TTS Role Filters ---
    pub tts_read_all: bool,
    pub tts_read_replies: bool,
    pub tts_read_highlighted: bool,
    pub tts_read_subscribers: bool,
    pub tts_read_vip: bool,
    pub tts_read_moderators: bool,

    // --- TTS Content ---
    pub tts_read_usernames: bool,
    pub tts_read_links: bool,
    pub tts_read_emotes: bool,
    pub tts_max_message_length: u32,

    // --- TTS Keywords ---
    pub tts_use_keywords: bool,
    pub tts_keywords: String,
    pub tts_strip_keywords: bool,

    // --- TTS Text Filters ---
    pub tts_ignore_symbols: String,
    pub tts_word_filter: String,
    pub tts_blacklist: String,
    pub tts_whitelist: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            font_size: 14,
            show_timestamp: false,
            show_badges: true,
            show_platform_icon: true,
            show_system_events: true,
            max_messages: 500,
            always_on_top: false,
            bg_opacity: 100,
            app_bg_opacity: 100,
            text_color: "#e0e0e0".to_string(),
            bg_color: "#1a1a2e".to_string(),

            language: "ru".to_string(),

            overlay_port: 8089,
            overlay_secret: String::new(), // Будет сгенерирован при загрузке если пуст

            last_twitch_channel: String::new(),
            last_kick_channel: String::new(),

            tts_enabled: false,
            tts_voice: "ru-RU-DmitryNeural".to_string(),
            tts_rate: 0,
            tts_volume: 100,
            tts_max_queue_size: 20,
            tts_pause_ms: 300,

            tts_read_all: true,
            tts_read_replies: true,
            tts_read_highlighted: false,
            tts_read_subscribers: false,
            tts_read_vip: false,
            tts_read_moderators: false,

            tts_read_usernames: true,
            tts_read_links: false,
            tts_read_emotes: false,
            tts_max_message_length: 200,

            tts_use_keywords: false,
            tts_keywords: String::new(),
            tts_strip_keywords: true,

            tts_ignore_symbols: "@".to_string(),
            tts_word_filter: String::new(),
            tts_blacklist: "Nightbot, Moobot, StreamElements".to_string(),
            tts_whitelist: String::new(),
        }
    }
}

impl AppSettings {
    /// Конвертировать в TtsSettings для TTS движка.
    /// Дублирует логику фронтенд `toTtsSettings()` из tts.ts.
    pub fn to_tts_settings(&self) -> crate::tts::settings::TtsSettings {
        let parse_list = |s: &str| -> Vec<String> {
            s.split(&[',', ' '][..])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };

        // "random" → "random-ru" / "random-en" в зависимости от языка
        let voice = if self.tts_voice == "random" {
            format!("random-{}", self.language)
        } else {
            self.tts_voice.clone()
        };

        crate::tts::settings::TtsSettings {
            enabled: self.tts_enabled,
            voice,
            rate: self.tts_rate,
            volume: self.tts_volume as i32 - 100, // UI: 0-100 → Edge TTS: -100..0
            max_queue_size: self.tts_max_queue_size as usize,
            pause_ms: self.tts_pause_ms,
            read_all: self.tts_read_all,
            read_replies: self.tts_read_replies,
            read_highlighted: self.tts_read_highlighted,
            read_subscribers: self.tts_read_subscribers,
            read_vip: self.tts_read_vip,
            read_moderators: self.tts_read_moderators,
            read_usernames: self.tts_read_usernames,
            read_links: self.tts_read_links,
            read_emotes: self.tts_read_emotes,
            max_message_length: self.tts_max_message_length as usize,
            use_keywords: self.tts_use_keywords,
            keywords: parse_list(&self.tts_keywords),
            strip_keywords: self.tts_strip_keywords,
            ignore_symbols: parse_list(&self.tts_ignore_symbols),
            word_filter: parse_list(&self.tts_word_filter),
            blacklist: parse_list(&self.tts_blacklist),
            whitelist: parse_list(&self.tts_whitelist),
        }
    }
}

/// Управляемое состояние конфигурации.
pub struct ConfigState {
    pub settings: Arc<Mutex<AppSettings>>,
    pub config_path: PathBuf,
}

const CONFIG_FILENAME: &str = "config.json";

/// Определить путь к config.json через Tauri path resolver.
/// Fallback на текущую директорию если path resolver недоступен.
pub fn config_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|e| {
            log::warn!("Не удалось определить директорию конфигурации: {}, используем fallback", e);
            std::env::current_dir().unwrap_or_default().join("omnichat89")
        })
        .join(CONFIG_FILENAME)
}

/// Генерировать случайный overlay secret (32 hex символов, 128 бит энтропии).
fn generate_overlay_secret() -> String {
    format!("{:032x}", rand::random::<u128>())
}

/// Загрузить настройки из файла. Если файла нет или он повреждён — возвращает дефолты.
/// Автоматически генерирует `overlay_secret` если пустой.
pub fn load_from_file(path: &PathBuf) -> AppSettings {
    let mut settings = match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("Ошибка парсинга config.json, используем дефолты: {}", e);
            AppSettings::default()
        }),
        Err(_) => {
            log::info!("config.json не найден, первый запуск — используем дефолты");
            AppSettings::default()
        }
    };

    // Генерируем overlay_secret если пустой (первый запуск или миграция)
    if settings.overlay_secret.is_empty() {
        settings.overlay_secret = generate_overlay_secret();
        // Сохраняем чтобы секрет был стабильным
        let _ = save_to_file(path, &settings);
    }

    settings
}

/// Сохранить настройки в файл (атомарная запись: .tmp → rename).
pub fn save_to_file(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    // Создать директорию если не существует
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Не удалось создать директорию конфига: {}", e))?;
    }

    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Ошибка сериализации настроек: {}", e))?;

    // Атомарная запись: пишем во временный файл, затем переименовываем
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("Ошибка записи временного файла: {}", e))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("Ошибка переименования файла конфига: {}", e))?;

    Ok(())
}

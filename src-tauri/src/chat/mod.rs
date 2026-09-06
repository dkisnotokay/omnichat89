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
pub mod viewers;

use std::sync::OnceLock;

/// Выбирает русскую форму слова по числу.
///
/// `one` — для 1, 21, 31... (1 стрим)
/// `few` — для 2-4, 22-24... (2 стрима)
/// `many` — для 0, 5-20, 25-30... (5 стримов)
pub fn plural_ru<'a>(n: u32, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n100 = n % 100;
    let n10 = n % 10;
    if (11..=14).contains(&n100) {
        many
    } else if n10 == 1 {
        one
    } else if (2..=4).contains(&n10) {
        few
    } else {
        many
    }
}

#[cfg(test)]
mod plural_tests {
    use super::plural_ru;

    #[test]
    fn picks_correct_russian_form() {
        let f = |n| plural_ru(n, "стрим", "стрима", "стримов");
        assert_eq!(f(1), "стрим");
        assert_eq!(f(2), "стрима");
        assert_eq!(f(4), "стрима");
        assert_eq!(f(5), "стримов");
        assert_eq!(f(11), "стримов"); // 11-14 — исключение
        assert_eq!(f(14), "стримов");
        assert_eq!(f(21), "стрим");
        assert_eq!(f(22), "стрима");
        assert_eq!(f(25), "стримов");
        assert_eq!(f(101), "стрим");
        assert_eq!(f(0), "стримов");
    }
}

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

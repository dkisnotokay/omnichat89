//! Загрузка бейджей Twitch через Helix API.
//!
//! Получает глобальные и канальные бейджи, формирует map:
//! `"set_id/version" → image_url`
//!
//! Требует OAuth токен и Client ID.

use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::auth::get_client_id;

/// Информация о бейдже для отправки во frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeMapEntry {
    pub image_url: String,
    pub title: String,
}

/// Тип badge map: ключ "set_id/version" → BadgeMapEntry.
pub type BadgeMap = HashMap<String, BadgeMapEntry>;

/// Ответ Twitch Helix API для бейджей.
#[derive(Deserialize)]
struct BadgesResponse {
    data: Vec<BadgeSet>,
}

#[derive(Deserialize)]
struct BadgeSet {
    set_id: String,
    versions: Vec<BadgeVersion>,
}

#[derive(Deserialize)]
struct BadgeVersion {
    id: String,
    image_url_2x: String,
    title: String,
}

/// Получить ID пользователя (broadcaster) по логину.
pub async fn get_user_id(token: &str, login: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct UserData {
        id: String,
    }
    #[derive(Deserialize)]
    struct UsersResponse {
        data: Vec<UserData>,
    }

    let client = super::twitch_http_client();
    let resp = client
        .get("https://api.twitch.tv/helix/users")
        .query(&[("login", login)])
        .header("Authorization", format!("Bearer {}", token))
        .header("Client-Id", get_client_id())
        .send()
        .await
        .map_err(|e| format!("Ошибка получения user ID: {}", e))?;

    let users: UsersResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ошибка парсинга: {}", e))?;

    users
        .data
        .first()
        .map(|u| u.id.clone())
        .ok_or_else(|| format!("Пользователь {} не найден", login))
}

/// Загружает глобальные + канальные бейджи и возвращает объединённый map.
pub async fn fetch_badges(token: &str, broadcaster_id: &str) -> Result<BadgeMap, String> {
    let client = super::twitch_http_client();
    let client_id = get_client_id();
    let mut badge_map = BadgeMap::new();

    // 1. Глобальные бейджи
    match fetch_badge_set(&client, token, &client_id, "https://api.twitch.tv/helix/chat/badges/global").await {
        Ok(badges) => {
            info!("Загружено глобальных бейджей: {}", badges.len());
            badge_map.extend(badges);
        }
        Err(e) => error!("Ошибка загрузки глобальных бейджей: {}", e),
    }

    // 2. Канальные бейджи (перезаписывают глобальные при совпадении)
    match fetch_channel_badges(&client, token, &client_id, broadcaster_id).await {
        Ok(badges) => {
            info!("Загружено канальных бейджей: {}", badges.len());
            badge_map.extend(badges);
        }
        Err(e) => error!("Ошибка загрузки канальных бейджей: {}", e),
    }

    info!("Всего бейджей в map: {}", badge_map.len());
    Ok(badge_map)
}

/// Загружает канальные бейджи с параметром broadcaster_id через .query() builder.
async fn fetch_channel_badges(
    client: &reqwest::Client,
    token: &str,
    client_id: &str,
    broadcaster_id: &str,
) -> Result<BadgeMap, String> {
    let resp = client
        .get("https://api.twitch.tv/helix/chat/badges")
        .query(&[("broadcaster_id", broadcaster_id)])
        .header("Authorization", format!("Bearer {}", token))
        .header("Client-Id", client_id)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API вернул статус {}", resp.status()));
    }

    let data: BadgesResponse = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;

    let mut map = BadgeMap::new();
    for set in data.data {
        for version in set.versions {
            let key = format!("{}/{}", set.set_id, version.id);
            map.insert(
                key,
                BadgeMapEntry {
                    image_url: version.image_url_2x,
                    title: version.title,
                },
            );
        }
    }

    Ok(map)
}

/// Загружает бейджи с одного API endpoint (для глобальных бейджей).
async fn fetch_badge_set(
    client: &reqwest::Client,
    token: &str,
    client_id: &str,
    url: &str,
) -> Result<BadgeMap, String> {
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Client-Id", client_id)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API вернул статус {}", resp.status()));
    }

    let data: BadgesResponse = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;

    let mut map = BadgeMap::new();
    for set in data.data {
        for version in set.versions {
            let key = format!("{}/{}", set.set_id, version.id);
            map.insert(
                key,
                BadgeMapEntry {
                    image_url: version.image_url_2x,
                    title: version.title,
                },
            );
        }
    }

    Ok(map)
}

//! Twitch OAuth авторизация через Implicit Grant.
//!
//! Поток:
//! 1. Запускаем локальный HTTP-сервер на localhost:9284
//! 2. Открываем браузер для авторизации Twitch
//! 3. Twitch редиректит на localhost с токеном в URL fragment
//! 4. Callback HTML извлекает токен и отправляет POST обратно
//! 5. Валидируем токен через Twitch API
//! 6. Emit'им событие `twitch-auth-success` во frontend

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// Client ID приложения Omnichat (зарегистрировано на dev.twitch.tv).
/// Это публичный идентификатор — безопасно хранить в коде.
/// Пользователь может переопределить через переменную окружения OMNICHAT_CLIENT_ID.
pub const TWITCH_CLIENT_ID: &str = "8zf4k66b2d9ruarkxgzc28itka20xo";

const OAUTH_PORT: u16 = 9284;
const REDIRECT_URI: &str = "http://localhost:9284/callback";

/// Информация о пользователе после авторизации.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchUserInfo {
    pub login: String,
    pub display_name: String,
    pub user_id: String,
}

/// Payload для события twitch-auth-success (только user info, без токена).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSuccessPayload {
    pub login: String,
    pub display_name: String,
    pub user_id: String,
}

/// Состояние авторизации Twitch (хранится в Tauri State).
pub struct TwitchAuth {
    pub access_token: Arc<Mutex<Option<String>>>,
    pub user_info: Arc<Mutex<Option<TwitchUserInfo>>>,
    /// CSRF state для OAuth flow.
    pub oauth_state: Arc<Mutex<Option<String>>>,
    /// Путь к файлу токена (инициализируется в setup).
    pub token_path: Arc<Mutex<Option<std::path::PathBuf>>>,
}

impl Default for TwitchAuth {
    fn default() -> Self {
        Self {
            access_token: Arc::new(Mutex::new(None)),
            user_info: Arc::new(Mutex::new(None)),
            oauth_state: Arc::new(Mutex::new(None)),
            token_path: Arc::new(Mutex::new(None)),
        }
    }
}

const KEYRING_SERVICE: &str = "omnichat89";
const KEYRING_USER: &str = "twitch_token";

/// Сохранить токен в системное хранилище (Windows Credential Manager).
/// Также удаляет старый plaintext файл auth.dat если он существует (миграция).
pub fn save_token_to_file(path: &std::path::Path, token: &str) -> Result<(), String> {
    // Сохраняем в keyring (основное хранилище)
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => {
            entry.set_password(token)
                .map_err(|e| format!("Ошибка сохранения токена в keyring: {}", e))?;
        }
        Err(e) => {
            warn!("Keyring недоступен ({}), fallback на файл", e);
            // Fallback: сохраняем в файл если keyring недоступен
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Ошибка создания директории: {}", e))?;
            }
            std::fs::write(path, token)
                .map_err(|e| format!("Ошибка сохранения токена: {}", e))?;
            return Ok(());
        }
    }

    // Удаляем старый plaintext файл если существует (миграция)
    if path.exists() {
        let _ = std::fs::remove_file(path);
        info!("Старый auth.dat удалён после миграции в keyring");
    }

    Ok(())
}

/// Загрузить токен: сначала из keyring, затем fallback на файл (миграция).
/// При загрузке из файла автоматически мигрирует в keyring.
pub fn load_token_from_file(path: &std::path::Path) -> Option<String> {
    // 1. Пробуем keyring
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(token) = entry.get_password() {
            if !token.is_empty() {
                return Some(token);
            }
        }
    }

    // 2. Fallback: читаем из файла (старая версия или keyring недоступен)
    let token = std::fs::read_to_string(path).ok().filter(|s| !s.is_empty())?;

    // 3. Мигрируем в keyring и удаляем файл
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if entry.set_password(&token).is_ok() {
            let _ = std::fs::remove_file(path);
            info!("Токен мигрирован из auth.dat в keyring");
        }
    }

    Some(token)
}

/// Удалить токен из keyring и файла.
pub fn delete_token_file(path: &std::path::Path) {
    // Удаляем из keyring
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.delete_credential();
    }
    // Удаляем файл (на случай если остался)
    let _ = std::fs::remove_file(path);
}

/// HTML страница для callback — извлекает токен и state из URL fragment.
const CALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Omnichat89 — Авторизация</title></head>
<body style="background:#1a1a2e;color:#e0e0e0;font-family:sans-serif;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;">
<div style="text-align:center;">
<h2>Авторизация...</h2>
<p id="status">Обработка токена...</p>
</div>
<script>
const hash = window.location.hash.substring(1);
const params = new URLSearchParams(hash);
const token = params.get('access_token');
const state = params.get('state') || '';
if (token) {
  fetch('/token?access_token=' + encodeURIComponent(token) + '&state=' + encodeURIComponent(state), { method: 'POST' })
    .then(() => {
      document.getElementById('status').textContent = 'Готово! Можете закрыть эту вкладку.';
    })
    .catch(() => {
      document.getElementById('status').textContent = 'Ошибка при отправке токена.';
    });
} else {
  document.getElementById('status').textContent = 'Токен не найден. Попробуйте снова.';
}
</script>
</body>
</html>"#;

/// Получить Client ID (из env или встроенный).
pub fn get_client_id() -> String {
    std::env::var("OMNICHAT_CLIENT_ID").unwrap_or_else(|_| TWITCH_CLIENT_ID.to_string())
}

/// Запускает OAuth flow: открывает браузер и ждёт callback с токеном.
/// Возвращает access_token при успехе.
/// `expected_state` — CSRF state для проверки callback.
pub async fn start_oauth_flow(expected_state: &str) -> Result<String, String> {
    let client_id = get_client_id();

    if client_id == "YOUR_CLIENT_ID_HERE" {
        return Err(
            "Client ID не настроен. Установите OMNICHAT_CLIENT_ID или замените TWITCH_CLIENT_ID в auth.rs"
                .to_string(),
        );
    }

    // Запускаем TCP listener
    let listener = TcpListener::bind(format!("127.0.0.1:{}", OAUTH_PORT))
        .await
        .map_err(|e| format!("Не удалось запустить OAuth сервер: {}", e))?;

    info!("OAuth сервер запущен на порту {}", OAUTH_PORT);

    // Формируем URL авторизации с state параметром (CSRF защита)
    let auth_url = format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=token&scope=chat:read&state={}",
        client_id,
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(expected_state),
    );

    // Открываем браузер
    info!("Открываем браузер для авторизации Twitch");
    if let Err(e) = tauri_plugin_opener::open_url(&auth_url, None::<&str>) {
        error!("Не удалось открыть браузер: {}", e);
        return Err(format!("Не удалось открыть браузер: {}", e));
    }

    // Ждём callback (макс. 120 секунд)
    let token = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_token(&listener, expected_state),
    )
    .await
    .map_err(|_| "Таймаут авторизации (120 секунд)".to_string())?
    .map_err(|e| format!("Ошибка OAuth callback: {}", e))?;

    info!("OAuth токен получен");
    Ok(token)
}

/// Ожидает запросы на localhost и извлекает токен из POST /token.
/// Проверяет `state` параметр для защиты от CSRF.
async fn wait_for_token(listener: &TcpListener, expected_state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("Accept error: {}", e))?;

        // Увеличенный буфер для длинных заголовков браузера
        let mut buf = vec![0u8; 16384];
        let mut total = 0;

        // Читаем до получения полного HTTP-запроса (ищем \r\n\r\n)
        loop {
            let n = stream
                .read(&mut buf[total..])
                .await
                .map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                break;
            }
            total += n;
            if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") || total >= buf.len() {
                break;
            }
        }

        let request = String::from_utf8_lossy(&buf[..total]);

        // Определяем тип запроса
        if request.starts_with("GET /callback") {
            // Callback от Twitch — отдаём HTML который прочитает fragment
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CALLBACK_HTML.len(),
                CALLBACK_HTML
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        } else if request.starts_with("POST /token") {
            // Callback JS отправляет токен и state как query parameters
            let token = extract_query_param(&request, "access_token");
            let state = extract_query_param(&request, "state").unwrap_or_default();

            // Проверяем CSRF state
            let state_valid = state == expected_state;

            let response_body = if token.is_some() && state_valid { "OK" } else { "ERROR" };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;

            if !state_valid {
                warn!("OAuth state mismatch (CSRF проверка не пройдена)");
                continue;
            }

            if let Some(token) = token {
                return Ok(token);
            }
        } else {
            // Любой другой запрос — 404
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    }
}

/// Извлекает параметр из query string HTTP запроса.
fn extract_query_param(request: &str, param: &str) -> Option<String> {
    // Формат: "POST /token?access_token=xxx HTTP/1.1\r\n..."
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;

    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == param {
            return kv.next().map(|v| urldecoded(v));
        }
    }
    None
}

/// Валидирует токен через Twitch API и возвращает информацию о пользователе.
pub async fn validate_token(token: &str) -> Result<TwitchUserInfo, String> {
    #[derive(Deserialize)]
    struct ValidateResponse {
        login: String,
        user_id: String,
    }

    let client = super::twitch_http_client();
    let resp = client
        .get("https://id.twitch.tv/oauth2/validate")
        .header("Authorization", format!("OAuth {}", token))
        .send()
        .await
        .map_err(|e| format!("Ошибка валидации токена: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Токен невалиден (статус {})", resp.status()));
    }

    let data: ValidateResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ошибка парсинга ответа: {}", e))?;

    // Получаем display_name через Users API
    let display_name = get_display_name(&client, token, &data.login)
        .await
        .unwrap_or_else(|_| data.login.clone());

    Ok(TwitchUserInfo {
        login: data.login,
        display_name,
        user_id: data.user_id,
    })
}

/// Получает display_name пользователя через Twitch Helix API.
async fn get_display_name(
    client: &reqwest::Client,
    token: &str,
    login: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct UserData {
        display_name: String,
    }
    #[derive(Deserialize)]
    struct UsersResponse {
        data: Vec<UserData>,
    }

    let resp = client
        .get("https://api.twitch.tv/helix/users")
        .query(&[("login", login)])
        .header("Authorization", format!("Bearer {}", token))
        .header("Client-Id", get_client_id())
        .send()
        .await
        .map_err(|e| format!("Ошибка получения пользователя: {}", e))?;

    let users: UsersResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ошибка парсинга: {}", e))?;

    users
        .data
        .first()
        .map(|u| u.display_name.clone())
        .ok_or_else(|| "Пользователь не найден".to_string())
}

/// Отзывает (revoke) OAuth токен.
pub async fn revoke_token(token: &str) -> Result<(), String> {
    let client_id = get_client_id();
    let client = super::twitch_http_client();

    client
        .post("https://id.twitch.tv/oauth2/revoke")
        .form(&[("client_id", &client_id), ("token", &token.to_string())])
        .send()
        .await
        .map_err(|e| format!("Ошибка отзыва токена: {}", e))?;

    Ok(())
}

/// URL decode через urlencoding crate.
fn urldecoded(s: &str) -> String {
    urlencoding::decode(s).unwrap_or_else(|_| std::borrow::Cow::Borrowed(s)).into_owned()
}

//! OBS Browser Source overlay — HTTP-сервер на localhost.
//!
//! Поднимает axum HTTP-сервер, который отдаёт:
//! - `GET /overlay` — самодостаточная HTML-страница с чатом
//! - `GET /overlay/events` — SSE поток сообщений чата
//! - `GET /overlay/settings` — текущие настройки отображения (JSON)
//! - `GET /overlay/settings/stream` — SSE поток обновлений настроек
//!
//! Стример вставляет `http://localhost:<port>/overlay` в OBS → Browser Source.

use crate::chat::badges::BadgeMap;
use crate::chat::message::ChatMessage;
use crate::config::AppSettings;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use log::{error, info};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// HTML-страница оверлея (встроена при компиляции).
const OVERLAY_HTML: &str = include_str!("page.html");

/// Query параметры для аутентификации overlay запросов.
#[derive(Deserialize)]
struct OverlayQuery {
    token: Option<String>,
}

/// Shared state для overlay HTTP-сервера.
#[derive(Clone)]
pub struct OverlayState {
    /// Broadcast канал для сообщений чата (Twitch/Kick → SSE).
    pub chat_tx: broadcast::Sender<ChatMessage>,
    /// Broadcast канал для обновлений настроек.
    pub settings_tx: broadcast::Sender<AppSettings>,
    /// Broadcast канал для управляющих команд (clear и т.д.).
    pub command_tx: broadcast::Sender<String>,
    /// Ссылка на текущие настройки (для GET /overlay/settings).
    pub config_state: Arc<Mutex<AppSettings>>,
    /// Badge map для resolving Twitch badge URLs.
    pub badge_map: Arc<Mutex<BadgeMap>>,
    /// Oneshot sender для graceful shutdown текущего сервера.
    pub shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Секретный токен для аутентификации запросов.
    pub overlay_secret: String,
}

/// Проверить аутентификацию overlay запроса.
fn check_auth(query: &OverlayQuery, secret: &str) -> bool {
    query.token.as_deref() == Some(secret)
}

/// Создать axum Router с маршрутами оверлея.
fn build_router(state: OverlayState) -> Router {
    Router::new()
        .route("/overlay", get(serve_overlay_page))
        .route("/overlay/events", get(serve_chat_sse))
        .route("/overlay/settings", get(serve_settings))
        .route("/overlay/settings/stream", get(serve_settings_sse))
        .route("/overlay/control", get(serve_control_sse))
        .with_state(state)
}

/// Запустить HTTP-сервер оверлея на указанном порту.
/// Использует `tauri::async_runtime::spawn` — безопасно вызывать из setup().
pub fn start_overlay_server(overlay_state: OverlayState, port: u16) {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    *overlay_state.shutdown_tx.blocking_lock() = Some(tx);

    let app = build_router(overlay_state);

    tauri::async_runtime::spawn(async move {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        info!("OBS Overlay сервер запущен на http://{}/overlay", addr);

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Не удалось запустить overlay сервер на порту {}: {}", port, e);
                return;
            }
        };

        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });
}

/// Перезапустить overlay сервер на новом порту.
/// Вызывается из save_settings при изменении overlay_port.
pub async fn restart_overlay_server(overlay_state: OverlayState, new_port: u16) {
    // 1. Остановить старый сервер
    if let Some(tx) = overlay_state.shutdown_tx.lock().await.take() {
        let _ = tx.send(());
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    // 2. Запустить новый сервер
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    *overlay_state.shutdown_tx.lock().await = Some(tx);

    let app = build_router(overlay_state);

    tauri::async_runtime::spawn(async move {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], new_port));
        info!("OBS Overlay сервер перезапущен на http://{}/overlay", addr);

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Не удалось запустить overlay на порту {}: {}", new_port, e);
                return;
            }
        };

        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });
}

/// GET /overlay — HTML-страница оверлея (требует ?token=secret).
async fn serve_overlay_page(
    State(state): State<OverlayState>,
    Query(query): Query<OverlayQuery>,
) -> impl IntoResponse {
    if !check_auth(&query, &state.overlay_secret) {
        return (StatusCode::FORBIDDEN, Html("403 Forbidden")).into_response();
    }
    Html(OVERLAY_HTML).into_response()
}

/// GET /overlay/events — SSE поток сообщений чата (требует ?token=secret).
async fn serve_chat_sse(
    State(state): State<OverlayState>,
    Query(query): Query<OverlayQuery>,
) -> impl IntoResponse {
    if !check_auth(&query, &state.overlay_secret) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }
    let rx = state.chat_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(msg) => {
            let json = serde_json::to_string(&msg).ok()?;
            Some(Ok::<_, Infallible>(Event::default().data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ).into_response()
}

/// GET /overlay/settings — текущие настройки (JSON, требует ?token=secret).
async fn serve_settings(
    State(state): State<OverlayState>,
    Query(query): Query<OverlayQuery>,
) -> impl IntoResponse {
    if !check_auth(&query, &state.overlay_secret) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }
    let settings = state.config_state.lock().await;
    let json = serde_json::to_string(&*settings).unwrap_or_default();
    ([(header::CONTENT_TYPE, "application/json")], json).into_response()
}

/// GET /overlay/settings/stream — SSE поток обновлений настроек (требует ?token=secret).
async fn serve_settings_sse(
    State(state): State<OverlayState>,
    Query(query): Query<OverlayQuery>,
) -> impl IntoResponse {
    if !check_auth(&query, &state.overlay_secret) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }
    let rx = state.settings_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(settings) => {
            let json = serde_json::to_string(&settings).ok()?;
            Some(Ok::<_, Infallible>(Event::default().data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ).into_response()
}

/// GET /overlay/control — SSE поток управляющих команд (требует ?token=secret).
async fn serve_control_sse(
    State(state): State<OverlayState>,
    Query(query): Query<OverlayQuery>,
) -> impl IntoResponse {
    if !check_auth(&query, &state.overlay_secret) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }
    let rx = state.command_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(cmd) => Some(Ok::<_, Infallible>(Event::default().data(cmd))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ).into_response()
}

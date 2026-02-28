//! Основной модуль бэкенда Omnichat89.
//!
//! Здесь настраивается Tauri-приложение: регистрируются команды (invoke),
//! плагины, state и обработчики событий.
//!
//! Модули:
//! - `chat` — подключение к чатам (Twitch, Kick), авторизация, бейджи
//! - `tts` — Edge TTS синтез речи, очередь, воспроизведение
//! - `config` — персистенция настроек (config.json)

mod chat;
mod config;
pub mod overlay;
mod tts;

use chat::auth::{AuthSuccessPayload, TwitchAuth};
use chat::kick::KickState;
use chat::twitch::TwitchState;
use config::ConfigState;
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

/// Managed state для доступа к пунктам меню трея (синхронизация галочек и языка).
pub struct TrayMenuState {
    pub show_hide: MenuItem<tauri::Wry>,
    pub tts_check: CheckMenuItem<tauri::Wry>,
    pub aot_check: CheckMenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
}

// ═══════════════════════════════════════════════════════════
// Команды чата
// ═══════════════════════════════════════════════════════════

/// Подключиться к Twitch каналу.
/// Вызывается из frontend: `invoke("connect_twitch", { channel: "channelname" })`
///
/// Запускает фоновую задачу, которая слушает IRC и отправляет
/// сообщения во frontend через event `chat-message`.
/// Если авторизованы — подключается под аккаунтом и загружает бейджи.
#[tauri::command]
async fn connect_twitch(
    channel: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, TwitchState>,
    auth_state: tauri::State<'_, TwitchAuth>,
) -> Result<(), String> {
    // Если уже подключены — останавливаем предыдущую задачу
    state.should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(handle) = state.task_handle.lock().await.take() {
        handle.abort();
    }

    // Обновляем текущий канал
    *state.current_channel.lock().await = Some(channel.clone());

    // Получаем токен и user info (если авторизованы)
    let token = auth_state.access_token.lock().await.clone();
    let user_login = auth_state
        .user_info
        .lock()
        .await
        .as_ref()
        .map(|u| u.login.clone());

    // Если авторизованы — загружаем бейджи для канала
    if let Some(ref tok) = token {
        let channel_clone = channel.clone();
        let app_clone = app_handle.clone();
        let tok_clone = tok.clone();
        tokio::spawn(async move {
            match chat::badges::get_user_id(&tok_clone, &channel_clone).await {
                Ok(broadcaster_id) => {
                    match chat::badges::fetch_badges(&tok_clone, &broadcaster_id).await {
                        Ok(badge_map) => {
                            let _ = app_clone.emit("badge-map-loaded", &badge_map);
                            log::info!("Badge map отправлен во frontend");
                            // Сохраняем в overlay state для resolving badge URL в OBS
                            if let Some(ov) = app_clone.try_state::<overlay::OverlayState>() {
                                *ov.badge_map.lock().await = badge_map;
                            }
                        }
                        Err(e) => log::error!("Ошибка загрузки бейджей: {}", e),
                    }
                }
                Err(e) => log::error!("Ошибка получения broadcaster ID: {}", e),
            }
        });
    }

    // Клонируем Arc для фоновой задачи
    let should_stop = Arc::clone(&state.should_stop);

    // Запускаем чтение чата в фоновой задаче
    let handle = tokio::spawn(async move {
        chat::twitch::connect_and_listen(channel, app_handle, should_stop, token, user_login)
            .await;
    });
    *state.task_handle.lock().await = Some(handle);

    Ok(())
}

/// Отключиться от Twitch канала.
/// Вызывается из frontend: `invoke("disconnect_twitch")`
#[tauri::command]
async fn disconnect_twitch(state: tauri::State<'_, TwitchState>) -> Result<(), String> {
    state.should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    *state.current_channel.lock().await = None;
    Ok(())
}

/// Запустить OAuth авторизацию Twitch.
/// Открывает браузер для авторизации. Результат приходит через event `twitch-auth-success`.
#[tauri::command]
async fn start_twitch_oauth(
    app_handle: tauri::AppHandle,
    auth_state: tauri::State<'_, TwitchAuth>,
) -> Result<(), String> {
    // Генерируем CSRF state
    let state = format!("{:032x}", rand::random::<u128>());
    *auth_state.oauth_state.lock().await = Some(state.clone());

    let app_clone = app_handle.clone();

    tokio::spawn(async move {
        match chat::auth::start_oauth_flow(&state).await {
            Ok(token) => {
                // Валидируем токен
                match chat::auth::validate_token(&token).await {
                    Ok(user_info) => {
                        // Сохраняем в state
                        let auth: tauri::State<TwitchAuth> = app_clone.state();
                        *auth.access_token.lock().await = Some(token.clone());
                        *auth.user_info.lock().await = Some(user_info.clone());

                        // Сохраняем токен в файл
                        {
                            let path = auth.token_path.lock().await;
                            if let Some(ref p) = *path {
                                let _ = chat::auth::save_token_to_file(p, &token);
                            }
                        }

                        // Уведомляем frontend (без токена — только user info)
                        let payload = AuthSuccessPayload {
                            login: user_info.login.clone(),
                            display_name: user_info.display_name.clone(),
                            user_id: user_info.user_id.clone(),
                        };
                        let _ = app_clone.emit("twitch-auth-success", &payload);
                        log::info!("Twitch OAuth успешно: {}", user_info.login);
                    }
                    Err(e) => {
                        log::error!("Ошибка валидации токена: {}", e);
                        let _ = app_clone.emit("twitch-auth-error", &e);
                    }
                }
            }
            Err(e) => {
                log::error!("Ошибка OAuth: {}", e);
                let _ = app_clone.emit("twitch-auth-error", &e);
            }
        }
    });

    Ok(())
}

/// Проверить сохранённую авторизацию Twitch (загружает токен из файла, валидирует).
/// Вызывается при старте frontend вместо передачи токена из localStorage.
#[tauri::command]
async fn check_twitch_auth(
    app_handle: tauri::AppHandle,
    auth_state: tauri::State<'_, TwitchAuth>,
) -> Result<Option<chat::auth::TwitchUserInfo>, String> {
    // Загружаем токен из файла
    let token = {
        let path = auth_state.token_path.lock().await;
        match &*path {
            Some(p) => chat::auth::load_token_from_file(p),
            None => None,
        }
    };

    let token = match token {
        Some(t) => t,
        None => return Ok(None),
    };

    // Предварительная проверка формата (только alnum, 20-50 символов)
    if token.len() < 20 || token.len() > 50 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        // Повреждённый файл — удаляем
        let path = auth_state.token_path.lock().await;
        if let Some(ref p) = *path {
            chat::auth::delete_token_file(p);
        }
        return Ok(None);
    }

    // Валидируем через Twitch API
    match chat::auth::validate_token(&token).await {
        Ok(user_info) => {
            *auth_state.access_token.lock().await = Some(token);
            *auth_state.user_info.lock().await = Some(user_info.clone());

            let payload = AuthSuccessPayload {
                login: user_info.login.clone(),
                display_name: user_info.display_name.clone(),
                user_id: user_info.user_id.clone(),
            };
            let _ = app_handle.emit("twitch-auth-success", &payload);
            Ok(Some(user_info))
        }
        Err(_) => {
            // Токен невалиден — удаляем файл
            let path = auth_state.token_path.lock().await;
            if let Some(ref p) = *path {
                chat::auth::delete_token_file(p);
            }
            Ok(None)
        }
    }
}

/// Выйти из Twitch аккаунта.
#[tauri::command]
async fn logout_twitch(
    app_handle: tauri::AppHandle,
    auth_state: tauri::State<'_, TwitchAuth>,
) -> Result<(), String> {
    // Отзываем токен
    if let Some(token) = auth_state.access_token.lock().await.take() {
        let _ = chat::auth::revoke_token(&token).await;
    }
    *auth_state.user_info.lock().await = None;

    // Удаляем файл токена
    {
        let path = auth_state.token_path.lock().await;
        if let Some(ref p) = *path {
            chat::auth::delete_token_file(p);
        }
    }

    // Очищаем badge map везде (возврат к emoji-фолбэку)
    if let Some(ov) = app_handle.try_state::<overlay::OverlayState>() {
        ov.badge_map.lock().await.clear();
    }
    let empty_map: chat::badges::BadgeMap = Default::default();
    let _ = app_handle.emit("badge-map-loaded", &empty_map);

    Ok(())
}

/// Подключиться к Kick каналу.
/// Вызывается из frontend: `invoke("connect_kick", { channel: "channelname" })`
#[tauri::command]
async fn connect_kick(
    channel: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, KickState>,
) -> Result<(), String> {
    // Если уже подключены — останавливаем предыдущую задачу
    state.should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(handle) = state.task_handle.lock().await.take() {
        handle.abort();
    }

    // Обновляем текущий канал
    *state.current_channel.lock().await = Some(channel.clone());

    // Получаем chatroom_id через Kick API
    let chatroom_id = chat::kick::fetch_chatroom_id(&channel).await?;

    // Клонируем Arc для фоновой задачи
    let should_stop = Arc::clone(&state.should_stop);

    // Запускаем чтение чата в фоновой задаче
    let handle = tokio::spawn(async move {
        chat::kick::connect_and_listen(channel, chatroom_id, app_handle, should_stop).await;
    });
    *state.task_handle.lock().await = Some(handle);

    Ok(())
}

/// Отключиться от Kick канала.
#[tauri::command]
async fn disconnect_kick(state: tauri::State<'_, KickState>) -> Result<(), String> {
    state.should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    *state.current_channel.lock().await = None;
    Ok(())
}

/// Очистить чат в OBS overlay.
/// Вызывается из frontend при очистке сообщений.
#[tauri::command]
async fn clear_overlay_chat(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.try_state::<overlay::OverlayState>() {
        let _ = overlay.command_tx.send("clear".to_string());
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════
// Команды настроек
// ═══════════════════════════════════════════════════════════

/// Загрузить настройки (при старте frontend).
/// Возвращает текущие AppSettings из памяти (загружены из config.json при запуске).
#[tauri::command]
async fn load_settings(
    config_state: tauri::State<'_, ConfigState>,
) -> Result<config::AppSettings, String> {
    let settings = config_state.settings.lock().await;
    Ok(settings.clone())
}

/// Сохранить настройки (при каждом изменении во frontend).
/// Обновляет config.json, TTS state, галочки трея и рассылает всем окнам.
#[tauri::command]
async fn save_settings(
    new_settings: config::AppSettings,
    app_handle: tauri::AppHandle,
    config_state: tauri::State<'_, ConfigState>,
    tts_state: tauri::State<'_, tts::TtsState>,
) -> Result<(), String> {
    // 0. Валидация полей настроек
    if !(1024..=65535).contains(&new_settings.overlay_port) {
        return Err("Порт overlay должен быть в диапазоне 1024-65535".to_string());
    }

    // Clamp числовых значений в допустимые диапазоны (defense-in-depth)
    let mut new_settings = new_settings;
    new_settings.font_size = new_settings.font_size.clamp(8, 72);
    new_settings.max_messages = new_settings.max_messages.clamp(50, 5000);
    new_settings.bg_opacity = new_settings.bg_opacity.clamp(0, 100);
    new_settings.app_bg_opacity = new_settings.app_bg_opacity.clamp(0, 100);
    new_settings.tts_volume = new_settings.tts_volume.clamp(0, 100);
    new_settings.tts_rate = new_settings.tts_rate.clamp(-50, 100);
    new_settings.tts_max_queue_size = new_settings.tts_max_queue_size.clamp(1, 100);
    new_settings.tts_pause_ms = new_settings.tts_pause_ms.clamp(0, 5000);
    new_settings.tts_max_message_length = new_settings.tts_max_message_length.clamp(10, 500);

    // Валидация цветов (hex формат #RRGGBB)
    let valid_hex = |s: &str| -> bool {
        s.len() == 7
            && s.starts_with('#')
            && s[1..].chars().all(|c| c.is_ascii_hexdigit())
    };
    if !valid_hex(&new_settings.text_color) {
        new_settings.text_color = "#e0e0e0".to_string();
    }
    if !valid_hex(&new_settings.bg_color) {
        new_settings.bg_color = "#1a1a2e".to_string();
    }

    // 1. Обновить в памяти (запоминаем старый порт для проверки)
    // Preserve overlay_secret — frontend не может его менять
    let old_overlay_port;
    {
        let mut current = config_state.settings.lock().await;
        old_overlay_port = current.overlay_port;
        new_settings.overlay_secret = current.overlay_secret.clone();
        *current = new_settings.clone();
    }

    // 2. Сохранить на диск
    config::save_to_file(&config_state.config_path, &new_settings)?;

    // 3. Обновить TTS настройки
    let tts_settings = new_settings.to_tts_settings();
    let was_enabled = {
        let s = tts_state.settings.lock().await;
        s.enabled
    };
    let new_enabled = tts_settings.enabled;
    *tts_state.settings.lock().await = tts_settings;

    // 4. Если TTS выключили — очистить очередь
    if was_enabled && !new_enabled {
        tts_state
            .clear_queue
            .store(true, std::sync::atomic::Ordering::Relaxed);
        tts_state
            .skip_current
            .store(true, std::sync::atomic::Ordering::Relaxed);
        tts_state.notify.notify_one();
        log::info!("TTS выключен — очередь очищена");
    }

    // 5. Синхронизировать трей: галочки + язык (если трей создан)
    if let Some(tray_state) = app_handle.try_state::<TrayMenuState>() {
        let _ = tray_state.tts_check.set_checked(new_settings.tts_enabled);
        let _ = tray_state.aot_check.set_checked(new_settings.always_on_top);

        // Обновить тексты меню при смене языка
        let is_en = new_settings.language == "en";
        let _ = tray_state.show_hide.set_text(if is_en { "Show/Hide" } else { "Показать/Скрыть" });
        let _ = tray_state.tts_check.set_text(if is_en { "TTS" } else { "TTS Озвучка" });
        let _ = tray_state.aot_check.set_text(if is_en { "Always on Top" } else { "Поверх окон" });
        let _ = tray_state.quit.set_text(if is_en { "Exit" } else { "Выход" });
    }

    // 6. Обновить OBS overlay (настройки + settings stream)
    if let Some(overlay) = app_handle.try_state::<overlay::OverlayState>() {
        let mut config = overlay.config_state.lock().await;
        *config = new_settings.clone();
        drop(config);
        let _ = overlay.settings_tx.send(new_settings.clone());
    }

    // 7. Рассылаем всем окнам
    let _ = app_handle.emit("settings-changed", &new_settings);

    // 8. Перезапустить overlay сервер при смене порта
    if new_settings.overlay_port != old_overlay_port {
        if let Some(overlay) = app_handle.try_state::<overlay::OverlayState>() {
            overlay::restart_overlay_server(overlay.inner().clone(), new_settings.overlay_port).await;
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════
// Команды TTS
// ═══════════════════════════════════════════════════════════

/// Пропустить текущее TTS сообщение.
#[tauri::command]
async fn tts_skip(state: tauri::State<'_, tts::TtsState>) -> Result<(), String> {
    state
        .skip_current
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state.notify.notify_one();
    Ok(())
}

/// Очистить очередь TTS.
#[tauri::command]
async fn tts_clear_queue(state: tauri::State<'_, tts::TtsState>) -> Result<(), String> {
    state
        .clear_queue
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state.notify.notify_one();
    Ok(())
}

// ═══════════════════════════════════════════════════════════
// System Tray
// ═══════════════════════════════════════════════════════════

/// Создать системный трей с иконкой и контекстным меню.
/// Возвращает TrayMenuState для синхронизации галочек из save_settings.
fn setup_tray(
    app: &tauri::App,
    settings: &config::AppSettings,
) -> Result<TrayMenuState, Box<dyn std::error::Error>> {
    let is_en = settings.language == "en";

    let show_hide = MenuItemBuilder::with_id(
        "show_hide",
        if is_en {
            "Show/Hide"
        } else {
            "Показать/Скрыть"
        },
    )
    .build(app)?;

    let tts_toggle = CheckMenuItemBuilder::with_id(
        "tts_toggle",
        if is_en {
            "TTS"
        } else {
            "TTS Озвучка"
        },
    )
    .checked(settings.tts_enabled)
    .build(app)?;

    let aot_toggle = CheckMenuItemBuilder::with_id(
        "aot_toggle",
        if is_en {
            "Always on Top"
        } else {
            "Поверх окон"
        },
    )
    .checked(settings.always_on_top)
    .build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let quit = MenuItemBuilder::with_id(
        "quit",
        if is_en { "Exit" } else { "Выход" },
    )
    .build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show_hide, &tts_toggle, &aot_toggle, &separator, &quit])
        .build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/icon.ico"))?;

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Omnichat89")
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show_hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "tts_toggle" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let config_state: tauri::State<ConfigState> = app.state();
                        let tts_state: tauri::State<tts::TtsState> = app.state();

                        let mut settings = config_state.settings.lock().await;
                        settings.tts_enabled = !settings.tts_enabled;
                        let new_settings = settings.clone();
                        drop(settings);

                        // Сохранить на диск
                        let _ =
                            config::save_to_file(&config_state.config_path, &new_settings);

                        // Обновить TTS state
                        let tts_settings = new_settings.to_tts_settings();
                        if !tts_settings.enabled {
                            tts_state.clear_queue.store(
                                true,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            tts_state.skip_current.store(
                                true,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            tts_state.notify.notify_one();
                        }
                        *tts_state.settings.lock().await = tts_settings;

                        // Уведомить frontend
                        let _ = app.emit("settings-changed", &new_settings);
                    });
                }
                "aot_toggle" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let config_state: tauri::State<ConfigState> = app.state();

                        let mut settings = config_state.settings.lock().await;
                        settings.always_on_top = !settings.always_on_top;
                        let new_settings = settings.clone();
                        drop(settings);

                        let _ =
                            config::save_to_file(&config_state.config_path, &new_settings);
                        let _ = app.emit("settings-changed", &new_settings);
                    });
                }
                "quit" => {
                    // Очищаем overlay перед выходом
                    if let Some(overlay) = app.try_state::<overlay::OverlayState>() {
                        let _ = overlay.command_tx.send("clear".to_string());
                    }
                    // Небольшая задержка, чтобы overlay успел получить команду
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(TrayMenuState {
        show_hide,
        tts_check: tts_toggle,
        aot_check: aot_toggle,
        quit,
    })
}

// ═══════════════════════════════════════════════════════════
// Запуск приложения
// ═══════════════════════════════════════════════════════════

/// Запуск Tauri-приложения.
/// Вызывается из main.rs.
pub fn run() {
    tauri::Builder::default()
        // Плагины
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // Состояния (кроме ConfigState — инициализируется в setup)
        .manage(TwitchState::default())
        .manage(KickState::default())
        .manage(TwitchAuth::default())
        .manage(tts::TtsState::default())
        // Инициализация при старте
        .setup(|app| {
            // 1. Загрузить настройки из файла
            let path = config::config_path(&app.handle());
            let settings = config::load_from_file(&path);
            log::info!("Настройки загружены из {:?}", path);

            // 1.1. Инициализировать путь к файлу токена
            {
                let auth: tauri::State<TwitchAuth> = app.state();
                let token_path = path.with_file_name("auth.dat");
                *auth.token_path.blocking_lock() = Some(token_path);
            }

            // 2. Инициализировать TTS настройками из конфига
            let tts_settings = settings.to_tts_settings();
            let tts_state: tauri::State<tts::TtsState> = app.state();
            tauri::async_runtime::block_on(async {
                *tts_state.settings.lock().await = tts_settings;
            });

            // 3. Зарегистрировать ConfigState
            app.manage(ConfigState {
                settings: Arc::new(Mutex::new(settings.clone())),
                config_path: path,
            });

            // 4. Создать System Tray + сохранить ссылки на CheckMenuItem
            match setup_tray(app, &settings) {
                Ok(tray_menu_state) => {
                    app.manage(tray_menu_state);
                }
                Err(e) => {
                    log::error!("Ошибка создания System Tray: {}", e);
                }
            }

            // 5. Запустить OBS Overlay HTTP-сервер
            let (chat_tx, _) = tokio::sync::broadcast::channel(256);
            let (settings_tx, _) = tokio::sync::broadcast::channel(16);
            let (command_tx, _) = tokio::sync::broadcast::channel(16);
            let overlay_state = overlay::OverlayState {
                chat_tx,
                settings_tx,
                command_tx,
                config_state: Arc::new(Mutex::new(settings.clone())),
                badge_map: Arc::new(Mutex::new(Default::default())),
                shutdown_tx: Arc::new(Mutex::new(None)),
                overlay_secret: settings.overlay_secret.clone(),
            };
            app.manage(overlay_state.clone());
            overlay::start_overlay_server(overlay_state, settings.overlay_port);

            // 6. Запустить фоновый TTS процессор
            tts::start_tts_processor(app.handle().clone());

            Ok(())
        })
        // Перехватываем закрытие главного окна → сворачиваем в трей
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Settings окно закрывается нормально
            }
        })
        // Регистрация команд
        .invoke_handler(tauri::generate_handler![
            connect_twitch,
            disconnect_twitch,
            connect_kick,
            disconnect_kick,
            clear_overlay_chat,
            start_twitch_oauth,
            check_twitch_auth,
            logout_twitch,
            load_settings,
            save_settings,
            tts_skip,
            tts_clear_queue,
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске Omnichat89");
}

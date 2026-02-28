// Предотвращает появление консольного окна на Windows в релиз-билдах.
// Не убирайте! Без этого при запуске .exe будет мелькать чёрное окно консоли.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Точка входа приложения Omnichat.
/// Делегирует запуск в lib.rs, где настраивается Tauri.
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();
    omnichat_lib::run()
}

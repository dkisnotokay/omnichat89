//! TTS менеджер — очередь сообщений, фильтрация, синтез и воспроизведение.
//!
//! Содержит:
//! - `TtsState` — состояние TTS (Tauri managed state)
//! - Фильтрация сообщений (по ролям, ключевым словам, чёрный/белый список)
//! - Подготовка текста (удаление ссылок, эмоутов, символов)
//! - Фоновый цикл обработки очереди

pub mod edge;
pub mod player;
pub mod settings;

use crate::chat::message::ChatMessage;
use rand::seq::SliceRandom;
use settings::{TtsSettings, AVAILABLE_VOICES};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, Notify};

/// Состояние TTS озвучки — хранится в Tauri managed state.
pub struct TtsState {
    /// Текущие настройки
    pub settings: Arc<Mutex<TtsSettings>>,
    /// Очередь текстов для озвучки
    pub queue: Arc<Mutex<VecDeque<String>>>,
    /// Идёт ли сейчас воспроизведение
    pub is_speaking: Arc<AtomicBool>,
    /// Флаг: пропустить текущее сообщение
    pub skip_current: Arc<AtomicBool>,
    /// Флаг: очистить очередь
    pub clear_queue: Arc<AtomicBool>,
    /// Уведомление о новых сообщениях в очереди
    pub notify: Arc<Notify>,
}

impl Default for TtsState {
    fn default() -> Self {
        Self {
            settings: Arc::new(Mutex::new(TtsSettings::default())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            is_speaking: Arc::new(AtomicBool::new(false)),
            skip_current: Arc::new(AtomicBool::new(false)),
            clear_queue: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }
}

/// Payload события tts-status для frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TtsStatusPayload {
    pub is_speaking: bool,
    pub queue_size: usize,
}

/// Попробовать добавить сообщение в очередь TTS.
///
/// Проверяет все фильтры, подготавливает текст и добавляет в очередь.
/// Порядок проверок:
/// 1. whitelist → bypass всех фильтров
/// 2. blacklist → блокировка
/// 3. keywords → проверка триггеров
/// 4. roles → проверка ролей (если read_all=false)
/// 5. word_filter → блокировка по словам
/// 6. Подготовка текста → очередь
pub fn try_enqueue(app_handle: &AppHandle, msg: &ChatMessage) {
    let state: tauri::State<TtsState> = app_handle.state();
    let settings = state.settings.clone();
    let queue = state.queue.clone();
    let notify = state.notify.clone();
    let is_speaking = state.is_speaking.clone();

    let msg = msg.clone();
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let s = settings.lock().await;

        if !s.enabled {
            log::debug!("TTS выключен, пропускаем сообщение");
            return;
        }

        // Фильтр системных событий:
        // - sub/resub/raid/gift/announcement/viewermilestone → никогда не озвучивать
        // - highlighted → только если read_highlighted включён
        // - action (/me) → обычное сообщение, проходит все фильтры
        if let Some(ref event_type) = msg.event_type {
            match event_type.as_str() {
                "action" => {
                    // /me сообщения проходят через обычные фильтры
                }
                "highlighted" => {
                    if !s.read_highlighted {
                        log::debug!("TTS: highlighted сообщение, read_highlighted=false, пропускаем");
                        return;
                    }
                    // Highlighted проходит через обычные фильтры
                }
                _ => {
                    log::debug!("TTS: системное событие '{}', пропускаем", event_type);
                    return;
                }
            }
        }

        log::info!("TTS: получено сообщение от {}: {}", msg.username, msg.message);

        let username_lower = msg.username.to_lowercase();

        // 1. Белый список — bypass всех фильтров
        let is_whitelisted = s
            .whitelist
            .iter()
            .any(|w| w.to_lowercase() == username_lower);

        if !is_whitelisted {
            // 2. Чёрный список
            if s.blacklist
                .iter()
                .any(|b| b.to_lowercase() == username_lower)
            {
                return;
            }

            // 3. Ключевые слова
            if s.use_keywords {
                let has_keyword = s
                    .keywords
                    .iter()
                    .any(|kw| msg.message.starts_with(kw.as_str()));
                if !has_keyword {
                    return;
                }
            }

            // 4. Фильтр ответов (независимый от read_all)
            if !s.read_replies && msg.reply_to.is_some() {
                return;
            }

            // 5. Фильтр по ролям
            if !s.read_all {
                let has_badge = |id: &str| msg.badges.iter().any(|b| b.id == id);

                let should_read = (s.read_subscribers && has_badge("subscriber"))
                    || (s.read_vip && has_badge("vip"))
                    || (s.read_moderators
                        && (has_badge("moderator") || has_badge("broadcaster")));

                if !should_read {
                    return;
                }
            }

            // 5. Фильтр слов
            let msg_lower = msg.message.to_lowercase();
            if s.word_filter
                .iter()
                .any(|w| !w.is_empty() && msg_lower.contains(&w.to_lowercase()))
            {
                return;
            }
        }

        // 6. Подготовка текста
        let text = prepare_text(&msg, &s);

        if text.trim().is_empty() {
            return;
        }

        // 7. Проверка размера очереди
        let mut q = queue.lock().await;
        if q.len() >= s.max_queue_size {
            return;
        }
        q.push_back(text.clone());
        let queue_size = q.len();
        drop(q);
        drop(s);

        log::info!("TTS: добавлено в очередь [{}]: \"{}\"", queue_size, text);
        notify.notify_one();
        emit_tts_status(&app, is_speaking.load(Ordering::Relaxed), queue_size);
    });
}

/// Подготовить текст сообщения для озвучки.
fn prepare_text(msg: &ChatMessage, settings: &TtsSettings) -> String {
    let mut text = msg.message.clone();

    // Удалить ключевые слова из начала текста
    if settings.use_keywords && settings.strip_keywords {
        for kw in &settings.keywords {
            if text.starts_with(kw.as_str()) {
                text = text[kw.len()..].trim_start().to_string();
                break;
            }
        }
    }

    // Удалить эмоуты (по позициям)
    if !settings.read_emotes && !msg.emotes.is_empty() {
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::new();
        for (ch_idx, ch) in chars.iter().enumerate() {
            let is_emote = msg
                .emotes
                .iter()
                .any(|e| ch_idx >= e.start && ch_idx <= e.end);
            if !is_emote {
                result.push(*ch);
            } else if !result.ends_with(' ') {
                result.push(' ');
            }
        }
        text = result;
    }

    // Удалить ссылки
    if !settings.read_links {
        text = remove_urls(&text);
    }

    // Удалить игнорируемые символы/слова
    for symbol in &settings.ignore_symbols {
        if !symbol.is_empty() {
            text = text.replace(symbol.as_str(), "");
        }
    }

    // Обрезать по длине
    if settings.max_message_length > 0 {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() > settings.max_message_length {
            text = chars[..settings.max_message_length].iter().collect();
        }
    }

    // Убрать лишние пробелы
    text = text.split_whitespace().collect::<Vec<&str>>().join(" ");

    // Добавить имя пользователя
    if settings.read_usernames && !text.is_empty() {
        text = format!("{} сказал: {}", msg.display_name, text);
    }

    text
}

/// Удалить URL-ы из текста.
fn remove_urls(text: &str) -> String {
    text.split_whitespace()
        .filter(|word| {
            !word.starts_with("http://")
                && !word.starts_with("https://")
                && !word.starts_with("www.")
        })
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Запустить фоновый TTS процессор.
///
/// Создаёт поток для аудио плеера (rodio) и async задачу для обработки очереди.
/// Связь между ними через mpsc каналы.
pub fn start_tts_processor(app_handle: AppHandle) {
    log::info!("TTS процессор запускается...");
    let state: tauri::State<TtsState> = app_handle.state();
    let settings = state.settings.clone();
    let queue = state.queue.clone();
    let is_speaking = state.is_speaking.clone();
    let skip_current = state.skip_current.clone();
    let clear_queue = state.clear_queue.clone();
    let notify = state.notify.clone();

    // Каналы для общения с плеером
    let (play_tx, play_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let (done_tx, done_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // Плеер в отдельном OS-потоке (rodio OutputStream не Send)
    let skip_for_player = skip_current.clone();
    let speaking_for_player = is_speaking.clone();
    std::thread::spawn(move || {
        let player = match player::TtsPlayer::new() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Не удалось инициализировать аудио плеер: {}", e);
                return;
            }
        };

        while let Ok(audio_data) = play_rx.recv() {
            // Проверяем skip перед воспроизведением
            if skip_for_player.load(Ordering::Relaxed) {
                skip_for_player.store(false, Ordering::Relaxed);
                let _ = done_tx.send(());
                continue;
            }
            speaking_for_player.store(true, Ordering::Relaxed);
            let _ = player.play_mp3(audio_data);
            speaking_for_player.store(false, Ordering::Relaxed);
            let _ = done_tx.send(());
        }
    });

    // Обёртка done_rx для async
    let done_rx = Arc::new(tokio::sync::Mutex::new(done_rx));

    // Async цикл обработки очереди с prefetch:
    // Пока текущее сообщение воспроизводится, следующее уже синтезируется.
    // prefetched хранит (текст, аудио) чтобы не перепутать сообщения.
    tauri::async_runtime::spawn(async move {
        let mut prefetched: Option<(String, Vec<u8>)> = None;

        /// Читает голос/rate/volume из настроек.
        /// Поддерживает "random-ru" и "random-en" для случайного выбора по языку.
        async fn read_voice_params(
            settings: &Mutex<settings::TtsSettings>,
        ) -> (String, i32, i32, u64) {
            let s = settings.lock().await;
            let voice = if s.voice.starts_with("random") {
                // "random-ru" → фильтр по "ru-", "random-en" → по "en-", "random" → все
                let lang_prefix = s.voice.strip_prefix("random-").unwrap_or("");
                let voices: Vec<&str> = AVAILABLE_VOICES
                    .iter()
                    .filter(|(v, _)| lang_prefix.is_empty() || v.starts_with(lang_prefix))
                    .map(|(v, _)| *v)
                    .collect();
                let mut rng = rand::thread_rng();
                voices
                    .choose(&mut rng)
                    .unwrap_or(&"ru-RU-DmitryNeural")
                    .to_string()
            } else {
                s.voice.clone()
            };
            (voice, s.rate, s.volume, s.pause_ms)
        }

        loop {
            // Ждём уведомления или периодическую проверку
            tokio::select! {
                _ = notify.notified() => {}
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
            }

            // Очистка очереди
            if clear_queue.load(Ordering::Relaxed) {
                clear_queue.store(false, Ordering::Relaxed);
                prefetched = None;
                let mut q = queue.lock().await;
                q.clear();
                emit_tts_status(&app_handle, is_speaking.load(Ordering::Relaxed), 0);
                continue;
            }

            // Если сейчас воспроизводится — ждём
            if is_speaking.load(Ordering::Relaxed) {
                continue;
            }

            // Определяем текст и аудио: из prefetch или из очереди
            let (text, audio_data) = if let Some((ptext, pdata)) = prefetched.take() {
                // Есть prefetch — используем его напрямую
                log::info!("TTS: используем prefetch для \"{}\"", ptext);
                (ptext, pdata)
            } else {
                // Берём из очереди
                let text = {
                    let mut q = queue.lock().await;
                    q.pop_front()
                };
                let text = match text {
                    Some(t) => t,
                    None => continue,
                };

                log::info!("TTS процессор: синтез \"{}\"", text);
                let (voice, rate, volume, _) = read_voice_params(&settings).await;

                match edge::synthesize(&text, &voice, rate, volume).await {
                    Ok(data) => {
                        log::info!("TTS: синтез OK, {} байт", data.len());
                        (text, data)
                    }
                    Err(e) => {
                        log::error!("Ошибка синтеза TTS: {}", e);
                        continue;
                    }
                }
            };

            // Проверяем skip
            if skip_current.load(Ordering::Relaxed) {
                skip_current.store(false, Ordering::Relaxed);
                prefetched = None;
                continue;
            }

            // Воспроизводим
            log::info!("TTS: воспроизводим \"{}\"", text);
            is_speaking.store(true, Ordering::Relaxed);
            {
                let q = queue.lock().await;
                emit_tts_status(&app_handle, true, q.len());
            }
            let _ = play_tx.send(audio_data);

            // Пока воспроизводится — синтезируем следующее сообщение (prefetch)
            let next_text = {
                let mut q = queue.lock().await;
                q.pop_front()
            };

            if let Some(next) = next_text {
                log::info!("TTS prefetch: синтезируем \"{}\"", next);
                let (next_voice, next_rate, next_volume, _) =
                    read_voice_params(&settings).await;

                let prefetch_handle = tauri::async_runtime::spawn(async move {
                    edge::synthesize(&next, &next_voice, next_rate, next_volume)
                        .await
                        .map(|data| (next, data))
                });

                // Ждём окончания воспроизведения текущего
                {
                    let mut rx = done_rx.lock().await;
                    let _ = rx.recv().await;
                }

                // Забираем результат prefetch
                if !clear_queue.load(Ordering::Relaxed)
                    && !skip_current.load(Ordering::Relaxed)
                {
                    match prefetch_handle.await {
                        Ok(Ok((ptext, pdata))) => {
                            log::info!("TTS prefetch OK для \"{}\"", ptext);
                            prefetched = Some((ptext, pdata));
                        }
                        Ok(Err(e)) => {
                            log::error!("Ошибка prefetch TTS: {}", e);
                        }
                        Err(_) => {}
                    }
                } else {
                    prefetch_handle.abort();
                    prefetched = None;
                }
            } else {
                // Очередь пуста — просто ждём окончания воспроизведения
                let mut rx = done_rx.lock().await;
                let _ = rx.recv().await;
            }

            is_speaking.store(false, Ordering::Relaxed);
            skip_current.store(false, Ordering::Relaxed);

            // Пауза между сообщениями
            let (_, _, _, pause_ms) = read_voice_params(&settings).await;
            if pause_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(pause_ms)).await;
            }

            {
                let q = queue.lock().await;
                emit_tts_status(&app_handle, false, q.len());
            }
        }
    });
}

/// Отправить статус TTS во frontend.
fn emit_tts_status(app_handle: &AppHandle, is_speaking: bool, queue_size: usize) {
    let _ = app_handle.emit(
        "tts-status",
        TtsStatusPayload {
            is_speaking,
            queue_size,
        },
    );
}

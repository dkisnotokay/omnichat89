//! Аудио плеер для TTS.
//!
//! Использует rodio для воспроизведения MP3 аудио из байтов.
//! Поддерживает прерывание текущего воспроизведения (skip).

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::Mutex;

/// TTS аудио плеер.
///
/// Хранит rodio OutputStream (должен жить всё время работы TTS)
/// и текущий Sink для управления воспроизведением.
pub struct TtsPlayer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    current_sink: Mutex<Option<Sink>>,
}

impl TtsPlayer {
    /// Создать новый плеер.
    /// OutputStream создаётся один раз и живёт до уничтожения плеера.
    pub fn new() -> Result<Self, String> {
        let (_stream, stream_handle) =
            OutputStream::try_default().map_err(|e| format!("Ошибка инициализации аудио: {}", e))?;
        Ok(Self {
            _stream,
            stream_handle,
            current_sink: Mutex::new(None),
        })
    }

    /// Воспроизвести MP3 из байтов. Блокирует до окончания воспроизведения.
    ///
    /// Возвращает `Ok(true)` если воспроизведение завершилось полностью,
    /// `Ok(false)` если было прервано (skip).
    pub fn play_mp3(&self, data: Vec<u8>) -> Result<bool, String> {
        let cursor = Cursor::new(data);
        let source =
            Decoder::new(cursor).map_err(|e| format!("Ошибка декодирования MP3: {}", e))?;

        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Ошибка создания Sink: {}", e))?;

        sink.append(source);

        // Сохраняем sink для возможности skip
        {
            let mut current = self.current_sink.lock().unwrap_or_else(|e| e.into_inner());
            *current = Some(sink);
        }

        // Ждём окончания воспроизведения
        // Проверяем каждые 50мс — позволяет реагировать на skip
        loop {
            let sink_ref = self.current_sink.lock().unwrap_or_else(|e| e.into_inner());
            match &*sink_ref {
                Some(s) if s.empty() => {
                    break;
                }
                None => {
                    // Sink был убран (skip)
                    return Ok(false);
                }
                _ => {}
            }
            drop(sink_ref);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Очищаем
        {
            let mut current = self.current_sink.lock().unwrap_or_else(|e| e.into_inner());
            *current = None;
        }

        Ok(true)
    }
}

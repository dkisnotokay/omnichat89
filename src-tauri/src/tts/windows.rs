//! Локальный TTS движок — Windows.Media.SpeechSynthesis (WinRT).
//!
//! Работает полностью оффлайн с голосами, установленными в Windows
//! (Параметры → Время и язык → Речь). На русской Windows обычно есть Irina.
//!
//! Синтез возвращает WAV-байты — воспроизводятся тем же rodio-плеером,
//! что и MP3 от Edge TTS (Decoder автоопределяет формат).

use serde::Serialize;
use windows::core::HSTRING;
use windows::Media::SpeechSynthesis::SpeechSynthesizer;
use windows::Storage::Streams::DataReader;

/// Информация о голосе Windows для frontend.
#[derive(Debug, Clone, Serialize)]
pub struct WindowsVoice {
    /// Отображаемое имя ("Microsoft Irina")
    pub name: String,
    /// Язык ("ru-RU")
    pub language: String,
}

/// Перечислить установленные голоса Windows.
pub fn list_voices() -> Result<Vec<WindowsVoice>, String> {
    let voices = SpeechSynthesizer::AllVoices()
        .map_err(|e| format!("Ошибка получения голосов Windows: {}", e))?;

    let mut result = Vec::new();
    for voice in &voices {
        let name = voice
            .DisplayName()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let language = voice
            .Language()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            result.push(WindowsVoice { name, language });
        }
    }
    // Русские голоса первыми, затем по алфавиту
    result.sort_by(|a, b| {
        let a_ru = a.language.starts_with("ru");
        let b_ru = b.language.starts_with("ru");
        b_ru.cmp(&a_ru).then(a.name.cmp(&b.name))
    });
    Ok(result)
}

/// Синтезировать текст локальным голосом Windows.
///
/// # Аргументы
/// * `voice_name` — DisplayName голоса (пусто = голос Windows по умолчанию)
/// * `rate` — скорость в процентах, как у Edge TTS (-50..+100)
/// * `volume` — громкость, как в TtsSettings (-100..0 → 0..100%)
///
/// # Возвращает
/// WAV-байты.
pub async fn synthesize(
    text: &str,
    voice_name: &str,
    rate: i32,
    volume: i32,
) -> Result<Vec<u8>, String> {
    let text = text.to_string();
    let voice_name = voice_name.to_string();

    // WinRT вызовы блокирующие (.get()) — уводим в blocking-поток tokio
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let synth = SpeechSynthesizer::new()
            .map_err(|e| format!("Ошибка создания синтезатора: {}", e))?;

        // Выбор голоса по имени
        if !voice_name.is_empty() {
            if let Ok(voices) = SpeechSynthesizer::AllVoices() {
                for voice in &voices {
                    if voice
                        .DisplayName()
                        .map(|n| n.to_string() == voice_name)
                        .unwrap_or(false)
                    {
                        let _ = synth.SetVoice(&voice);
                        break;
                    }
                }
            }
        }

        // Скорость и громкость
        if let Ok(options) = synth.Options() {
            // rate -50..+100 (%) → SpeakingRate 0.5..2.0
            let speaking_rate = (1.0 + rate as f64 / 100.0).clamp(0.5, 6.0);
            let _ = options.SetSpeakingRate(speaking_rate);
            // volume -100..0 → AudioVolume 0.0..1.0
            let audio_volume = ((volume + 100) as f64 / 100.0).clamp(0.0, 1.0);
            let _ = options.SetAudioVolume(audio_volume);
        }

        // Синтез → WinRT stream → Vec<u8>
        let stream = synth
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))
            .map_err(|e| format!("Ошибка запуска синтеза: {}", e))?
            .get()
            .map_err(|e| format!("Ошибка синтеза: {}", e))?;

        let size = stream
            .Size()
            .map_err(|e| format!("Ошибка размера потока: {}", e))? as u32;
        if size == 0 {
            return Err("Синтезатор вернул пустой звук".to_string());
        }

        let input = stream
            .GetInputStreamAt(0)
            .map_err(|e| format!("Ошибка чтения потока: {}", e))?;
        let reader = DataReader::CreateDataReader(&input)
            .map_err(|e| format!("Ошибка создания reader: {}", e))?;
        reader
            .LoadAsync(size)
            .map_err(|e| format!("Ошибка загрузки данных: {}", e))?
            .get()
            .map_err(|e| format!("Ошибка чтения данных: {}", e))?;

        let mut buf = vec![0u8; size as usize];
        reader
            .ReadBytes(&mut buf)
            .map_err(|e| format!("Ошибка копирования данных: {}", e))?;

        Ok(buf)
    })
    .await
    .map_err(|e| format!("Ошибка blocking-задачи: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ручная проверка локального синтеза:
    /// `cargo test probe_windows_tts -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn probe_windows_tts() {
        let voices = list_voices().expect("список голосов");
        for v in &voices {
            println!("voice: {} ({})", v.name, v.language);
        }
        let audio = synthesize("Привет, это проверка локальной озвучки", "", 0, 0)
            .await
            .expect("синтез");
        println!("audio bytes: {}", audio.len());
        assert!(audio.len() > 1000, "слишком маленький аудио-буфер");

        // Проверяем, что rodio может декодировать WAV (фича "wav" включена)
        let decoder = rodio::Decoder::new(std::io::Cursor::new(audio));
        assert!(decoder.is_ok(), "rodio не смог декодировать WAV: {:?}", decoder.err());
        println!("rodio decode: OK");
    }
}

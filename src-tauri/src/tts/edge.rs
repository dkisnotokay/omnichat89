//! Edge TTS WebSocket клиент.
//!
//! Реализует протокол Microsoft Edge TTS для синтеза речи.
//! Включает DRM-токен Sec-MS-GEC (обязателен с 2024 года).
//!
//! Протокол:
//! 1. Генерация Sec-MS-GEC токена (SHA-256 от timestamp + token)
//! 2. Подключение к WebSocket с правильными заголовками
//! 3. Отправка speech.config + SSML
//! 4. Получение бинарных аудио-чанков (MP3)
//! 5. turn.end — конец синтеза

use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

const EDGE_TTS_URL: &str =
    "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const AUDIO_OUTPUT_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
const CHROMIUM_FULL_VERSION: &str = "143.0.3650.75";
const SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";

/// Windows epoch offset (секунды между 1601-01-01 и 1970-01-01).
const WIN_EPOCH: u64 = 11644473600;

/// Генерация Sec-MS-GEC DRM токена.
///
/// Алгоритм: SHA-256( round_down(ticks, 300) * 1e7 + TRUSTED_CLIENT_TOKEN )
/// где ticks = unix_timestamp + WIN_EPOCH (переводит в Windows filetime).
fn generate_sec_ms_gec() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let ticks = now + WIN_EPOCH;
    let ticks = ticks - (ticks % 300); // round down to 5 min
    let ticks_ns = ticks as u128 * 10_000_000; // convert to 100-nanosecond intervals

    let str_to_hash = format!("{}{}", ticks_ns, TRUSTED_CLIENT_TOKEN);
    let hash = Sha256::digest(str_to_hash.as_bytes());
    hex::encode_upper(hash)
}

/// Генерация случайного MUID (Machine Unique Identifier).
fn generate_muid() -> String {
    let bytes: [u8; 16] = rand::random();
    hex::encode_upper(bytes)
}

/// Синтезировать речь через Edge TTS.
///
/// Возвращает MP3 байты или ошибку.
/// Каждый вызов создаёт новое WebSocket-подключение.
pub async fn synthesize(
    text: &str,
    voice: &str,
    rate: i32,
    volume: i32,
) -> Result<Vec<u8>, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст для синтеза".to_string());
    }

    // Hard cap: обрезаем текст до 500 символов (defense-in-depth)
    let truncated: String;
    let text = if text.chars().count() > 500 {
        truncated = text.chars().take(500).collect();
        truncated.as_str()
    } else {
        text
    };

    let connection_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let sec_ms_gec = generate_sec_ms_gec();

    let url = format!(
        "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
        EDGE_TTS_URL, TRUSTED_CLIENT_TOKEN, connection_id, sec_ms_gec, SEC_MS_GEC_VERSION
    );

    // Собираем WebSocket request с нужными заголовками
    let mut request = url.into_client_request().map_err(|e| format!("Ошибка URL: {}", e))?;
    let headers = request.headers_mut();
    headers.insert("Pragma", "no-cache".parse().unwrap());
    headers.insert("Cache-Control", "no-cache".parse().unwrap());
    headers.insert(
        "Origin",
        "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "User-Agent",
        format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
            (KHTML, like Gecko) Chrome/{} Safari/537.36 Edg/{}",
            CHROMIUM_FULL_VERSION, CHROMIUM_FULL_VERSION
        )
        .parse()
        .unwrap(),
    );
    headers.insert(
        "Cookie",
        format!("muid={};", generate_muid()).parse().unwrap(),
    );

    // Подключаемся к WebSocket
    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("Ошибка подключения к Edge TTS: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    // Отправляем speech.config
    let config_msg = format!(
        "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
        {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\
        \"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\
        \"outputFormat\":\"{}\"}}}}}}}}",
        AUDIO_OUTPUT_FORMAT
    );
    write
        .send(Message::Text(config_msg.into()))
        .await
        .map_err(|e| format!("Ошибка отправки config: {}", e))?;

    // Формируем SSML
    let request_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let escaped_text = escape_xml(text);

    let lang = if voice.len() >= 5 { &voice[..5] } else { "ru-RU" };

    let rate_str = if rate >= 0 {
        format!("+{}%", rate)
    } else {
        format!("{}%", rate)
    };
    let volume_str = if volume >= 0 {
        format!("+{}%", volume)
    } else {
        format!("{}%", volume)
    };

    let ssml = format!(
        "<speak version=\"1.0\" xmlns=\"http://www.w3.org/2001/10/synthesis\" \
        xmlns:mstts=\"https://www.w3.org/2001/mstts\" xml:lang=\"{}\">\
        <voice name=\"{}\">\
        <prosody rate=\"{}\" pitch=\"+0Hz\" volume=\"{}\">\
        {}\
        </prosody></voice></speak>",
        lang, voice, rate_str, volume_str, escaped_text
    );

    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nPath:ssml\r\n\r\n{}",
        request_id, ssml
    );
    write
        .send(Message::Text(ssml_msg.into()))
        .await
        .map_err(|e| format!("Ошибка отправки SSML: {}", e))?;

    // Собираем аудио-чанки
    let mut audio_data: Vec<u8> = Vec::new();
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(30));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                if audio_data.is_empty() {
                    return Err("Таймаут синтеза Edge TTS".to_string());
                }
                break;
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        // Формат: 2 байта (big-endian u16) длина заголовка + заголовок + аудио
                        if data.len() > 2 {
                            let header_len = u16::from_be_bytes([data[0], data[1]]) as usize;
                            let audio_start = 2 + header_len;
                            if audio_start < data.len() {
                                audio_data.extend_from_slice(&data[audio_start..]);
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if text_str.contains("Path:turn.end") {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Err(e)) => {
                        if audio_data.is_empty() {
                            return Err(format!("Ошибка WebSocket: {}", e));
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if audio_data.is_empty() {
        return Err("Edge TTS не вернул аудио данных".to_string());
    }

    Ok(audio_data)
}

/// XML-escape текста для SSML.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

# Omnichat89

Лёгкий оверлей чата для стримов на **Twitch** и **Kick** — оба чата в одном окне с озвучкой, OBS-оверлеем и гибкими настройками.

![Rust](https://img.shields.io/badge/Rust-57.1%25-orange) ![Svelte](https://img.shields.io/badge/Svelte-24.3%25-red) ![TypeScript](https://img.shields.io/badge/TypeScript-12.8%25-blue) ![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6)

> 🇬🇧 [English version](README.md)

## Возможности

- **Twitch + Kick** — два чата в одном приложении
- **OBS-оверлей** — Browser Source для стрима (localhost)
- **Озвучка (TTS)** — Edge TTS, 100+ голосов, фильтры, очередь, управление
- **Twitch OAuth** — бейджи и эмоуты через Helix API
- **Системные события** — подписки, рейды, подарки, выделенные сообщения
- **Модерация** — удаление сообщений, баны, очистка чата (приложение + оверлей)
- **Настройки** — шрифт, цвета, прозрачность, язык (RU/EN)
- **Авто-реконнект** — экспоненциальный backoff при потере соединения
- **Безопасность** — Windows Credential Manager для токенов, CSP, CSRF-защита

## Скачать

Перейдите в [**Releases**](https://github.com/dkisnotokay/omnichat89/releases) и скачайте последний установщик:

| Файл | Описание |
|---|---|
| `Omnichat89_*_x64-setup.exe` | NSIS-установщик (рекомендуется) |
| `Omnichat89_*_x64_en-US.msi` | MSI-установщик |

**Требования:** Windows 10/11 (x64), WebView2 (обычно уже установлен)

## Настройка OBS

1. Запустите приложение → Настройки → скопируйте **URL оверлея**
2. OBS → Источники → **Браузер** → вставьте URL
3. Ширина: **400**, Высота: **600**

Оверлей синхронизирует настройки, сообщения и модерацию в реальном времени.

## Сборка из исходников

```bash
# Требования: Node.js 20+, Rust, Visual Studio Build Tools
git clone https://github.com/dkisnotokay/omnichat89.git
cd omnichat89
npm install
npm run tauri dev      # Dev-режим (hot reload)
npm run tauri build    # Собрать установщик
```

## Стек технологий

| Слой | Технология |
|---|---|
| Десктоп-фреймворк | Tauri v2 |
| Фронтенд | Svelte 5 + TypeScript |
| Бэкенд | Rust (tokio, axum, reqwest) |
| Twitch | IRC WebSocket |
| Kick | Pusher WebSocket |
| Озвучка | Microsoft Edge TTS (WebSocket) |
| OBS-оверлей | HTTP + SSE (axum) |
| Хранение токенов | Windows Credential Manager (keyring) |

## Лицензия

MIT

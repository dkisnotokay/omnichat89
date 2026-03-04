# Omnichat89

Lightweight stream chat overlay for **Twitch** and **Kick** — both chats in one window with TTS, OBS overlay, and more.

> 🇷🇺 [Русская версия](README.ru.md)

![Rust](https://img.shields.io/badge/Rust-57.1%25-orange) ![Svelte](https://img.shields.io/badge/Svelte-24.3%25-red) ![TypeScript](https://img.shields.io/badge/TypeScript-12.8%25-blue) ![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6)

## Features

- **Twitch + Kick** — two chats in one app
- **OBS Overlay** — Browser Source for your stream (localhost)
- **TTS** — Edge TTS with 100+ voices, filters, queue, and controls
- **Twitch OAuth** — badges and emotes via Helix API
- **System events** — subs, raids, gifts, highlighted messages
- **Moderation sync** — message deletion, bans, chat clear (app + overlay)
- **Settings** — font, colors, opacity, language (RU/EN)
- **Auto-reconnect** — exponential backoff on connection loss
- **Secure** — Windows Credential Manager for token storage, CSP, CSRF protection

## Download

Go to [**Releases**](https://github.com/dkisnotokay/omnichat89/releases) and download the latest installer:

| File | Description |
|---|---|
| `Omnichat89_*_x64-setup.exe` | NSIS installer (recommended) |
| `Omnichat89_*_x64_en-US.msi` | MSI installer |

**Requirements:** Windows 10/11 (x64), WebView2 (usually pre-installed)

## OBS Setup

1. Launch the app → Settings → copy the **Overlay URL**
2. OBS → Sources → **Browser Source** → paste the URL
3. Width: **400**, Height: **600**

The overlay syncs settings, messages, and moderation actions in real-time.

## Build from Source

```bash
# Prerequisites: Node.js 20+, Rust, Visual Studio Build Tools
git clone https://github.com/dkisnotokay/omnichat89.git
cd omnichat89
npm install
npm run tauri dev      # Dev mode (hot reload)
npm run tauri build    # Build installer
```

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop framework | Tauri v2 |
| Frontend | Svelte 5 + TypeScript |
| Backend | Rust (tokio, axum, reqwest) |
| Twitch | IRC WebSocket |
| Kick | Pusher WebSocket |
| TTS | Microsoft Edge TTS (WebSocket) |
| OBS Overlay | HTTP + SSE (axum) |
| Token storage | Windows Credential Manager (keyring) |

## License

MIT

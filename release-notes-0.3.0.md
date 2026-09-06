## Auto-update ⚡

The app now updates itself. On startup it quietly checks for a new version and shows a banner — one click downloads and installs it, then restarts. There is also a **Check for updates** button at the bottom of Settings.

> **Note:** this is the last release you need to install by hand. Updates from 0.3.0 onward happen inside the app.

## New Features

- **GIFs in Twitch chat** — Twitch added GIF messages in July 2026, and they now render as actual images in both the app and the OBS overlay instead of a bare `[GIF by ...]` placeholder. Text-to-speech skips them.
- **Kicks gifts on Kick** — support for Kick's new gifting system (their answer to Twitch bits). Gifts appear as a highlighted green event with the sender, the gift and their attached message.
- **Broadcaster channel events on Kick** — the app previously listened only to the chat channel, so Kick's monetization events never arrived at all. Fixed.

## Under the hood

- Added live API probe tests (`cargo test probe_ -- --ignored`) covering Twitch IRC, Twitch viewer counts, Kick chat and both TTS engines — a one-minute check that the platforms haven't broken anything.
- Verified against live channels: everything from 0.2.1 still works.

---

## Автообновление ⚡

Приложение теперь обновляется само. При запуске оно тихо проверяет наличие новой версии и показывает баннер — одно нажатие, и обновление скачается, установится и перезапустит приложение. Ещё есть кнопка **«Проверить обновления»** внизу настроек.

> **Важно:** это последняя версия, которую нужно ставить руками. Начиная с 0.3.0 обновления происходят внутри приложения.

## Новые возможности

- **GIF в чате Twitch** — в июле 2026 Twitch добавил отправку GIF, и теперь они показываются картинкой в приложении и OBS-оверлее, а не текстом `[GIF by ...]`. Озвучка их пропускает.
- **Подарки Kicks на Kick** — поддержка новой системы подарков Kick (их аналог битов Twitch). Подарок показывается зелёным событием с именем отправителя, названием подарка и его сообщением.
- **События канала на Kick** — раньше приложение слушало только чат, поэтому события монетизации не приходили вообще. Исправлено.

## Под капотом

- Добавлены живые probe-тесты API (`cargo test probe_ -- --ignored`) для Twitch IRC, счётчиков зрителей, чата Kick и обоих движков озвучки — минутная проверка, что платформы ничего не сломали.
- Проверено на живых каналах: всё из 0.2.1 продолжает работать.

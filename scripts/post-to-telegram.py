#!/usr/bin/env python3
"""
Публикация поста в Telegram-канал Omnichat89.

Запуск:  python scripts/post-to-telegram.py <файл-с-текстом.md>
Проверка без отправки:  ... --dry-run

Токен и канал берутся из ~/.omnichat-tg.json — файл лежит ВНЕ репозитория
и в git не попадает. Формат: {"token": "...", "channel": "@omnichat89"}

Текст отправляется в разметке Markdown: **жирный**, [ссылка](url).
"""
import json
import os
import sys
import urllib.parse
import urllib.request

CONFIG = os.path.expanduser("~/.omnichat-tg.json")
API = "https://api.telegram.org/bot{token}/{method}"


def load_config():
    if not os.path.exists(CONFIG):
        sys.exit(f"Нет файла конфигурации: {CONFIG}")
    with open(CONFIG, encoding="utf-8-sig") as f:
        cfg = json.load(f)
    if not cfg.get("token") or not cfg.get("channel"):
        sys.exit("В конфигурации должны быть поля token и channel")
    return cfg


def send(token, channel, text, preview=True):
    """Отправляет сообщение. Возвращает (успех, описание)."""
    data = urllib.parse.urlencode({
        "chat_id": channel,
        "text": text,
        "parse_mode": "Markdown",
        # Превью ссылки на GitHub только мешает — пост и так со ссылкой в конце
        "link_preview_options": json.dumps({"is_disabled": not preview}),
    }).encode()
    req = urllib.request.Request(API.format(token=token, method="sendMessage"), data=data)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            res = json.load(r)
    except urllib.error.HTTPError as e:
        res = json.load(e)
    except Exception as e:
        return False, str(e)
    if res.get("ok"):
        mid = res["result"]["message_id"]
        return True, f"опубликовано, message_id={mid}"
    return False, res.get("description", "неизвестная ошибка")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    dry = "--dry-run" in sys.argv
    if not args:
        sys.exit("Укажите файл с текстом поста")

    with open(args[0], encoding="utf-8") as f:
        text = f.read().strip()
    if not text:
        sys.exit("Файл пуст")
    if len(text) > 4096:
        sys.exit(f"Слишком длинно для одного сообщения: {len(text)} символов (лимит 4096)")

    cfg = load_config()
    if dry:
        print(f"[dry-run] в {cfg['channel']} ушло бы {len(text)} символов, отправки не было")
        return

    ok, info = send(cfg["token"], cfg["channel"], text, preview=False)
    print(("OK: " if ok else "ОШИБКА: ") + info)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()

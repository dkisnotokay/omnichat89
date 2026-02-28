/**
 * Svelte store для настроек приложения.
 *
 * - Загружает настройки из Rust backend (config.json) при старте
 * - Сохраняет изменения через invoke("save_settings") → Rust → файл
 * - Синхронизирует между окнами через Tauri event "settings-changed"
 * - Миграция: при первом запуске подхватывает старые данные из localStorage
 * - Применяет CSS-переменные при изменении
 */
import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Настройки приложения */
export interface AppSettings {
  fontSize: number;
  showTimestamp: boolean;
  showBadges: boolean;
  showPlatformIcon: boolean;
  showSystemEvents: boolean;
  maxMessages: number;
  alwaysOnTop: boolean;
  bgOpacity: number;
  appBgOpacity: number;
  textColor: string;
  bgColor: string;

  // --- Язык интерфейса ---
  language: "ru" | "en";

  // --- OBS Overlay ---
  overlayPort: number;
  overlaySecret: string;

  // --- Последние подключённые каналы ---
  lastTwitchChannel: string;
  lastKickChannel: string;

  // --- TTS настройки ---
  ttsEnabled: boolean;
  ttsVoice: string;
  ttsRate: number;
  ttsVolume: number;
  ttsMaxQueueSize: number;
  ttsPauseMs: number;
  ttsReadAll: boolean;
  ttsReadReplies: boolean;
  ttsReadHighlighted: boolean;
  ttsReadSubscribers: boolean;
  ttsReadVip: boolean;
  ttsReadModerators: boolean;
  ttsReadUsernames: boolean;
  ttsReadLinks: boolean;
  ttsReadEmotes: boolean;
  ttsMaxMessageLength: number;
  ttsUseKeywords: boolean;
  ttsKeywords: string;
  ttsStripKeywords: boolean;
  ttsIgnoreSymbols: string;
  ttsWordFilter: string;
  ttsBlacklist: string;
  ttsWhitelist: string;
}

/** Значения по умолчанию */
export const defaultSettings: AppSettings = {
  fontSize: 14,
  showTimestamp: false,
  showBadges: true,
  showPlatformIcon: true,
  showSystemEvents: true,
  maxMessages: 500,
  alwaysOnTop: false,
  bgOpacity: 100,
  appBgOpacity: 100,
  textColor: "#e0e0e0",
  bgColor: "#1a1a2e",

  language: "ru",

  overlayPort: 8089,
  overlaySecret: "",

  lastTwitchChannel: "",
  lastKickChannel: "",

  ttsEnabled: false,
  ttsVoice: "ru-RU-DmitryNeural",
  ttsRate: 0,
  ttsVolume: 100,
  ttsMaxQueueSize: 20,
  ttsPauseMs: 300,
  ttsReadAll: true,
  ttsReadReplies: true,
  ttsReadHighlighted: false,
  ttsReadSubscribers: false,
  ttsReadVip: false,
  ttsReadModerators: false,
  ttsReadUsernames: true,
  ttsReadLinks: false,
  ttsReadEmotes: false,
  ttsMaxMessageLength: 200,
  ttsUseKeywords: false,
  ttsKeywords: "",
  ttsStripKeywords: true,
  ttsIgnoreSymbols: "@",
  ttsWordFilter: "",
  ttsBlacklist: "Nightbot, Moobot, StreamElements",
  ttsWhitelist: "",
};

/** Ключ localStorage (для миграции со старой версии) */
const STORAGE_KEY = "omnichat-settings";

/** Стор настроек (инициализируется с дефолтами, реальные значения из Rust) */
export const settings = writable<AppSettings>({ ...defaultSettings });

/** Флаг: обновление пришло от Rust, не надо отправлять обратно */
let _syncing = false;

/** Флаг: инициализация завершена, можно сохранять */
let _initialized = false;

/** Таймер debounce для сохранения (слайдеры дают много событий) */
let _saveTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Инициализировать настройки: загрузить из Rust, настроить синхронизацию.
 * Вызывать один раз из onMount().
 */
export async function initSettingsSync(): Promise<void> {
  // 1. Загрузить из Rust (config.json)
  try {
    const loaded = await invoke<AppSettings>("load_settings");
    _syncing = true;
    settings.set({ ...defaultSettings, ...loaded });
    _syncing = false;
    _initialized = true;
  } catch {
    // Fallback: миграция из localStorage (первый запуск после обновления)
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed: AppSettings = { ...defaultSettings, ...JSON.parse(saved) };
        _syncing = true;
        settings.set(parsed);
        _syncing = false;
        // Мигрируем в Rust
        await invoke("save_settings", { newSettings: parsed });
      }
    } catch { /* ignore */ }
    _initialized = true;
  }

  // 2. Слушать изменения от Rust (кросс-окно + трей)
  await listen<AppSettings>("settings-changed", (e) => {
    _syncing = true;
    settings.set({ ...defaultSettings, ...e.payload });
    _syncing = false;
  });

  // 3. Отправлять изменения в Rust при каждом обновлении (с debounce)
  settings.subscribe((s) => {
    if (!_syncing && _initialized) {
      if (_saveTimer) clearTimeout(_saveTimer);
      _saveTimer = setTimeout(() => {
        invoke("save_settings", { newSettings: s }).catch((e) => {
          console.error("Failed to save settings:", e);
        });
      }, 300);
    }
  });
}

/** Сбросить настройки по умолчанию */
export function resetSettings(): void {
  settings.set({ ...defaultSettings });
  // subscribe → invoke("save_settings") произойдёт автоматически
}

/**
 * Применяет CSS-переменные на :root на основе текущих настроек.
 * В окне настроек НЕ вызывается (шрифт фиксирован).
 *
 * Примечание: bgColor/bgOpacity НЕ устанавливаются как CSS-переменные —
 * они применяются только к области чата (ChatView), не ко всему приложению.
 */
export function applyCssVariables(s: AppSettings): void {
  const root = document.documentElement;
  root.style.setProperty("--font-size", `${s.fontSize}px`);
  root.style.setProperty("--text-color", s.textColor);
  // Обновить lang атрибут HTML для screen readers и spellcheck
  root.lang = s.language;
}

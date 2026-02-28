/**
 * Svelte store для сообщений чата.
 *
 * Подписывается на Tauri events от Twitch и Kick и накапливает сообщения.
 * Ограничение: максимум MAX_MESSAGES сообщений (старые удаляются).
 *
 * Per-platform статусы: twitchStatus / kickStatus — раздельные stores.
 * Общий connectionStatus — derived (любой connected → "connected").
 *
 * Авто-реконнект: при обрыве соединения автоматически переподключается
 * с экспоненциальным backoff (3с, 6с, 12с, 24с, 30с макс, до 10 попыток).
 */
import { writable, derived, get } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage } from "../types";

/** Тип статуса подключения */
type ConnectionStatus = "disconnected" | "connecting" | "connected";

/** Максимальное количество сообщений в буфере */
const MAX_MESSAGES = 500;

/** Максимальное число попыток переподключения */
const MAX_RECONNECT_RETRIES = 10;

/** Badge map: ключ "set_id/version" → image_url */
export const badgeMap = writable<Record<string, string>>({});

/** Массив сообщений чата (общий для всех платформ) */
export const messages = writable<ChatMessage[]>([]);

// ──────────────────────────────────────────────────────────
// Per-platform stores
// ──────────────────────────────────────────────────────────

/** Статус подключения Twitch */
export const twitchStatus = writable<ConnectionStatus>("disconnected");

/** Текущий канал Twitch */
export const twitchChannel = writable<string>("");

/** Последняя ошибка Twitch (от асинхронных событий) */
export const twitchAsyncError = writable<string>("");

/** Статус подключения Kick */
export const kickStatus = writable<ConnectionStatus>("disconnected");

/** Текущий канал Kick */
export const kickChannel = writable<string>("");

// ──────────────────────────────────────────────────────────
// Обратная совместимость: общие derived stores
// ──────────────────────────────────────────────────────────

/** Общий статус: connected если хотя бы одна платформа подключена */
export const connectionStatus = derived(
  [twitchStatus, kickStatus],
  ([$twitch, $kick]) => {
    if ($twitch === "connected" || $kick === "connected") return "connected" as const;
    if ($twitch === "connecting" || $kick === "connecting") return "connecting" as const;
    return "disconnected" as const;
  }
);

/** Текущий канал (первый подключённый) */
export const currentChannel = derived(
  [twitchStatus, twitchChannel, kickStatus, kickChannel],
  ([$ts, $tc, $ks, $kc]) => {
    if ($ts === "connected") return $tc;
    if ($ks === "connected") return $kc;
    return "";
  }
);

/** Счётчики сообщений (writable — O(1) инкремент вместо filter по всему массиву) */
export const messageCount = writable(0);
export const twitchMessageCount = writable(0);
export const kickMessageCount = writable(0);

/** Очистить все сообщения (приложение + OBS overlay) */
export function clearAllMessages() {
  messages.set([]);
  messageCount.set(0);
  twitchMessageCount.set(0);
  kickMessageCount.set(0);
  invoke("clear_overlay_chat").catch(() => {});
}

// ──────────────────────────────────────────────────────────
// Авто-реконнект (внутреннее состояние)
// ──────────────────────────────────────────────────────────

/** Twitch reconnect state */
let _twitchManualDisconnect = false;
let _twitchReconnecting = false;
let _twitchRetryCount = 0;
let _twitchReconnectTimer: ReturnType<typeof setTimeout> | null = null;

/** Kick reconnect state */
let _kickManualDisconnect = false;
let _kickReconnecting = false;
let _kickRetryCount = 0;
let _kickReconnectTimer: ReturnType<typeof setTimeout> | null = null;

/** Задержка с экспоненциальным backoff: 3с, 6с, 12с, 24с, 30с макс */
function reconnectDelay(attempt: number): number {
  return Math.min(3000 * Math.pow(2, attempt - 1), 30000);
}

/** Запланировать переподключение к Twitch */
function scheduleTwitchReconnect() {
  if (_twitchReconnectTimer) clearTimeout(_twitchReconnectTimer);
  if (_twitchRetryCount >= MAX_RECONNECT_RETRIES) {
    console.warn(`Twitch: reconnect gave up after ${MAX_RECONNECT_RETRIES} attempts`);
    _twitchReconnecting = false;
    return;
  }
  _twitchReconnecting = true;
  _twitchRetryCount++;
  const delay = reconnectDelay(_twitchRetryCount);
  console.log(
    `Twitch: reconnecting in ${delay / 1000}s (attempt ${_twitchRetryCount}/${MAX_RECONNECT_RETRIES})`
  );

  _twitchReconnectTimer = setTimeout(async () => {
    _twitchReconnectTimer = null;
    const channel = get(twitchChannel);
    if (channel && _twitchReconnecting) {
      twitchStatus.set("connecting");
      try {
        await invoke("connect_twitch", { channel });
      } catch {
        scheduleTwitchReconnect();
      }
    }
  }, delay);
}

/** Отменить переподключение к Twitch */
function cancelTwitchReconnect() {
  _twitchReconnecting = false;
  _twitchRetryCount = 0;
  if (_twitchReconnectTimer) {
    clearTimeout(_twitchReconnectTimer);
    _twitchReconnectTimer = null;
  }
}

/** Запланировать переподключение к Kick */
function scheduleKickReconnect() {
  if (_kickReconnectTimer) clearTimeout(_kickReconnectTimer);
  if (_kickRetryCount >= MAX_RECONNECT_RETRIES) {
    console.warn(`Kick: reconnect gave up after ${MAX_RECONNECT_RETRIES} attempts`);
    _kickReconnecting = false;
    return;
  }
  _kickReconnecting = true;
  _kickRetryCount++;
  const delay = reconnectDelay(_kickRetryCount);
  console.log(
    `Kick: reconnecting in ${delay / 1000}s (attempt ${_kickRetryCount}/${MAX_RECONNECT_RETRIES})`
  );

  _kickReconnectTimer = setTimeout(async () => {
    _kickReconnectTimer = null;
    const channel = get(kickChannel);
    if (channel && _kickReconnecting) {
      kickStatus.set("connecting");
      try {
        await invoke("connect_kick", { channel });
      } catch {
        scheduleKickReconnect();
      }
    }
  }, delay);
}

/** Отменить переподключение к Kick */
function cancelKickReconnect() {
  _kickReconnecting = false;
  _kickRetryCount = 0;
  if (_kickReconnectTimer) {
    clearTimeout(_kickReconnectTimer);
    _kickReconnectTimer = null;
  }
}

// ──────────────────────────────────────────────────────────
// Инициализация event listeners
// ──────────────────────────────────────────────────────────

/** Инициализация подписок на Tauri events (Twitch + Kick + модерация) */
export async function initChatListeners() {
  // ── Общее: новые сообщения (от обеих платформ) ──
  await listen<ChatMessage>("chat-message", (event) => {
    const msg = event.payload;
    messages.update((msgs) => {
      const updated = [...msgs, msg];
      if (updated.length > MAX_MESSAGES) {
        // Подсчитываем удаляемые сообщения для корректировки счётчиков
        const trimmed = updated.slice(0, updated.length - MAX_MESSAGES);
        for (const m of trimmed) {
          if (m.platform === "twitch") twitchMessageCount.update((n) => Math.max(0, n - 1));
          else if (m.platform === "kick") kickMessageCount.update((n) => Math.max(0, n - 1));
        }
        messageCount.update((n) => Math.max(0, n - trimmed.length));
        return updated.slice(updated.length - MAX_MESSAGES);
      }
      return updated;
    });
    // Инкремент счётчиков (O(1))
    messageCount.update((n) => n + 1);
    if (msg.platform === "twitch") twitchMessageCount.update((n) => n + 1);
    else if (msg.platform === "kick") kickMessageCount.update((n) => n + 1);
  });

  // ── Twitch events ──
  await listen<string>("chat-connected", (event) => {
    twitchStatus.set("connected");
    twitchChannel.set(event.payload);
    twitchAsyncError.set("");
    // Успешное (пере)подключение — сбрасываем reconnect state
    _twitchReconnecting = false;
    _twitchRetryCount = 0;
  });

  await listen<string>("chat-disconnected", () => {
    twitchStatus.set("disconnected");
    // Авто-реконнект только при неожиданном обрыве
    if (!_twitchManualDisconnect) {
      scheduleTwitchReconnect();
    }
    _twitchManualDisconnect = false;
  });

  await listen<string>("chat-error", (event) => {
    console.error("Twitch chat error:", event.payload);
    twitchStatus.set("disconnected");
    twitchAsyncError.set(event.payload);
    // Реконнект только если уже в процессе переподключения
    // и ошибка не «канал не найден»
    if (_twitchReconnecting && !event.payload.includes("не найден")) {
      scheduleTwitchReconnect();
    }
  });

  // ── Kick events ──
  await listen<string>("kick-chat-connected", (event) => {
    kickStatus.set("connected");
    kickChannel.set(event.payload);
    _kickReconnecting = false;
    _kickRetryCount = 0;
  });

  await listen<string>("kick-chat-disconnected", () => {
    kickStatus.set("disconnected");
    if (!_kickManualDisconnect) {
      scheduleKickReconnect();
    }
    _kickManualDisconnect = false;
  });

  await listen<string>("kick-chat-error", (event) => {
    console.error("Kick chat error:", event.payload);
    kickStatus.set("disconnected");
    if (_kickReconnecting && !event.payload.includes("не найден")) {
      scheduleKickReconnect();
    }
  });

  // ── Модерация: удаление конкретного сообщения (CLEARMSG / Kick delete) ──
  await listen<string>("chat-msg-deleted", (event) => {
    const msgId = event.payload;
    messages.update((msgs) =>
      msgs.map((m) => (m.id === msgId ? { ...m, deleted: true } : m))
    );
  });

  // ── Модерация: очистка сообщений пользователя (CLEARCHAT с username) ──
  await listen<string>("chat-user-cleared", (event) => {
    const username = event.payload.toLowerCase();
    messages.update((msgs) =>
      msgs.map((m) =>
        m.username.toLowerCase() === username ? { ...m, deleted: true } : m
      )
    );
  });

  // ── Модерация: полная очистка чата (CLEARCHAT без username / Kick clear) ──
  await listen<string>("chat-cleared", () => {
    messages.update((msgs) => msgs.map((m) => ({ ...m, deleted: true })));
  });
}

// ──────────────────────────────────────────────────────────
// Twitch — подключение / отключение
// ──────────────────────────────────────────────────────────

/** Подключиться к Twitch каналу. Бросает ошибку если не удалось. */
export async function connectTwitch(channel: string) {
  _twitchManualDisconnect = false;
  cancelTwitchReconnect();
  twitchStatus.set("connecting");
  // Очищаем чат только если Kick не подключён
  const kickConnected = get(kickStatus) === "connected";
  if (!kickConnected) {
    clearAllMessages();
  }
  try {
    await invoke("connect_twitch", { channel });
  } catch (e) {
    console.error("Failed to connect to Twitch:", e);
    twitchStatus.set("disconnected");
    throw e;
  }
}

/** Отключиться от Twitch */
export async function disconnectTwitch() {
  _twitchManualDisconnect = true;
  cancelTwitchReconnect();
  try {
    await invoke("disconnect_twitch");
  } catch (e) {
    console.error("Failed to disconnect from Twitch:", e);
  }
  twitchStatus.set("disconnected");
}

// ──────────────────────────────────────────────────────────
// Kick — подключение / отключение
// ──────────────────────────────────────────────────────────

/** Подключиться к Kick каналу. Бросает ошибку если не удалось. */
export async function connectKick(channel: string) {
  _kickManualDisconnect = false;
  cancelKickReconnect();
  kickStatus.set("connecting");
  // Очищаем чат только если Twitch не подключён
  const twitchConnected = get(twitchStatus) === "connected";
  if (!twitchConnected) {
    clearAllMessages();
  }
  try {
    await invoke("connect_kick", { channel });
  } catch (e) {
    console.error("Failed to connect to Kick:", e);
    kickStatus.set("disconnected");
    throw e;
  }
}

/** Отключиться от Kick */
export async function disconnectKick() {
  _kickManualDisconnect = true;
  cancelKickReconnect();
  try {
    await invoke("disconnect_kick");
  } catch (e) {
    console.error("Failed to disconnect from Kick:", e);
  }
  kickStatus.set("disconnected");
}

// ──────────────────────────────────────────────────────────
// Бейджи
// ──────────────────────────────────────────────────────────

/** Слушать badge-map-loaded event от Rust */
export async function initBadgeListener() {
  await listen<Record<string, { image_url: string; title: string }>>(
    "badge-map-loaded",
    (event) => {
      const map: Record<string, string> = {};
      for (const [key, value] of Object.entries(event.payload)) {
        map[key] = value.image_url;
      }
      badgeMap.set(map);
      console.log(`Badge map loaded: ${Object.keys(map).length} badges`);
    }
  );
}

/**
 * Статус HTTP-сервера OBS-оверлея.
 *
 * Если порт занят (обычно — уже запущен второй экземпляр приложения),
 * сервер не стартует, и чат в OBS молча перестаёт обновляться. Это событие
 * позволяет показать пользователю причину вместо тишины.
 */
import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export interface OverlayServerStatus {
  running: boolean;
  port: number;
  error: string | null;
}

/** null — статус ещё не получен */
export const overlayServerStatus = writable<OverlayServerStatus | null>(null);

/**
 * Подписаться на статус overlay-сервера (вызывать один раз при старте).
 *
 * Сервер стартует раньше, чем загружается интерфейс, поэтому событие о неудаче
 * может уйти до подписки — сохранённый статус дозапрашивается отдельно.
 */
export async function initOverlayStatusListener(): Promise<void> {
  await listen<OverlayServerStatus>("overlay-server-status", (event) => {
    overlayServerStatus.set(event.payload);
  });
  try {
    const saved = await invoke<OverlayServerStatus | null>("get_overlay_status");
    if (saved) overlayServerStatus.set(saved);
  } catch {
    /* статус недоступен — не критично */
  }
}

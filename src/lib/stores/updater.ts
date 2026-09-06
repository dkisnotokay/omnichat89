/**
 * Svelte store для автообновления приложения.
 *
 * Проверяет наличие новой версии через Tauri updater plugin
 * (latest.json в GitHub Releases), скачивает и устанавливает.
 *
 * Проверка при старте — отложенная (не тормозит запуск).
 * Ошибки сети не показываются пользователю: обновление не критично,
 * приложение продолжает работать.
 */
import { writable, get } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** Состояние процесса обновления */
export type UpdateStage =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

/** Статус обновления для UI */
export interface UpdaterState {
  stage: UpdateStage;
  /** Версия доступного обновления */
  version: string;
  /** Прогресс скачивания 0-100 */
  progress: number;
  /** Текст ошибки (только для ручной проверки) */
  error: string;
}

export const updater = writable<UpdaterState>({
  stage: "idle",
  version: "",
  progress: 0,
  error: "",
});

/** Найденное обновление (между check и install) */
let _pending: Update | null = null;

/** Задержка перед автопроверкой при старте (мс) */
const STARTUP_DELAY_MS = 5000;

/**
 * Проверить наличие обновления.
 * @param silent — не показывать ошибки (для автопроверки при старте)
 */
export async function checkForUpdate(silent = false): Promise<boolean> {
  if (get(updater).stage === "downloading") return false;

  updater.update((s) => ({ ...s, stage: "checking", error: "" }));
  try {
    const update = await check();
    if (update) {
      _pending = update;
      updater.set({
        stage: "available",
        version: update.version,
        progress: 0,
        error: "",
      });
      return true;
    }
    updater.update((s) => ({ ...s, stage: "idle", version: "" }));
    return false;
  } catch (e) {
    console.error("Update check failed:", e);
    updater.update((s) => ({
      ...s,
      stage: silent ? "idle" : "error",
      error: silent ? "" : String(e),
    }));
    return false;
  }
}

/**
 * Скачать и установить найденное обновление, затем перезапустить приложение.
 */
export async function installUpdate(): Promise<void> {
  if (!_pending) return;

  updater.update((s) => ({ ...s, stage: "downloading", progress: 0 }));

  let downloaded = 0;
  let total = 0;

  try {
    await _pending.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? 0;
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          if (total > 0) {
            updater.update((s) => ({
              ...s,
              progress: Math.min(100, Math.round((downloaded / total) * 100)),
            }));
          }
          break;
        case "Finished":
          updater.update((s) => ({ ...s, stage: "ready", progress: 100 }));
          break;
      }
    });
    // Установщик отработал — перезапускаем на новой версии
    await relaunch();
  } catch (e) {
    console.error("Update install failed:", e);
    updater.update((s) => ({ ...s, stage: "error", error: String(e) }));
  }
}

/** Скрыть баннер обновления (пользователь отложил) */
export function dismissUpdate(): void {
  updater.update((s) => ({ ...s, stage: "idle" }));
}

/** Автопроверка при старте приложения (с задержкой, тихо) */
export function initUpdaterCheck(): void {
  setTimeout(() => {
    checkForUpdate(true).catch(() => {});
  }, STARTUP_DELAY_MS);
}

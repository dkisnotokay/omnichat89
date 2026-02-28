/**
 * Svelte store для TTS статуса и управления.
 *
 * Получает события от Rust backend (tts-status)
 * и предоставляет функции управления (skip, clear).
 *
 * TTS настройки теперь синхронизируются через save_settings в Rust (config.rs),
 * не нужен отдельный initTtsSettingsSync().
 */
import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { TtsStatus } from "../types";

/** Текущий статус TTS */
export const ttsStatus = writable<TtsStatus>({
  is_speaking: false,
  queue_size: 0,
});

/** Инициализировать TTS event listeners */
export async function initTtsListeners(): Promise<void> {
  await listen<TtsStatus>("tts-status", (event) => {
    ttsStatus.set(event.payload);
  });
}

/** Пропустить текущее TTS сообщение */
export async function ttsSkip(): Promise<void> {
  await invoke("tts_skip");
}

/** Очистить очередь TTS */
export async function ttsClearQueue(): Promise<void> {
  await invoke("tts_clear_queue");
}

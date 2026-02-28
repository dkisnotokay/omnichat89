/**
 * Svelte store для Twitch авторизации.
 *
 * - Сохраняет ТОЛЬКО информацию о пользователе в localStorage (без токена!)
 * - Токен хранится исключительно на стороне Rust (в памяти + auth.dat)
 * - Слушает Tauri events: twitch-auth-success, twitch-auth-error
 * - При старте вызывает check_twitch_auth для валидации сохранённого токена
 */
import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Информация о пользователе (от Rust после валидации токена) */
export interface TwitchUserInfo {
  login: string;
  display_name: string;
  user_id: string;
}

/** Состояние авторизации */
export interface AuthState {
  /** Информация о пользователе */
  userInfo: TwitchUserInfo | null;
  /** Идёт процесс авторизации */
  loading: boolean;
  /** Текст ошибки */
  error: string | null;
}

const AUTH_STORAGE_KEY = "omnichat-twitch-auth";

/** Загрузить из localStorage (только userInfo, без токена) */
function loadAuthFromStorage(): AuthState {
  try {
    const saved = localStorage.getItem(AUTH_STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      return {
        userInfo: parsed.userInfo || null,
        loading: false,
        error: null,
      };
    }
  } catch { /* ignore */ }
  return { userInfo: null, loading: false, error: null };
}

/** Стор авторизации */
export const auth = writable<AuthState>(loadAuthFromStorage());

/** Автосохранение в localStorage (только userInfo) */
auth.subscribe((state) => {
  try {
    localStorage.setItem(
      AUTH_STORAGE_KEY,
      JSON.stringify({
        userInfo: state.userInfo,
      })
    );
  } catch { /* ignore */ }
});

/**
 * Инициализировать авторизацию:
 * - Слушать Tauri events (auth-success, auth-error)
 * - Проверить сохранённый токен через Rust (check_twitch_auth)
 */
export async function initAuth(): Promise<void> {
  // Слушаем успех авторизации (payload: user info без токена)
  await listen<TwitchUserInfo>("twitch-auth-success", (event) => {
    auth.update((s) => ({
      ...s,
      userInfo: event.payload,
      loading: false,
      error: null,
    }));
  });

  // Слушаем ошибки авторизации
  await listen<string>("twitch-auth-error", (event) => {
    auth.update((s) => ({
      ...s,
      loading: false,
      error: event.payload,
    }));
  });

  // Проверяем сохранённый токен через Rust (токен хранится в auth.dat, не в localStorage)
  try {
    const userInfo = await invoke<TwitchUserInfo | null>("check_twitch_auth");
    if (!userInfo) {
      // Токен невалиден или отсутствует — очищаем
      auth.set({ userInfo: null, loading: false, error: null });
    }
    // Если валиден — event twitch-auth-success уже обновил стор
  } catch {
    auth.set({ userInfo: null, loading: false, error: null });
  }
}

/** Запустить OAuth flow (открывает браузер) */
export async function loginTwitch(): Promise<void> {
  auth.update((s) => ({ ...s, loading: true, error: null }));
  try {
    await invoke("start_twitch_oauth");
    // Результат придёт через event twitch-auth-success
  } catch (e) {
    auth.update((s) => ({
      ...s,
      loading: false,
      error: String(e),
    }));
  }
}

/** Выйти из Twitch аккаунта */
export async function logoutTwitch(): Promise<void> {
  try {
    await invoke("logout_twitch");
  } catch { /* ignore */ }
  auth.set({ userInfo: null, loading: false, error: null });
}

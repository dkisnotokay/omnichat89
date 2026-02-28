<script lang="ts">
  /**
   * App.svelte — корневой компонент.
   *
   * Два режима Twitch:
   * 1. Без авторизации → поле ввода канала, анонимное подключение
   * 2. С авторизацией → автоподключение к каналу пользователя (+ бейджи)
   *
   * Kick: всегда анонимный (поле ввода канала).
   *
   * Оба чата могут работать одновременно — сообщения в общем потоке.
   */
  import { onMount } from "svelte";
  import "./styles/global.css";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import ChatView from "./lib/components/ChatView.svelte";
  import SettingsWindow from "./lib/components/SettingsWindow.svelte";
  import {
    twitchStatus,
    twitchChannel,
    twitchAsyncError,
    kickStatus,
    kickChannel,
    twitchMessageCount,
    kickMessageCount,
    messages,
    clearAllMessages,
    initChatListeners,
    connectTwitch,
    disconnectTwitch,
    connectKick,
    disconnectKick,
  } from "./lib/stores/chat";
  import { settings, applyCssVariables, initSettingsSync } from "./lib/stores/settings";
  import { auth, initAuth, loginTwitch, logoutTwitch } from "./lib/stores/auth";
  import { initBadgeListener } from "./lib/stores/chat";
  import { ttsStatus, initTtsListeners, ttsSkip, ttsClearQueue } from "./lib/stores/tts";
  import { getStrings } from "./lib/i18n";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import twitchIcon from "./lib/assets/twitch-icon.svg";
  import kickIcon from "./lib/assets/kick-icon.svg";

  /** Ссылка на компонент контекстного меню */
  let contextMenu = $state<ContextMenu>();

  let t = $derived(getStrings($settings.language));

  /** Определяем: это окно настроек или главное? */
  const isSettingsView =
    new URLSearchParams(window.location.search).get("view") === "settings";

  /** Имя канала из поля ввода (для анонимного режима Twitch) */
  let twitchInput = $state("");

  /** Имя канала из поля ввода (Kick) */
  let kickInput = $state("");

  /** Флаг: автоподключение Twitch уже было выполнено */
  let autoConnected = $state(false);

  /** Флаг: идёт процесс выхода (отключение + очистка) */
  let loggingOut = $state(false);

  /** Флаг: идёт отключение от Kick */
  let kickDisconnecting = $state(false);

  /** Сообщение об ошибке Twitch (отображается 4 секунды) */
  let twitchError = $state("");

  /** Сообщение об ошибке Kick (отображается 4 секунды) */
  let kickError = $state("");

  /** Таймер автоочистки ошибок */
  let twitchErrorTimer: ReturnType<typeof setTimeout> | null = null;
  let kickErrorTimer: ReturnType<typeof setTimeout> | null = null;

  /** Показать ошибку Twitch на 4 секунды */
  function showTwitchError(msg: string) {
    twitchError = msg;
    if (twitchErrorTimer) clearTimeout(twitchErrorTimer);
    twitchErrorTimer = setTimeout(() => { twitchError = ""; }, 4000);
  }

  /** Показать ошибку Kick на 4 секунды */
  function showKickError(msg: string) {
    kickError = msg;
    if (kickErrorTimer) clearTimeout(kickErrorTimer);
    kickErrorTimer = setTimeout(() => { kickError = ""; }, 4000);
  }

  onMount(() => {
    // Контекстное меню: блокируем везде, кроме полей ввода каналов
    document.addEventListener("contextmenu", (e) => {
      const target = e.target as HTMLElement;
      if (target.classList.contains("channel-input") && target instanceof HTMLInputElement) {
        e.preventDefault();
        contextMenu?.show(e, target);
      } else {
        e.preventDefault();
      }
    });

    if (isSettingsView) {
      initSettingsSync();
    } else {
      initChatListeners();
      initBadgeListener();
      initTtsListeners();

      // Ждём загрузки настроек и авторизации, затем авто-подключаемся к сохранённым каналам
      Promise.all([initSettingsSync(), initAuth()]).then(() => {
        // Twitch: если нет OAuth → подключаемся к последнему каналу
        if (!$auth.userInfo && $settings.lastTwitchChannel) {
          twitchInput = $settings.lastTwitchChannel;
          connectTwitch($settings.lastTwitchChannel);
        }
        // Kick: всегда подключаемся к последнему каналу
        if ($settings.lastKickChannel) {
          kickInput = $settings.lastKickChannel;
          connectKick($settings.lastKickChannel);
        }
      });
    }
  });

  /** Показываем асинхронные ошибки Twitch (ROOMSTATE timeout и др.) */
  $effect(() => {
    const err = $twitchAsyncError;
    if (err) {
      showTwitchError(err);
      twitchAsyncError.set("");
    }
  });

  /** Автоподключение Twitch при авторизации */
  $effect(() => {
    if (
      !isSettingsView &&
      $auth.userInfo &&
      !autoConnected &&
      !loggingOut &&
      $twitchStatus === "disconnected"
    ) {
      autoConnected = true;
      connectTwitch($auth.userInfo.login);
    }
  });

  /** Применяем CSS-переменные и alwaysOnTop только в главном окне */
  $effect(() => {
    if (!isSettingsView) {
      applyCssVariables($settings);
      getCurrentWindow().setAlwaysOnTop($settings.alwaysOnTop).catch(() => {});
    }
  });

  // ──────────────────────────────────────────────────────────
  // Утилиты
  // ──────────────────────────────────────────────────────────

  /** Известные хостнеймы платформ */
  const PLATFORM_HOSTS: Record<string, string> = {
    "twitch.tv": "Twitch",
    "www.twitch.tv": "Twitch",
    "kick.com": "Kick",
    "www.kick.com": "Kick",
  };

  /**
   * Извлекает slug канала из URL или возвращает как есть.
   * Возвращает { slug, error }.
   * Если вставлен URL чужой платформы — возвращает ошибку.
   */
  function parseChannelInput(
    input: string,
    expectedHost: string,
    platformName: string
  ): { slug: string; error: string } {
    const value = input.trim().toLowerCase();
    if (!value) return { slug: "", error: "" };

    try {
      const url = new URL(value.startsWith("http") ? value : `https://${value}`);
      const host = url.hostname;

      // Проверяем, не вставлена ли ссылка другой платформы
      const detectedPlatform = PLATFORM_HOSTS[host];
      if (detectedPlatform && host !== expectedHost && host !== `www.${expectedHost}`) {
        const errMsg = $settings.language === "en"
          ? `This is a ${detectedPlatform} link. Paste it in the ${detectedPlatform} field.`
          : `Это ссылка на ${detectedPlatform}. Вставьте в поле ${detectedPlatform}.`;
        return { slug: "", error: errMsg };
      }

      // Извлекаем slug из URL правильной платформы
      if (host === expectedHost || host === `www.${expectedHost}`) {
        const slug = url.pathname.replace(/^\//, "").split("/")[0];
        if (!slug) return { slug: "", error: $settings.language === "en" ? "Could not extract channel from URL" : "Не удалось извлечь канал из URL" };
        return { slug, error: "" };
      }
    } catch {
      // Не URL — используем как slug
    }

    return { slug: value, error: "" };
  }

  // ──────────────────────────────────────────────────────────
  // Twitch handlers
  // ──────────────────────────────────────────────────────────

  /** Подключиться / отключиться от Twitch (анонимный режим) */
  async function handleTwitchConnect() {
    twitchError = "";

    if ($twitchStatus === "connected") {
      loggingOut = true;
      try {
        await disconnectTwitch();
        settings.update(s => ({ ...s, lastTwitchChannel: "" }));
        if ($kickStatus !== "connected") {
          clearAllMessages();
        }
        twitchInput = "";
      } finally {
        loggingOut = false;
      }
    } else if ($twitchStatus === "disconnected") {
      const { slug, error } = parseChannelInput(twitchInput, "twitch.tv", "Twitch");
      if (error) {
        showTwitchError(error);
        return;
      }
      if (slug) {
        settings.update(s => ({ ...s, lastTwitchChannel: slug }));
        try {
          await connectTwitch(slug);
        } catch (e) {
          showTwitchError(String(e));
        }
      }
    }
  }

  /** Войти через Twitch */
  async function handleLogin() {
    if ($twitchStatus === "connected") {
      await disconnectTwitch();
      twitchInput = "";
    }
    await loginTwitch();
  }

  /** Выйти из аккаунта: отключиться + очистить чат + очистить auth */
  async function handleLogout() {
    loggingOut = true;
    try {
      await disconnectTwitch();
      settings.update(s => ({ ...s, lastTwitchChannel: "" }));
      if ($kickStatus !== "connected") {
        clearAllMessages();
      }
      autoConnected = false;
      twitchInput = "";
      await logoutTwitch();
    } finally {
      loggingOut = false;
    }
  }

  function handleTwitchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      handleTwitchConnect();
    }
  }

  // ──────────────────────────────────────────────────────────
  // Kick handlers
  // ──────────────────────────────────────────────────────────

  /** Подключиться / отключиться от Kick */
  async function handleKickConnect() {
    kickError = "";

    if ($kickStatus === "connected") {
      kickDisconnecting = true;
      try {
        await disconnectKick();
        settings.update(s => ({ ...s, lastKickChannel: "" }));
        if ($twitchStatus !== "connected") {
          clearAllMessages();
        }
        kickInput = "";
      } finally {
        kickDisconnecting = false;
      }
    } else if ($kickStatus === "disconnected") {
      const { slug, error } = parseChannelInput(kickInput, "kick.com", "Kick");
      if (error) {
        showKickError(error);
        return;
      }
      if (slug) {
        settings.update(s => ({ ...s, lastKickChannel: slug }));
        try {
          await connectKick(slug);
        } catch (e) {
          showKickError(String(e));
        }
      }
    }
  }

  function handleKickKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      handleKickConnect();
    }
  }
</script>

<!-- Окно настроек -->
{#if isSettingsView}
  <SettingsWindow />

<!-- Главное окно (чат) -->
{:else}
  <main>
    <TitleBar />

    <!-- ═══════════════════════════════════════════════════ -->
    <!-- TWITCH BAR                                         -->
    <!-- ═══════════════════════════════════════════════════ -->

    <!-- Состояние: Идёт выход из аккаунта Twitch -->
    {#if loggingOut}
      <div class="connect-bar">
        <img class="platform-badge-icon" src={twitchIcon} alt="Twitch" />
        <span class="status-text busy">{t.disconnecting}</span>
      </div>

    <!-- Состояние: Идёт авторизация (ждём OAuth callback) -->
    {:else if $auth.loading}
      <div class="connect-bar">
        <img class="platform-badge-icon" src={twitchIcon} alt="Twitch" />
        <span class="status-text busy">{t.authorizing}</span>
      </div>

    <!-- Режим: Авторизован через Twitch -->
    {:else if $auth.userInfo}
      <div class="connect-bar auth-bar">
        <img class="platform-badge-icon" src={twitchIcon} alt="Twitch" />
        <span class="auth-user">{$auth.userInfo.display_name}</span>

        {#if $twitchStatus === "connecting"}
          <span class="status-text busy">{t.connecting}</span>
        {:else if $twitchStatus === "connected"}
          <span class="status-dot connected"></span>
          <span class="channel-name">#{$twitchChannel}</span>
          <span class="msg-count">{$twitchMessageCount} {t.msgCount}</span>
        {/if}

        <button
          class="auth-btn logout"
          onclick={handleLogout}
          disabled={$twitchStatus === "connecting"}
        >{t.logout}</button>
      </div>

    <!-- Режим: Без авторизации Twitch -->
    {:else}
      <!-- Анонимное подключение: идёт подключение -->
      {#if $twitchStatus === "connecting"}
        <div class="connect-bar">
          <img class="platform-badge-icon" src={twitchIcon} alt="Twitch" />
          <span class="status-text busy">{t.connectingTo} #{twitchInput}...</span>
        </div>

      <!-- Анонимное подключение: ввод канала / управление -->
      {:else}
        <div class="connect-bar">
          <img class="platform-badge-icon" src={twitchIcon} alt="Twitch" />
          <input
            type="text"
            class="channel-input"
            class:input-error={!!twitchError}
            placeholder={t.twitchPlaceholder}
            bind:value={twitchInput}
            onkeydown={handleTwitchKeydown}
            oninput={() => { twitchError = ""; }}
            disabled={$twitchStatus === "connected"}
          />
          <button
            class="connect-btn"
            class:connected={$twitchStatus === "connected"}
            onclick={handleTwitchConnect}
          >
            {#if $twitchStatus === "connected"}
              ✕
            {:else}
              →
            {/if}
          </button>
        </div>

        {#if twitchError}
          <div class="error-bar">{twitchError}</div>
        {:else}
          <div class="status-bar">
            <button
              class="auth-btn login"
              onclick={handleLogin}
              disabled={$twitchStatus === "connected"}
            >
              {t.loginTwitch}
            </button>

            {#if $twitchStatus === "connected"}
              <span class="status-separator">│</span>
              <span class="status-dot connected"></span>
              <span>#{$twitchChannel}</span>
              <span class="msg-count">{$twitchMessageCount} {t.msgCount}</span>
            {/if}
          </div>
        {/if}
      {/if}
    {/if}

    <!-- ═══════════════════════════════════════════════════ -->
    <!-- KICK BAR                                           -->
    <!-- ═══════════════════════════════════════════════════ -->

    <!-- Kick: идёт отключение -->
    {#if kickDisconnecting}
      <div class="connect-bar">
        <img class="platform-badge-icon" src={kickIcon} alt="Kick" />
        <span class="status-text busy">{t.disconnecting}</span>
      </div>

    <!-- Kick: идёт подключение -->
    {:else if $kickStatus === "connecting"}
      <div class="connect-bar">
        <img class="platform-badge-icon" src={kickIcon} alt="Kick" />
        <span class="status-text busy">{t.connectingTo} #{kickInput}...</span>
      </div>

    <!-- Kick: ввод канала / подключён -->
    {:else}
      <div class="connect-bar">
        <img class="platform-badge-icon" src={kickIcon} alt="Kick" />
        <input
          type="text"
          class="channel-input"
          class:input-error={!!kickError}
          placeholder={t.kickPlaceholder}
          bind:value={kickInput}
          onkeydown={handleKickKeydown}
          oninput={() => { kickError = ""; }}
          disabled={$kickStatus === "connected"}
        />
        <button
          class="connect-btn"
          class:connected={$kickStatus === "connected"}
          onclick={handleKickConnect}
        >
          {#if $kickStatus === "connected"}
            ✕
          {:else}
            →
          {/if}
        </button>
      </div>

      {#if kickError}
        <div class="error-bar">{kickError}</div>
      {:else if $kickStatus === "connected"}
        <div class="status-bar">
          <span class="status-dot connected"></span>
          <span>#{$kickChannel}</span>
          <span class="msg-count">{$kickMessageCount} {t.msgCount}</span>
        </div>
      {/if}
    {/if}

    <!-- TTS Controls (видимый когда TTS включён) -->
    {#if $settings.ttsEnabled}
      <div class="tts-bar">
        <span class="tts-label">TTS</span>
        {#if $ttsStatus.is_speaking}
          <span class="tts-speaking">&#9654;</span>
        {/if}
        {#if $ttsStatus.queue_size > 0}
          <span class="tts-queue">{t.ttsQueue}: {$ttsStatus.queue_size}</span>
        {/if}
        <div class="tts-buttons">
          <button class="tts-btn" onclick={() => ttsSkip()}>{t.ttsSkip}</button>
          <button class="tts-btn" onclick={() => ttsClearQueue()}>{t.ttsClear}</button>
        </div>
      </div>
    {/if}

    <ChatView />
    <ContextMenu bind:this={contextMenu} />
  </main>
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    color: var(--text-color, #e0e0e0);
    overflow: hidden;
    background-color: transparent;
  }

  .connect-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: #16213e;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }

  .platform-badge-icon {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: 3px;
  }

  .channel-input {
    flex: 1;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    padding: 7px 12px;
    color: var(--text-color, #e0e0e0);
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.2s;
  }

  .channel-input:focus {
    border-color: var(--accent-color, #667eea);
  }

  .channel-input:disabled {
    opacity: 0.5;
  }

  .connect-btn {
    width: 34px;
    height: 34px;
    border: none;
    border-radius: 6px;
    background: var(--accent-color, #667eea);
    color: white;
    font-size: 1.1rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.2s;
    flex-shrink: 0;
  }

  .connect-btn:hover {
    background: var(--accent-hover, #764ba2);
  }

  .connect-btn.connected {
    background: #e74c3c;
  }

  .connect-btn.connected:hover {
    background: #c0392b;
  }

  .connect-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .channel-name {
    font-size: 0.8rem;
    color: var(--text-color, #e0e0e0);
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    background: #141c30;
    font-size: 0.75rem;
    color: var(--text-muted, #888);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #888;
    flex-shrink: 0;
  }

  .status-dot.connected {
    background: #2ecc71;
  }

  .msg-count {
    margin-left: auto;
    white-space: nowrap;
  }

  .status-text.busy {
    font-size: 0.85rem;
    color: var(--text-muted, #888);
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.5; }
    50% { opacity: 1; }
  }

  /* --- Auth --- */
  .auth-bar {
    gap: 6px;
  }

  .auth-user {
    font-weight: 600;
    color: #9146ff;
    font-size: 0.85rem;
  }

  .auth-btn {
    border: none;
    border-radius: 4px;
    padding: 2px 8px;
    font-size: 0.7rem;
    cursor: pointer;
    transition: opacity 0.2s;
  }

  .auth-btn:hover {
    opacity: 0.8;
  }

  .auth-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .auth-btn.login {
    background: #9146ff;
    color: white;
  }

  .auth-btn.logout {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-muted, #888);
    margin-left: auto;
  }

  .status-separator {
    color: rgba(255, 255, 255, 0.15);
    margin: 0 2px;
  }

  /* --- Ошибки --- */
  .error-bar {
    padding: 4px 12px;
    font-size: 0.75rem;
    color: #e74c3c;
    background: #1e1520;
    border-bottom: 1px solid rgba(231, 76, 60, 0.2);
    flex-shrink: 0;
  }

  .input-error {
    border-color: #e74c3c !important;
  }

  /* --- TTS Bar --- */
  .tts-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 12px;
    background: #141c30;
    font-size: 0.8rem;
    color: var(--text-muted, #888);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .tts-label {
    font-weight: 700;
    color: var(--accent-color, #667eea);
    font-size: 0.8rem;
  }

  .tts-speaking {
    color: #53fc18;
    animation: pulse 1s ease-in-out infinite;
  }

  .tts-queue {
    font-size: 0.75rem;
  }

  .tts-buttons {
    margin-left: auto;
    display: flex;
    gap: 4px;
  }

  .tts-btn {
    border: none;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-muted, #888);
    font-size: 0.75rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    white-space: nowrap;
  }

  .tts-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    color: var(--text-color, #e0e0e0);
  }
</style>

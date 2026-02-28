<script lang="ts">
  /**
   * SettingsWindow.svelte — корневой компонент окна настроек.
   *
   * Отображается в отдельном Tauri-окне (не внутри чата).
   * Шрифт фиксирован — не зависит от настройки fontSize.
   * Включает свой title bar для перетаскивания и закрытия.
   * Поддерживает русский и английский интерфейс.
   */
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { settings, resetSettings, type AppSettings } from "../stores/settings";
  import { getStrings } from "../i18n";

  const appWindow = getCurrentWindow();

  let currentSettings = $derived($settings);
  let t = $derived(getStrings(currentSettings.language));

  function update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    settings.update((s) => ({ ...s, [key]: value }));
  }

  /** Переключить язык + сменить голос на подходящий */
  function switchLanguage(lang: "ru" | "en") {
    if (currentSettings.language === lang) return;
    settings.update((s) => {
      const updated = { ...s, language: lang };
      // Автосмена голоса при переключении языка
      if (lang === "en" && s.ttsVoice.startsWith("ru-")) {
        updated.ttsVoice = "en-US-ChristopherNeural";
      } else if (lang === "ru" && s.ttsVoice.startsWith("en-")) {
        updated.ttsVoice = "ru-RU-DmitryNeural";
      }
      return updated;
    });
  }

  async function toggleAlwaysOnTop() {
    const newVal = !currentSettings.alwaysOnTop;
    update("alwaysOnTop", newVal);
  }

  function close() {
    appWindow.close();
  }

  let overlayUrl = $derived(`http://localhost:${currentSettings.overlayPort}/overlay?token=${currentSettings.overlaySecret}`);
  let maskedOverlayUrl = $derived(`http://localhost:${currentSettings.overlayPort}/overlay?token=${"*".repeat(8)}`);
  let copySuccess = $state(false);

  async function copyOverlayUrl() {
    try {
      await writeText(overlayUrl);
      copySuccess = true;
      setTimeout(() => { copySuccess = false; }, 2000);
    } catch { /* ignore */ }
  }

  function handleReset() {
    resetSettings();
  }
</script>

<div class="settings-window">
  <!-- Title bar окна настроек -->
  <div class="title-bar" data-tauri-drag-region>
    <span class="title" data-tauri-drag-region>{t.settings}</span>
    <button class="close-btn" onclick={close} title={t.close}>✕</button>
  </div>

  <!-- Контент настроек -->
  <div class="settings-body">
    <!-- Язык интерфейса -->
    <div class="section">
      <h3 class="section-title">{t.langSection}</h3>
      <div class="setting-row">
        <div class="lang-buttons">
          <button
            class="lang-btn"
            class:active={currentSettings.language === "ru"}
            onclick={() => switchLanguage("ru")}
          >🇷🇺 Русский</button>
          <button
            class="lang-btn"
            class:active={currentSettings.language === "en"}
            onclick={() => switchLanguage("en")}
          >🇬🇧 English</button>
        </div>
      </div>
    </div>

    <!-- Внешний вид -->
    <div class="section">
      <h3 class="section-title">{t.appearance}</h3>

      <div class="setting-row">
        <label class="setting-label" for="fontSize">
          {t.fontSize}
          <span class="setting-value">{currentSettings.fontSize}px</span>
        </label>
        <input
          id="fontSize"
          type="range"
          min="10"
          max="32"
          step="1"
          value={currentSettings.fontSize}
          oninput={(e) => update("fontSize", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label" for="appBgOpacity">
          {t.appBgOpacity}
          <span class="setting-value">{currentSettings.appBgOpacity}%</span>
        </label>
        <input
          id="appBgOpacity"
          type="range"
          min="0"
          max="100"
          step="5"
          value={currentSettings.appBgOpacity}
          oninput={(e) => update("appBgOpacity", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label" for="bgOpacity">
          {t.bgOpacity}
          <span class="setting-value">{currentSettings.bgOpacity}%</span>
        </label>
        <input
          id="bgOpacity"
          type="range"
          min="0"
          max="100"
          step="5"
          value={currentSettings.bgOpacity}
          oninput={(e) => update("bgOpacity", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <div class="setting-row color-row">
        <label class="setting-label" for="textColor">{t.textColor}</label>
        <div class="color-picker-wrap">
          <input
            id="textColor"
            type="color"
            value={currentSettings.textColor}
            oninput={(e) => update("textColor", e.currentTarget.value)}
            class="color-input"
          />
          <span class="color-hex">{currentSettings.textColor}</span>
        </div>
      </div>

      <div class="setting-row color-row">
        <label class="setting-label" for="bgColor">{t.bgColor}</label>
        <div class="color-picker-wrap">
          <input
            id="bgColor"
            type="color"
            value={currentSettings.bgColor}
            oninput={(e) => update("bgColor", e.currentTarget.value)}
            class="color-input"
          />
          <span class="color-hex">{currentSettings.bgColor}</span>
        </div>
      </div>
    </div>

    <!-- Отображение -->
    <div class="section">
      <h3 class="section-title">{t.display}</h3>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.showTimestamp}</span>
        <button
          class="toggle"
          class:active={currentSettings.showTimestamp}
          onclick={() => update("showTimestamp", !currentSettings.showTimestamp)}
          aria-label={t.showTimestamp}
          role="switch"
          aria-checked={currentSettings.showTimestamp}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">
          {t.showBadges}
          <span class="setting-hint">{t.showBadgesHint}</span>
        </span>
        <button
          class="toggle"
          class:active={currentSettings.showBadges}
          onclick={() => update("showBadges", !currentSettings.showBadges)}
          aria-label={t.showBadges}
          role="switch"
          aria-checked={currentSettings.showBadges}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.platformIcon}</span>
        <button
          class="toggle"
          class:active={currentSettings.showPlatformIcon}
          onclick={() => update("showPlatformIcon", !currentSettings.showPlatformIcon)}
          aria-label={t.platformIcon}
          role="switch"
          aria-checked={currentSettings.showPlatformIcon}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">
          {t.showSystemEvents}
          <span class="setting-hint">{t.showSystemEventsHint}</span>
        </span>
        <button
          class="toggle"
          class:active={currentSettings.showSystemEvents}
          onclick={() => update("showSystemEvents", !currentSettings.showSystemEvents)}
          aria-label={t.showSystemEvents}
          role="switch"
          aria-checked={currentSettings.showSystemEvents}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>
    </div>

    <!-- Окно -->
    <div class="section">
      <h3 class="section-title">{t.window}</h3>

      <div class="setting-row toggle-row">
        <span class="setting-label">
          {t.alwaysOnTop}
          <span class="setting-hint">{t.alwaysOnTopHint}</span>
        </span>
        <button
          class="toggle"
          class:active={currentSettings.alwaysOnTop}
          onclick={toggleAlwaysOnTop}
          aria-label={t.alwaysOnTop}
          role="switch"
          aria-checked={currentSettings.alwaysOnTop}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>
    </div>

    <!-- OBS Overlay -->
    <div class="section">
      <h3 class="section-title">{t.obsOverlay}</h3>

      <div class="setting-row">
        <label class="setting-label" for="overlayPort">{t.overlayPort}</label>
        <input
          id="overlayPort"
          type="number"
          min="1024"
          max="65535"
          value={currentSettings.overlayPort}
          oninput={(e) => {
            const v = Number(e.currentTarget.value);
            if (v >= 1024 && v <= 65535) update("overlayPort", v);
          }}
          class="text-input port-input"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label">{t.overlayUrl}</label>
        <div class="overlay-url-row">
          <code class="overlay-url">{maskedOverlayUrl}</code>
          <button class="copy-btn" onclick={copyOverlayUrl}>
            {copySuccess ? "✓" : t.copyUrl}
          </button>
        </div>
        <span class="setting-hint obs-hint">{t.obsHint}</span>
        <span class="setting-hint obs-hint">{t.obsDimensions}</span>
      </div>
    </div>

    <!-- TTS Озвучка -->
    <div class="section">
      <h3 class="section-title">{t.ttsSection}</h3>

      <!-- Мастер вкл/выкл -->
      <div class="setting-row toggle-row">
        <span class="setting-label">{t.enableTts}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsEnabled}
          onclick={() => update("ttsEnabled", !currentSettings.ttsEnabled)}
          role="switch"
          aria-checked={currentSettings.ttsEnabled}
          aria-label={t.enableTts}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <!-- Голос -->
      <div class="setting-row">
        <label class="setting-label" for="ttsVoice">{t.voice}</label>
        <select
          id="ttsVoice"
          class="select-input"
          value={currentSettings.ttsVoice}
          onchange={(e) => update("ttsVoice", e.currentTarget.value)}
        >
          {#if currentSettings.language === "ru"}
            <option value="ru-RU-DmitryNeural">Дмитрий</option>
            <option value="ru-RU-SvetlanaNeural">Светлана</option>
          {:else}
            <option value="en-US-ChristopherNeural">Christopher</option>
            <option value="en-US-JennyNeural">Jenny</option>
            <option value="en-US-GuyNeural">Guy</option>
            <option value="en-US-AriaNeural">Aria</option>
          {/if}
          <option value="random">{t.random}</option>
        </select>
      </div>

      <!-- Скорость -->
      <div class="setting-row">
        <label class="setting-label" for="ttsRate">
          {t.speed}
          <span class="setting-value">{currentSettings.ttsRate > 0 ? "+" : ""}{currentSettings.ttsRate}%</span>
        </label>
        <input
          id="ttsRate"
          type="range"
          min="-50"
          max="100"
          step="10"
          value={currentSettings.ttsRate}
          oninput={(e) => update("ttsRate", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <!-- Громкость -->
      <div class="setting-row">
        <label class="setting-label" for="ttsVolume">
          {t.volume}
          <span class="setting-value">{currentSettings.ttsVolume}%</span>
        </label>
        <input
          id="ttsVolume"
          type="range"
          min="0"
          max="100"
          step="5"
          value={currentSettings.ttsVolume}
          oninput={(e) => update("ttsVolume", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <!-- Очередь -->
      <div class="setting-row">
        <label class="setting-label" for="ttsQueue">
          {t.messageQueue}
          <span class="setting-value">{currentSettings.ttsMaxQueueSize}</span>
        </label>
        <input
          id="ttsQueue"
          type="range"
          min="5"
          max="50"
          step="5"
          value={currentSettings.ttsMaxQueueSize}
          oninput={(e) => update("ttsMaxQueueSize", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <!-- Пауза -->
      <div class="setting-row">
        <label class="setting-label" for="ttsPause">
          {t.pauseBetween}
          <span class="setting-value">{currentSettings.ttsPauseMs}{t.msUnit}</span>
        </label>
        <input
          id="ttsPause"
          type="range"
          min="0"
          max="2000"
          step="100"
          value={currentSettings.ttsPauseMs}
          oninput={(e) => update("ttsPauseMs", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>

      <!-- Ограничение длины -->
      <div class="setting-row">
        <label class="setting-label" for="ttsMaxLen">
          {t.textLengthLimit}
          <span class="setting-value">{currentSettings.ttsMaxMessageLength}</span>
        </label>
        <input
          id="ttsMaxLen"
          type="range"
          min="50"
          max="500"
          step="50"
          value={currentSettings.ttsMaxMessageLength}
          oninput={(e) => update("ttsMaxMessageLength", Number(e.currentTarget.value))}
          class="slider"
        />
      </div>
    </div>

    <!-- TTS: Что озвучивать -->
    <div class="section">
      <h3 class="section-title">{t.ttsWhat}</h3>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readAll}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadAll}
          onclick={() => update("ttsReadAll", !currentSettings.ttsReadAll)}
          role="switch"
          aria-checked={currentSettings.ttsReadAll}
          aria-label={t.readAll}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row" class:disabled-row={currentSettings.ttsReadAll}>
        <span class="setting-label">{t.readSubscribers}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadSubscribers}
          onclick={() => !currentSettings.ttsReadAll && update("ttsReadSubscribers", !currentSettings.ttsReadSubscribers)}
          role="switch"
          aria-checked={currentSettings.ttsReadSubscribers}
          disabled={currentSettings.ttsReadAll}
          aria-label={t.readSubscribers}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row" class:disabled-row={currentSettings.ttsReadAll}>
        <span class="setting-label">{t.readVip}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadVip}
          onclick={() => !currentSettings.ttsReadAll && update("ttsReadVip", !currentSettings.ttsReadVip)}
          role="switch"
          aria-checked={currentSettings.ttsReadVip}
          disabled={currentSettings.ttsReadAll}
          aria-label={t.readVip}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row" class:disabled-row={currentSettings.ttsReadAll}>
        <span class="setting-label">{t.readModerators}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadModerators}
          onclick={() => !currentSettings.ttsReadAll && update("ttsReadModerators", !currentSettings.ttsReadModerators)}
          role="switch"
          aria-checked={currentSettings.ttsReadModerators}
          disabled={currentSettings.ttsReadAll}
          aria-label={t.readModerators}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readUsernames}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadUsernames}
          onclick={() => update("ttsReadUsernames", !currentSettings.ttsReadUsernames)}
          role="switch"
          aria-checked={currentSettings.ttsReadUsernames}
          aria-label={t.readUsernames}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readReplies}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadReplies}
          onclick={() => update("ttsReadReplies", !currentSettings.ttsReadReplies)}
          role="switch"
          aria-checked={currentSettings.ttsReadReplies}
          aria-label={t.readReplies}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readHighlighted}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadHighlighted}
          onclick={() => update("ttsReadHighlighted", !currentSettings.ttsReadHighlighted)}
          role="switch"
          aria-checked={currentSettings.ttsReadHighlighted}
          aria-label={t.readHighlighted}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readLinks}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadLinks}
          onclick={() => update("ttsReadLinks", !currentSettings.ttsReadLinks)}
          role="switch"
          aria-checked={currentSettings.ttsReadLinks}
          aria-label={t.readLinks}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.readEmotes}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsReadEmotes}
          onclick={() => update("ttsReadEmotes", !currentSettings.ttsReadEmotes)}
          role="switch"
          aria-checked={currentSettings.ttsReadEmotes}
          aria-label={t.readEmotes}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>
    </div>

    <!-- TTS: Ключевые слова -->
    <div class="section">
      <h3 class="section-title">{t.ttsKeywords}</h3>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.useKeywords}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsUseKeywords}
          onclick={() => update("ttsUseKeywords", !currentSettings.ttsUseKeywords)}
          role="switch"
          aria-checked={currentSettings.ttsUseKeywords}
          aria-label={t.useKeywords}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>

      <div class="setting-row">
        <label class="setting-label" for="ttsKeywords">{t.keywordsLabel}</label>
        <input
          id="ttsKeywords"
          type="text"
          class="text-input"
          value={currentSettings.ttsKeywords}
          oninput={(e) => update("ttsKeywords", e.currentTarget.value)}
          placeholder="!say !s"
        />
      </div>

      <div class="setting-row toggle-row">
        <span class="setting-label">{t.stripKeywords}</span>
        <button
          class="toggle"
          class:active={currentSettings.ttsStripKeywords}
          onclick={() => update("ttsStripKeywords", !currentSettings.ttsStripKeywords)}
          role="switch"
          aria-checked={currentSettings.ttsStripKeywords}
          aria-label={t.stripKeywords}
        >
          <span class="toggle-knob"></span>
        </button>
      </div>
    </div>

    <!-- TTS: Фильтры -->
    <div class="section">
      <h3 class="section-title">{t.ttsFilters}</h3>

      <div class="setting-row">
        <label class="setting-label" for="ttsIgnore">{t.ignoreSymbols}</label>
        <input
          id="ttsIgnore"
          type="text"
          class="text-input"
          value={currentSettings.ttsIgnoreSymbols}
          oninput={(e) => update("ttsIgnoreSymbols", e.currentTarget.value)}
          placeholder="@"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label" for="ttsWordFilter">{t.wordFilter}</label>
        <input
          id="ttsWordFilter"
          type="text"
          class="text-input"
          value={currentSettings.ttsWordFilter}
          oninput={(e) => update("ttsWordFilter", e.currentTarget.value)}
          placeholder={currentSettings.language === "ru" ? "слово1, слово2" : "word1, word2"}
        />
      </div>

      <div class="setting-row">
        <label class="setting-label" for="ttsBlacklist">{t.blacklist}</label>
        <input
          id="ttsBlacklist"
          type="text"
          class="text-input"
          value={currentSettings.ttsBlacklist}
          oninput={(e) => update("ttsBlacklist", e.currentTarget.value)}
          placeholder="Nightbot, Moobot, StreamElements"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label" for="ttsWhitelist">{t.whitelist}</label>
        <input
          id="ttsWhitelist"
          type="text"
          class="text-input"
          value={currentSettings.ttsWhitelist}
          oninput={(e) => update("ttsWhitelist", e.currentTarget.value)}
          placeholder="username1, username2"
        />
      </div>
    </div>
  </div>

  <!-- Подвал: сброс + версия -->
  <div class="settings-footer">
    <button class="reset-btn" onclick={handleReset}>{t.resetSettings}</button>
    <span class="version">Omnichat89 v0.1.0</span>
  </div>
</div>

<style>
  .settings-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-secondary, #16213e);
    color: var(--text-color, #e0e0e0);
    /* Фиксированный шрифт — не зависит от настройки */
    font-size: 14px;
    overflow: hidden;
  }

  /* --- Title bar --- */
  .title-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 12px;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    -webkit-app-region: drag;
    user-select: none;
    flex-shrink: 0;
  }

  .title {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-muted, #888);
  }

  .close-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-muted, #888);
    font-size: 0.8rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    -webkit-app-region: no-drag;
  }

  .close-btn:hover {
    background: #e74c3c;
    color: white;
  }

  /* --- Контент --- */
  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .section {
    padding: 8px 16px 16px;
  }

  .section-title {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    color: var(--accent-color, #667eea);
    margin-bottom: 10px;
  }

  .setting-row {
    margin-bottom: 12px;
  }

  .setting-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.82rem;
    color: var(--text-color, #e0e0e0);
    margin-bottom: 6px;
  }

  .setting-value {
    font-size: 0.75rem;
    color: var(--text-muted, #888);
    font-weight: 500;
    min-width: 40px;
    text-align: right;
  }

  .setting-hint {
    font-size: 0.7rem;
    color: var(--text-muted, #888);
    font-weight: 400;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .toggle-row .setting-label {
    margin-bottom: 0;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }

  /* --- Color picker --- */
  .color-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .color-row .setting-label {
    margin-bottom: 0;
  }

  .color-picker-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .color-input {
    width: 28px;
    height: 28px;
    border: 2px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    padding: 0;
  }

  .color-input::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .color-input::-webkit-color-swatch {
    border: none;
    border-radius: 4px;
  }

  .color-hex {
    font-size: 0.72rem;
    color: var(--text-muted, #888);
    font-family: monospace;
    min-width: 56px;
  }

  /* --- Слайдер --- */
  .slider {
    width: 100%;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 2px;
    outline: none;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent-color, #667eea);
    cursor: pointer;
  }

  /* --- Toggle --- */
  .toggle {
    position: relative;
    width: 40px;
    height: 22px;
    border: none;
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.12);
    cursor: pointer;
    transition: background 0.2s;
    flex-shrink: 0;
    padding: 0;
  }

  .toggle.active {
    background: var(--accent-color, #667eea);
  }

  .toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: white;
    transition: transform 0.2s;
  }

  .toggle.active .toggle-knob {
    transform: translateX(18px);
  }

  /* --- Language buttons --- */
  .lang-buttons {
    display: flex;
    gap: 6px;
    width: 100%;
  }

  .lang-btn {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-muted, #888);
    font-size: 0.82rem;
    cursor: pointer;
    transition: background 0.2s, color 0.2s, border-color 0.2s;
  }

  .lang-btn.active {
    background: var(--accent-color, #667eea);
    color: white;
    border-color: var(--accent-color, #667eea);
  }

  .lang-btn:not(.active):hover {
    background: rgba(255, 255, 255, 0.08);
  }

  /* --- Select dropdown --- */
  .select-input {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: #1a1a2e;
    color: var(--text-color, #e0e0e0);
    font-size: 0.82rem;
    outline: none;
    cursor: pointer;
  }

  .select-input option {
    background: #1a1a2e;
    color: var(--text-color, #e0e0e0);
  }

  .select-input:focus {
    border-color: var(--accent-color, #667eea);
  }

  /* --- Text input --- */
  .text-input {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-color, #e0e0e0);
    font-size: 0.82rem;
    outline: none;
    box-sizing: border-box;
  }

  .text-input:focus {
    border-color: var(--accent-color, #667eea);
  }

  .text-input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  /* --- Port input (без стандартных стрелок) --- */
  .port-input {
    width: 100px;
    text-align: center;
    -moz-appearance: textfield;
  }

  .port-input::-webkit-inner-spin-button,
  .port-input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  /* --- OBS Overlay URL --- */
  .overlay-url-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .overlay-url {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--accent-color, #667eea);
    font-size: 0.78rem;
    font-family: monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .copy-btn {
    padding: 6px 10px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-color, #e0e0e0);
    font-size: 0.78rem;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.2s;
    flex-shrink: 0;
  }

  .copy-btn:hover {
    background: var(--accent-color, #667eea);
    color: white;
    border-color: var(--accent-color, #667eea);
  }

  .obs-hint {
    display: block;
    margin-top: 2px;
  }

  /* --- Disabled row (когда "Озвучивать все" включён) --- */
  .disabled-row {
    opacity: 0.4;
    pointer-events: none;
  }

  /* --- Подвал --- */
  .settings-footer {
    padding: 12px 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .reset-btn {
    width: 100%;
    padding: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted, #888);
    font-size: 0.8rem;
    cursor: pointer;
    transition: background 0.2s, color 0.2s;
  }

  .reset-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-color, #e0e0e0);
  }

  .version {
    font-size: 0.7rem;
    color: var(--text-muted, #888);
  }
</style>

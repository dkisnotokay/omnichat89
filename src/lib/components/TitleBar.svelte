<script lang="ts">
  /**
   * TitleBar.svelte — кастомный заголовок окна.
   *
   * Заменяет стандартный title bar Windows.
   * Содержит: название, кнопку настроек, свернуть, закрыть.
   * Поддерживает перетаскивание окна.
   */
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { settings } from "../stores/settings";
  import { getStrings } from "../i18n";

  const appWindow = getCurrentWindow();
  let t = $derived(getStrings($settings.language));
  let settingsWindow: WebviewWindow | null = null;

  /** Свернуть окно */
  async function minimize() {
    await appWindow.minimize();
  }

  /** Закрыть приложение */
  async function close() {
    await appWindow.close();
  }

  /** Создать новое окно настроек */
  async function createSettingsWindow() {
    const pos = await appWindow.outerPosition();
    const size = await appWindow.outerSize();

    const win = new WebviewWindow("settings", {
      url: "/?view=settings",
      title: "Настройки",
      width: 280,
      height: size.height,
      x: pos.x + size.width + 4,
      y: pos.y,
      resizable: true,
      decorations: false,
      transparent: true,
      minWidth: 240,
      minHeight: 300,
    });

    // Сбрасываем ссылку когда окно закрывается
    win.once("tauri://destroyed", () => {
      settingsWindow = null;
    });

    settingsWindow = win;
  }

  /** Открыть окно настроек справа от главного окна */
  async function openSettings() {
    // Если окно уже открыто — пробуем фокус
    if (settingsWindow) {
      try {
        await settingsWindow.setFocus();
        return;
      } catch {
        // Окно было закрыто, но событие не сработало — создаём заново
        settingsWindow = null;
      }
    }

    try {
      await createSettingsWindow();
    } catch (e) {
      console.error("Failed to open settings window:", e);
    }
  }
</script>

<!-- data-tauri-drag-region позволяет перетаскивать окно за эту область -->
<div class="title-bar" data-tauri-drag-region>
  <span class="title" data-tauri-drag-region>Omnichat89</span>
  <div class="title-buttons">
    <button
      class="settings-btn"
      onclick={openSettings}
      title={t.settings}
    >
      ⚙ {t.settings}
    </button>
    <button
      class="title-btn"
      onclick={minimize}
      title={t.minimize}
    >
      ─
    </button>
    <button
      class="title-btn close-btn"
      onclick={close}
      title={t.close}
    >
      ✕
    </button>
  </div>
</div>

<style>
  .title-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px;
    background: rgba(var(--bg-color-rgb, 22, 33, 62), var(--bg-opacity, 1));
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    -webkit-app-region: drag;
    user-select: none;
    flex-shrink: 0;
  }

  .title {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-muted, #888);
    letter-spacing: 0.5px;
    text-transform: uppercase;
  }

  .title-buttons {
    display: flex;
    align-items: center;
    gap: 2px;
    -webkit-app-region: no-drag;
  }

  .title-btn {
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
    transition: background 0.15s, color 0.15s;
  }

  .title-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-color, #e0e0e0);
  }

  .close-btn:hover {
    background: #e74c3c;
    color: white;
  }

  .settings-btn {
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted, #888);
    font-size: 0.72rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 4px;
    transition: background 0.15s, color 0.15s;
    -webkit-app-region: no-drag;
    white-space: nowrap;
  }

  .settings-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-color, #e0e0e0);
  }
</style>

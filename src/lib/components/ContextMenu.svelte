<script lang="ts">
  /**
   * ContextMenu.svelte — кастомное контекстное меню для полей ввода.
   *
   * Показывает: Вырезать, Копировать, Вставить, Выделить всё.
   * Использует Tauri clipboard plugin — работает без диалогов разрешений.
   */
  import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { settings } from "../stores/settings";
  import { getStrings } from "../i18n";

  let t = $derived(getStrings($settings.language));

  /** Видимо ли меню */
  let visible = $state(false);

  /** Позиция меню */
  let x = $state(0);
  let y = $state(0);

  /** Элемент input, к которому привязано меню */
  let targetInput: HTMLInputElement | null = null;

  /**
   * Показать контекстное меню для указанного input.
   * Вызывается из родительского компонента.
   */
  export function show(event: MouseEvent, input: HTMLInputElement) {
    targetInput = input;
    // Позиционируем с учётом границ окна
    const menuWidth = 180;
    const menuHeight = 144; // ~4 пункта по 36px
    x = Math.min(event.clientX, window.innerWidth - menuWidth - 4);
    y = Math.min(event.clientY, window.innerHeight - menuHeight - 4);
    visible = true;

    // Закрыть при клике вне меню
    setTimeout(() => {
      document.addEventListener("mousedown", handleOutsideClick);
      document.addEventListener("keydown", handleKeydown);
    }, 0);
  }

  /** Скрыть меню */
  function hide() {
    visible = false;
    targetInput = null;
    document.removeEventListener("mousedown", handleOutsideClick);
    document.removeEventListener("keydown", handleKeydown);
  }

  /** Закрыть при клике вне меню */
  function handleOutsideClick(e: MouseEvent) {
    const menu = document.querySelector(".ctx-menu");
    if (menu && !menu.contains(e.target as Node)) {
      hide();
    }
  }

  /** Закрыть по Escape */
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      hide();
    }
  }

  /** Вырезать выделенный текст */
  async function handleCut() {
    if (!targetInput) return;
    targetInput.focus();
    const start = targetInput.selectionStart ?? 0;
    const end = targetInput.selectionEnd ?? 0;
    if (start !== end) {
      const selected = targetInput.value.slice(start, end);
      await writeText(selected);
      // Удаляем выделенный текст — execCommand("delete") для реактивности Svelte bind:value
      document.execCommand("delete");
    }
    hide();
  }

  /** Копировать выделенный текст */
  async function handleCopy() {
    if (!targetInput) return;
    targetInput.focus();
    const start = targetInput.selectionStart ?? 0;
    const end = targetInput.selectionEnd ?? 0;
    if (start !== end) {
      const selected = targetInput.value.slice(start, end);
      await writeText(selected);
    }
    hide();
  }

  /** Вставить из буфера обмена */
  async function handlePaste() {
    if (!targetInput) return;
    targetInput.focus();
    const text = await readText();
    if (text) {
      // execCommand("insertText") корректно обновляет input value + Svelte bind:value
      document.execCommand("insertText", false, text);
    }
    hide();
  }

  /** Выделить весь текст */
  function handleSelectAll() {
    if (!targetInput) return;
    targetInput.focus();
    targetInput.select();
    hide();
  }
</script>

{#if visible}
  <div
    class="ctx-menu"
    style="left: {x}px; top: {y}px"
    role="menu"
  >
    <button class="ctx-item" onclick={handleCut} role="menuitem">
      {t.ctxCut}
    </button>
    <button class="ctx-item" onclick={handleCopy} role="menuitem">
      {t.ctxCopy}
    </button>
    <button class="ctx-item" onclick={handlePaste} role="menuitem">
      {t.ctxPaste}
    </button>
    <div class="ctx-separator"></div>
    <button class="ctx-item" onclick={handleSelectAll} role="menuitem">
      {t.ctxSelectAll}
    </button>
  </div>
{/if}

<style>
  .ctx-menu {
    position: fixed;
    z-index: 9999;
    min-width: 160px;
    background: rgba(30, 30, 40, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    padding: 4px 0;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(8px);
  }

  .ctx-item {
    display: block;
    width: 100%;
    padding: 8px 16px;
    border: none;
    background: transparent;
    color: var(--text-color, #e0e0e0);
    font-size: 0.82rem;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s;
  }

  .ctx-item:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .ctx-separator {
    height: 1px;
    margin: 4px 8px;
    background: rgba(255, 255, 255, 0.08);
  }
</style>

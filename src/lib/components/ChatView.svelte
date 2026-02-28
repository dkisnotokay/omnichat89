<script lang="ts">
  /**
   * ChatView.svelte — контейнер со списком сообщений чата.
   *
   * Функционал:
   * - Авто-скролл вниз при новых сообщениях
   * - Пауза авто-скролла, если пользователь прокрутил вверх
   * - Кнопка "прокрутить вниз" при паузе
   */
  import { tick } from "svelte";
  import { messages } from "../stores/chat";
  import { settings } from "../stores/settings";
  import { getStrings } from "../i18n";
  import ChatMessageComponent from "./ChatMessage.svelte";

  let t = $derived(getStrings($settings.language));

  /** Контейнер со скроллом */
  let chatContainer: HTMLDivElement;

  /** Включён ли авто-скролл (отключается при ручной прокрутке вверх) */
  let autoScroll = $state(true);

  /** Вычисляем rgba фон чата на основе настроек (hex → rgb) */
  let chatBgStyle = $derived(() => {
    const hex = $settings.bgColor;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `background-color: rgba(${r}, ${g}, ${b}, ${$settings.appBgOpacity / 100})`;
  });

  /** Отслеживаем новые сообщения для авто-скролла */
  $effect(() => {
    // Подписываемся на изменения messages
    const _msgs = $messages;
    if (autoScroll && chatContainer) {
      // tick() ждёт обновления DOM перед скроллом
      tick().then(() => {
        chatContainer.scrollTop = chatContainer.scrollHeight;
      });
    }
  });

  /** Обработчик скролла — определяем, скроллил ли пользователь вверх */
  function handleScroll() {
    if (!chatContainer) return;
    const { scrollTop, scrollHeight, clientHeight } = chatContainer;
    // Если пользователь в пределах 50px от низа — включаем авто-скролл
    autoScroll = scrollHeight - scrollTop - clientHeight < 50;
  }

  /** Прокрутить вниз вручную */
  function scrollToBottom() {
    if (chatContainer) {
      chatContainer.scrollTop = chatContainer.scrollHeight;
      autoScroll = true;
    }
  }
</script>

<div class="chat-view" style={chatBgStyle()}>
  <div
    class="chat-messages"
    bind:this={chatContainer}
    onscroll={handleScroll}
  >
    {#each $messages as msg (msg.id)}
      <ChatMessageComponent
        {msg}
        showTimestamp={$settings.showTimestamp}
        showBadges={$settings.showBadges}
        showPlatformIcon={$settings.showPlatformIcon}
        showSystemEvents={$settings.showSystemEvents}
      />
    {/each}

    {#if $messages.length === 0}
      <div class="empty-state">
        {t.noMessages}
      </div>
    {/if}
  </div>

  <!-- Кнопка "прокрутить вниз" при паузе авто-скролла -->
  {#if !autoScroll}
    <button class="scroll-to-bottom" onclick={scrollToBottom}>
      {t.scrollDown}
    </button>
  {/if}
</div>

<style>
  .chat-view {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .chat-messages {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 4px 0;
    font-size: var(--font-size, 14px);
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted, #888);
    font-size: 0.9rem;
  }

  .scroll-to-bottom {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--accent-color, #667eea);
    color: white;
    border: none;
    border-radius: 16px;
    padding: 6px 16px;
    font-size: 0.8rem;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    transition: opacity 0.2s;
    z-index: 10;
  }

  .scroll-to-bottom:hover {
    opacity: 0.9;
  }
</style>

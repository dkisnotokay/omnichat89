<script lang="ts">
  /**
   * ChatMessage.svelte — отображение одного сообщения чата.
   *
   * Показывает: [время] [платформа] [бейджи] ник: текст
   * Поддерживает ответы (reply) с цитатой.
   * Эмоуты отображаются как <img> (Twitch CDN).
   */
  import type { ChatMessage, EmoteRef } from "../types";
  import { badgeMap } from "../stores/chat";
  import twitchIcon from "../assets/twitch-icon.svg";
  import kickIcon from "../assets/kick-icon.svg";
  import { kickBadgeUrls } from "../assets/kick-badges";

  /** Сегмент сообщения: текст или эмоут */
  interface MessageSegment {
    type: "text" | "emote";
    content: string;
    url?: string;
  }

  /** Пропсы компонента */
  let {
    msg,
    showTimestamp = false,
    showBadges = true,
    showPlatformIcon = true,
    showSystemEvents = true,
  }: {
    msg: ChatMessage;
    showTimestamp?: boolean;
    showBadges?: boolean;
    showPlatformIcon?: boolean;
    showSystemEvents?: boolean;
  } = $props();

  /** Системное событие (sub, raid, gift) — не highlighted и не action */
  const isSystemEvent = $derived(
    msg.event_type != null && msg.event_type !== "highlighted" && msg.event_type !== "action"
  );
  /** Выделенное сообщение (Channel Points) */
  const isHighlighted = $derived(msg.event_type === "highlighted");
  /** /me сообщение (ACTION) */
  const isAction = $derived(msg.event_type === "action");

  /** Цвет ника: используем заданный или генерируем по алгоритму Twitch */
  const nickColor = $derived(msg.color || twitchDefaultColor(msg.username));

  /**
   * Разбивает текст сообщения на сегменты: текст и эмоуты.
   * Twitch передаёт позиции эмоутов как индексы символов (code points).
   * Используем [...message] для корректной работы с Unicode (кириллица, emoji).
   */
  function buildSegments(message: string, emotes: EmoteRef[]): MessageSegment[] {
    if (!emotes || emotes.length === 0) {
      return [{ type: "text", content: message }];
    }

    const chars = [...message]; // Array of code points (handles surrogates)
    const segments: MessageSegment[] = [];
    let lastIndex = 0;

    // Эмоуты уже отсортированы по start из Rust
    for (const emote of emotes) {
      // Текст перед эмоутом
      if (emote.start > lastIndex) {
        const text = chars.slice(lastIndex, emote.start).join("");
        if (text) segments.push({ type: "text", content: text });
      }

      // Сам эмоут
      segments.push({
        type: "emote",
        content: emote.code,
        url: emote.url,
      });

      lastIndex = emote.end + 1; // end — включительно
    }

    // Оставшийся текст после последнего эмоута
    if (lastIndex < chars.length) {
      const text = chars.slice(lastIndex).join("");
      if (text) segments.push({ type: "text", content: text });
    }

    return segments;
  }

  /** Сегменты текущего сообщения (текст + эмоуты) */
  const segments = $derived(buildSegments(msg.message, msg.emotes));

  /**
   * 15 цветов по умолчанию, которые Twitch назначает пользователям без заданного цвета.
   * Это официальный набор — те же цвета, что видны на twitch.tv.
   */
  const TWITCH_COLORS = [
    "#FF0000", "#0000FF", "#00FF00", "#B22222", "#FF7F50",
    "#9ACD32", "#FF4500", "#2E8B57", "#DAA520", "#D2691E",
    "#5F9EA0", "#1E90FF", "#FF69B4", "#8A2BE2", "#00FF7F",
  ];

  /**
   * Генерирует цвет ника по алгоритму Twitch.
   * Twitch использует сумму char-кодов username % 15 для выбора из палитры.
   */
  function twitchDefaultColor(username: string): string {
    let sum = 0;
    for (let i = 0; i < username.length; i++) {
      sum += username.charCodeAt(i);
    }
    return TWITCH_COLORS[sum % TWITCH_COLORS.length];
  }

  /**
   * Emoji-иконки для основных бейджей Twitch.
   * Фоллбэк — пока не загружены реальные бейджи через API.
   */
  const badgeEmojis: Record<string, string> = {
    broadcaster: "📺",
    moderator: "⚔️",
    vip: "💎",
    subscriber: "⭐",
    premium: "👑",
    "bits-leader": "💰",
    "sub-gifter": "🎁",
    turbo: "⚡",
    partner: "✔️",
    staff: "🔧",
  };

  /**
   * Форматирует timestamp в строку времени HH:MM.
   */
  function formatTime(ts: number): string {
    const date = new Date(ts);
    return date.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
  }
</script>

{#if !isSystemEvent || showSystemEvents}
<div class="chat-message" class:system-event={isSystemEvent} class:highlighted-msg={isHighlighted}>
  <!-- Баннер системного события (sub, raid, gift) -->
  {#if isSystemEvent && msg.system_message}
    <div class="system-banner">
      {msg.system_message}
    </div>
  {/if}

  <!-- Ответ на сообщение (reply / цитата) -->
  {#if msg.reply_to && msg.reply_text}
    <div class="reply-quote">
      <span class="reply-arrow">↩</span>
      <span class="reply-author">{msg.reply_to}</span>:
      <span class="reply-body">{msg.reply_text}</span>
    </div>
  {/if}

  <!-- Строка сообщения: показываем если есть пользовательский текст или это не системное событие -->
  {#if msg.message || !isSystemEvent}
  <div class="message-line">
    <!-- Время сообщения (опционально) -->
    {#if showTimestamp}
      <span class="timestamp">{formatTime(msg.timestamp)}</span>
    {/if}

    <!-- Иконка платформы (опционально) -->
    {#if showPlatformIcon}
      {#if msg.platform === "twitch"}
        <img class="platform-icon-img" src={twitchIcon} alt="TTV" />
      {:else if msg.platform === "kick"}
        <img class="platform-icon-img" src={kickIcon} alt="KICK" />
      {/if}
    {/if}

    <!-- Бейджи пользователя (Twitch: API badge map / Kick: локальные SVG / emoji-фоллбэк) -->
    {#if showBadges}
      {#each msg.badges as badge}
        {@const badgeUrl = $badgeMap[`${badge.id}/${badge.version}`]
          || (msg.platform === "kick" ? kickBadgeUrls[badge.id] : "")
          || badge.image_url}
        {#if badgeUrl}
          <img
            class="badge-img"
            src={badgeUrl}
            alt={badge.title}
            title={badge.title}
            loading="lazy"
          />
        {:else}
          <span class="badge" title={badge.title}>
            {badgeEmojis[badge.id] || "🏷️"}
          </span>
        {/if}
      {/each}
    {/if}

    <!-- Ник пользователя -->
    <span class="username" style="color: {nickColor}">
      {msg.display_name}
    </span>

    {#if isAction}
      <span class="separator"> </span>
    {:else}
      <span class="separator">: </span>
    {/if}

    <!-- Текст сообщения: удалённое или обычное -->
    {#if msg.deleted}
      <span class="message-text deleted">&lt;сообщение удалено&gt;</span>
    {:else}
      <span class="message-text" class:action-text={isAction}>
        {#each segments as segment}
          {#if segment.type === "emote"}
            <img
              class="emote-img"
              src={segment.url}
              alt={segment.content}
              title={segment.content}
              loading="lazy"
            />
          {:else}
            {segment.content}
          {/if}
        {/each}
      </span>
    {/if}
  </div>
  {/if}
</div>
{/if}

<style>
  .chat-message {
    padding: 2px var(--chat-padding, 8px);
    line-height: 1.4;
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .chat-message:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .message-line {
    display: inline;
  }

  /* --- Ответ / цитата --- */
  .reply-quote {
    font-size: 0.8em;
    color: var(--text-muted, #888);
    padding: 2px 8px 2px 12px;
    margin-bottom: 1px;
    border-left: 2px solid rgba(255, 255, 255, 0.15);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .reply-arrow {
    margin-right: 4px;
    opacity: 0.5;
  }

  .reply-author {
    font-weight: 600;
    color: var(--text-color, #e0e0e0);
  }

  .reply-body {
    opacity: 0.7;
  }

  /* --- Время --- */
  .timestamp {
    font-size: 0.75em;
    color: var(--text-muted, #888);
    margin-right: 4px;
    opacity: 0.6;
  }

  /* --- Иконка платформы --- */
  .platform-icon-img {
    display: inline-block;
    width: 1.1em;
    height: 1.1em;
    vertical-align: middle;
    margin-right: 3px;
    border-radius: 2px;
  }

  /* --- Бейджи --- */
  .badge {
    vertical-align: middle;
    margin-right: 2px;
    font-size: 0.85em;
    cursor: help;
  }

  .badge-img {
    display: inline-block;
    height: 1.1em;
    width: auto;
    vertical-align: middle;
    margin-right: 2px;
    cursor: help;
    object-fit: contain;
  }

  /* --- Ник --- */
  .username {
    font-weight: 700;
    cursor: pointer;
  }

  .separator {
    color: var(--text-muted, #888);
  }

  /* --- Текст сообщения --- */
  .message-text {
    color: var(--text-color, #e0e0e0);
    word-break: break-word;
  }

  .message-text.deleted {
    color: var(--text-muted, #888);
    font-style: italic;
    opacity: 0.5;
  }

  /* --- /me (ACTION) сообщение — курсив, цвет ника --- */
  .message-text.action-text {
    font-style: italic;
  }

  /* --- Эмоуты (картинки) --- */
  .emote-img {
    display: inline-block;
    height: 1.8em;
    width: auto;
    vertical-align: middle;
    margin: -2px 1px;
    object-fit: contain;
  }

  /* --- Системное событие (sub, raid, gift) --- */
  .system-event {
    border-left: 3px solid #9146ff;
    background: rgba(145, 70, 255, 0.08);
    padding-left: calc(var(--chat-padding, 8px) - 3px);
  }

  .system-banner {
    font-size: 0.85em;
    color: #b4a0d4;
    padding: 2px 0;
    font-weight: 500;
  }

  /* --- Выделенное сообщение (highlighted) --- */
  .highlighted-msg {
    border-left: 3px solid #755ebc;
    background: rgba(117, 94, 188, 0.12);
    padding-left: calc(var(--chat-padding, 8px) - 3px);
  }
</style>

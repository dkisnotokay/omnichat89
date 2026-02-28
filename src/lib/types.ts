/**
 * TypeScript типы для Omnichat.
 * Зеркало Rust структур из chat/message.rs — должны совпадать.
 */

/** Платформа чата */
export type Platform = "twitch" | "kick";

/** Бейдж (иконка) пользователя */
export interface Badge {
  id: string;
  version: string;
  image_url: string;
  title: string;
}

/** Ссылка на эмоут в тексте */
export interface EmoteRef {
  id: string;
  code: string;
  url: string;
  start: number;
  end: number;
}

/** Сообщение чата (единый формат для всех платформ) */
export interface ChatMessage {
  id: string;
  platform: Platform;
  username: string;
  display_name: string;
  color: string | null;
  badges: Badge[];
  message: string;
  emotes: EmoteRef[];
  timestamp: number;
  channel: string;
  /** Имя автора сообщения, на которое отвечают (reply) */
  reply_to: string | null;
  /** Текст сообщения, на которое отвечают */
  reply_text: string | null;
  /** Тип системного события (null для обычных сообщений) */
  event_type: string | null;
  /** Системный текст события */
  system_message: string | null;
  /** Сообщение удалено модератором */
  deleted?: boolean;
}

/** Настройки TTS озвучки (зеркало Rust TtsSettings) */
export interface TtsSettings {
  enabled: boolean;
  voice: string;
  rate: number;
  volume: number;
  max_queue_size: number;
  pause_ms: number;

  read_all: boolean;
  read_replies: boolean;
  read_highlighted: boolean;
  read_subscribers: boolean;
  read_vip: boolean;
  read_moderators: boolean;

  read_usernames: boolean;
  read_links: boolean;
  read_emotes: boolean;
  max_message_length: number;

  use_keywords: boolean;
  keywords: string[];
  strip_keywords: boolean;

  ignore_symbols: string[];
  word_filter: string[];
  blacklist: string[];
  whitelist: string[];
}

/** Статус TTS (от backend) */
export interface TtsStatus {
  is_speaking: boolean;
  queue_size: number;
}

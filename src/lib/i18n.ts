/**
 * Простая система локализации для Omnichat.
 *
 * Поддерживает русский и английский языки.
 * Используется через getStrings(lang) — возвращает объект со всеми строками.
 */

export type Lang = "ru" | "en";

export interface I18nStrings {
  // Title bar
  settings: string;
  minimize: string;
  close: string;

  // Settings sections
  langSection: string;
  appearance: string;
  display: string;
  window: string;
  ttsSection: string;
  ttsWhat: string;
  ttsKeywords: string;
  ttsFilters: string;

  // Appearance
  fontSize: string;
  appBgOpacity: string;
  bgOpacity: string;
  textColor: string;
  bgColor: string;

  // Display
  showTimestamp: string;
  showBadges: string;
  showBadgesHint: string;
  platformIcon: string;
  showSystemEvents: string;
  showSystemEventsHint: string;
  readHighlighted: string;

  // Window
  alwaysOnTop: string;
  alwaysOnTopHint: string;

  // TTS main
  enableTts: string;
  voice: string;
  speed: string;
  volume: string;
  messageQueue: string;
  pauseBetween: string;
  msUnit: string;
  textLengthLimit: string;
  random: string;

  // TTS what to read
  readAll: string;
  readReplies: string;
  readSubscribers: string;
  readVip: string;
  readModerators: string;
  readUsernames: string;
  readLinks: string;
  readEmotes: string;

  // TTS keywords
  useKeywords: string;
  keywordsLabel: string;
  stripKeywords: string;

  // TTS filters
  ignoreSymbols: string;
  wordFilter: string;
  blacklist: string;
  whitelist: string;

  // OBS Overlay
  obsOverlay: string;
  overlayPort: string;
  overlayUrl: string;
  copyUrl: string;
  obsHint: string;
  obsDimensions: string;

  // Footer
  resetSettings: string;

  // App.svelte — main window
  disconnecting: string;
  authorizing: string;
  connecting: string;
  connectingTo: string;
  msgCount: string;
  logout: string;
  loginTwitch: string;
  ttsQueue: string;
  ttsSkip: string;
  ttsClear: string;

  // Placeholders
  twitchPlaceholder: string;
  kickPlaceholder: string;

  // ChatView
  noMessages: string;
  scrollDown: string;

  // Context menu
  ctxCut: string;
  ctxCopy: string;
  ctxPaste: string;
  ctxSelectAll: string;
}

const ru: I18nStrings = {
  settings: "Настройки",
  minimize: "Свернуть",
  close: "Закрыть",

  langSection: "Язык / Language",
  appearance: "Внешний вид",
  display: "Отображение",
  window: "Окно",
  ttsSection: "TTS Озвучка",
  ttsWhat: "Что озвучивать",
  ttsKeywords: "Ключевые слова для TTS",
  ttsFilters: "Фильтры для TTS",

  fontSize: "Размер шрифта",
  appBgOpacity: "Непрозрачность фона",
  bgOpacity: "Непрозрачность фона (Overlay)",
  textColor: "Цвет текста",
  bgColor: "Цвет фона",

  showTimestamp: "Показывать время",
  showBadges: "Показывать бейджи",
  showBadgesHint: "Для картинок бейджей нужна авторизация Twitch",
  platformIcon: "Иконка платформы",
  showSystemEvents: "Показывать системные события",
  showSystemEventsHint: "Подписки, рейды, подарки",
  readHighlighted: "Озвучивать выделенные сообщения",

  alwaysOnTop: "Поверх окон",
  alwaysOnTopHint: "Не работает в полноэкранных приложениях",

  enableTts: "Включить TTS",
  voice: "Голос",
  speed: "Скорость",
  volume: "Громкость",
  messageQueue: "Очередь сообщений",
  pauseBetween: "Пауза между сообщениями",
  msUnit: "мс",
  textLengthLimit: "Ограничение длины текста",
  random: "Случайный",

  readAll: "Озвучивать все сообщения",
  readReplies: "Озвучивать сообщения с ответом",
  readSubscribers: "Озвучивать сообщения подписчиков",
  readVip: "Озвучивать сообщения VIP-пользователей",
  readModerators: "Озвучивать сообщения модераторов",
  readUsernames: "Озвучивать имена",
  readLinks: "Озвучивать ссылки",
  readEmotes: "Озвучивать смайлики",

  useKeywords: "Использовать ключевые слова",
  keywordsLabel: "Ключевые слова",
  stripKeywords: "Удалять ключевые слова из сообщений",

  ignoreSymbols: "Игнорирование слов или символов",
  wordFilter: "Фильтр слов и фраз",
  blacklist: "Чёрный список",
  whitelist: "Белый список",

  obsOverlay: "OBS Overlay",
  overlayPort: "Порт",
  overlayUrl: "URL для OBS",
  copyUrl: "Копировать URL",
  obsHint: "Вставьте этот URL в OBS → Sources → Browser",
  obsDimensions: "Рекомендуемый размер: 400 × 600 px",

  resetSettings: "Сбросить настройки",

  disconnecting: "Отключение...",
  authorizing: "Авторизация... откроется браузер",
  connecting: "Подключение...",
  connectingTo: "Подключение к",
  msgCount: "сообщ.",
  logout: "Выйти",
  loginTwitch: "🔗 Войти через Twitch",
  ttsQueue: "В очереди",
  ttsSkip: "⏭ Пропустить",
  ttsClear: "🗑 Очистить",

  twitchPlaceholder: "twitch.tv/channel или имя",
  kickPlaceholder: "kick.com/channel или имя",

  noMessages: "Сообщений пока нет...",
  scrollDown: "⬇ Новые сообщения",

  ctxCut: "Вырезать",
  ctxCopy: "Копировать",
  ctxPaste: "Вставить",
  ctxSelectAll: "Выделить всё",
};

const en: I18nStrings = {
  settings: "Settings",
  minimize: "Minimize",
  close: "Close",

  langSection: "Language",
  appearance: "Appearance",
  display: "Display",
  window: "Window",
  ttsSection: "TTS Speech",
  ttsWhat: "What to read",
  ttsKeywords: "TTS Keywords",
  ttsFilters: "TTS Filters",

  fontSize: "Font size",
  appBgOpacity: "Background opacity",
  bgOpacity: "Background opacity (Overlay)",
  textColor: "Text color",
  bgColor: "Background color",

  showTimestamp: "Show timestamp",
  showBadges: "Show badges",
  showBadgesHint: "Twitch authorization required for badge images",
  platformIcon: "Platform icon",
  showSystemEvents: "Show system events",
  showSystemEventsHint: "Subs, raids, gifts",
  readHighlighted: "Read highlighted messages",

  alwaysOnTop: "Always on top",
  alwaysOnTopHint: "Does not work in fullscreen apps",

  enableTts: "Enable TTS",
  voice: "Voice",
  speed: "Speed",
  volume: "Volume",
  messageQueue: "Message queue",
  pauseBetween: "Pause between messages",
  msUnit: "ms",
  textLengthLimit: "Text length limit",
  random: "Random",

  readAll: "Read all messages",
  readReplies: "Read replies",
  readSubscribers: "Read subscriber messages",
  readVip: "Read VIP messages",
  readModerators: "Read moderator messages",
  readUsernames: "Read usernames",
  readLinks: "Read links",
  readEmotes: "Read emotes",

  useKeywords: "Use keywords",
  keywordsLabel: "Keywords",
  stripKeywords: "Strip keywords from messages",

  ignoreSymbols: "Ignore words or symbols",
  wordFilter: "Word and phrase filter",
  blacklist: "Blacklist",
  whitelist: "Whitelist",

  obsOverlay: "OBS Overlay",
  overlayPort: "Port",
  overlayUrl: "URL for OBS",
  copyUrl: "Copy URL",
  obsHint: "Paste this URL in OBS → Sources → Browser",
  obsDimensions: "Recommended size: 400 × 600 px",

  resetSettings: "Reset settings",

  disconnecting: "Disconnecting...",
  authorizing: "Authorizing... browser will open",
  connecting: "Connecting...",
  connectingTo: "Connecting to",
  msgCount: "msgs",
  logout: "Logout",
  loginTwitch: "🔗 Login with Twitch",
  ttsQueue: "Queue",
  ttsSkip: "⏭ Skip",
  ttsClear: "🗑 Clear",

  twitchPlaceholder: "twitch.tv/channel or name",
  kickPlaceholder: "kick.com/channel or name",

  noMessages: "No messages yet...",
  scrollDown: "⬇ New messages",

  ctxCut: "Cut",
  ctxCopy: "Copy",
  ctxPaste: "Paste",
  ctxSelectAll: "Select all",
};

const strings: Record<Lang, I18nStrings> = { ru, en };

/** Получить все строки для указанного языка */
export function getStrings(lang: Lang): I18nStrings {
  return strings[lang] || strings.ru;
}

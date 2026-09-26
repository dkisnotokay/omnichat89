/**
 * Санитайзеры для данных, приходящих из чата.
 *
 * Сообщения, ники, цвета и ссылки на картинки приходят от зрителей и
 * платформ — то есть это недоверенный ввод. Svelte сам экранирует разметку,
 * поэтому XSS через текст невозможен, но остаются два вектора:
 *   - цвет ника попадает в атрибут style → CSS-инъекция;
 *   - src картинки может указывать на чужой сервер → сторонний трекинг.
 * Эти функции закрывают оба. Те же правила продублированы в overlay/page.html.
 */

/** Хосты, с которых разрешено грузить картинки (CDN платформ). */
const ALLOWED_IMG_HOSTS = new Set([
  "static-cdn.jtvnw.net",
  "files.kick.com",
  "kick.com",
  "media.giphy.com",
  "media0.giphy.com",
  "media1.giphy.com",
  "media2.giphy.com",
  "media3.giphy.com",
  "media4.giphy.com",
  "i.giphy.com",
]);

/**
 * Пропускает только https-ссылки на CDN платформ и наши локальные SVG.
 * Всё остальное → пустая строка (картинка не рисуется).
 */
export function safeImageUrl(url: string | undefined | null): string {
  if (!url) return "";
  const raw = String(url);
  if (raw.startsWith("data:image/svg+xml,")) return raw;
  try {
    const u = new URL(raw);
    if (u.protocol !== "https:") return "";
    if (!ALLOWED_IMG_HOSTS.has(u.hostname)) return "";
    return u.href;
  } catch {
    return "";
  }
}

/**
 * Пропускает только безопасные записи цвета: #hex, rgb(...) и имя цвета.
 * Всё остальное → пустая строка (вызывающий подставит цвет по умолчанию).
 */
export function safeColor(color: string | undefined | null): string {
  if (!color) return "";
  const c = String(color).trim();
  if (/^#[0-9a-fA-F]{3,8}$/.test(c)) return c;
  if (/^rgb\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*\)$/.test(c)) return c;
  if (/^[a-zA-Z]{1,20}$/.test(c)) return c;
  return "";
}

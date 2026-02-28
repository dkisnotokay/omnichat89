/**
 * Маппинг Kick badge type → inline SVG data URI.
 * Kick не передаёт image_url в WebSocket сообщениях,
 * поэтому используем локальные SVG-иконки.
 */

function svg(bg: string, path: string, fill = "white"): string {
  return `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 18 18">` +
    `<rect width="18" height="18" rx="3" fill="${bg}"/>` +
    `<path d="${path}" fill="${fill}"/>` +
    `</svg>`
  )}`;
}

/** Kick badge type → image URL (data URI) */
export const kickBadgeUrls: Record<string, string> = {
  // Broadcaster — иконка камеры
  broadcaster: svg("#e74c3c",
    "M4 6.5a1 1 0 011-1h4.5a1 1 0 011 1v5a1 1 0 01-1 1H5a1 1 0 01-1-1v-5zm8 .7l2.5-1.7v7l-2.5-1.7V7.2z"),
  // Moderator — меч
  moderator: svg("#2ecc71",
    "M9 3l1.2 1.2L13 7l-1 1-1.5-1.5-1 4.5H8.5l-1-4.5L6 8 5 7l2.8-2.8L9 3zm-2 9h4v1.5a2 2 0 01-4 0V12z"),
  // VIP — бриллиант
  vip: svg("#9b59b6",
    "M9 4l3.5 3.5L9 14 5.5 7.5 9 4zm0 2.2L7.2 7.8 9 12l1.8-4.2L9 6.2z"),
  // OG — звезда
  og: svg("#f39c12",
    "M9 3.5l1.8 3.6 4 .6-2.9 2.8.7 4L9 12.6l-3.6 1.9.7-4-2.9-2.8 4-.6L9 3.5z"),
  // Founder — лента
  founder: svg("#3498db",
    "M6 4h6v7.5L9 9.5 6 11.5V4z"),
  // Subscriber — сердце
  subscriber: svg("#1abc9c",
    "M9 13.5l-4.2-4.2a2.5 2.5 0 013.5-3.5L9 6.5l.7-.7a2.5 2.5 0 013.5 3.5L9 13.5z"),
  // Verified — галочка
  verified: svg("#3498db",
    "M7.5 11.5l-2.5-2.5 1.2-1.2 1.3 1.3 3.8-3.8L12.5 6.5 7.5 11.5z"),
  // Staff — гаечный ключ
  staff: svg("#e67e22",
    "M11.5 4.5a3 3 0 00-2.8 2L6 9.2 4.5 7.7 3.3 8.9l2.8 2.8 1.2-1.2.5.5-1.2 1.2 1.2 1.2 1.2-1.2.5.5-1.2 1.2L9.5 15l1.2-1.2L8 11.2l2.8-2.8a3 3 0 10.7-3.9z"),
  // Sub gifter — подарок
  sub_gifter: svg("#e84393",
    "M5 8h8v5.5a1 1 0 01-1 1H6a1 1 0 01-1-1V8zm1-2.5C6 4.7 6.7 4 7.5 4S9 4.7 9 5.5V7H6V5.5zM9 5.5V7h3V5.5C12 4.7 11.3 4 10.5 4S9 4.7 9 5.5z"),
};

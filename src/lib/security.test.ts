/**
 * Тесты санитайзеров чата. Запуск: npm run test:security
 * Без тест-раннера — Node умеет исполнять TypeScript напрямую.
 */
import { strict as assert } from "node:assert";
import { safeColor, safeImageUrl } from "./security.ts";

let passed = 0;
function check(name: string, fn: () => void) {
  fn();
  passed++;
  console.log("  ok —", name);
}

console.log("safeColor:");

check("пропускает hex", () => {
  assert.equal(safeColor("#00FF00"), "#00FF00");
  assert.equal(safeColor("#fff"), "#fff");
});

check("пропускает rgb() и имя цвета", () => {
  assert.equal(safeColor("rgb(10, 20, 30)"), "rgb(10, 20, 30)");
  assert.equal(safeColor("red"), "red");
});

check("отбрасывает значение, ломающее атрибут", () => {
  // кавычка позволила бы закрыть style и дописать свой атрибут
  assert.equal(safeColor('blue" data-x="y'), "");
  assert.equal(safeColor("red;background:url(http://tracker.example/p.png)"), "");
});

check("отбрасывает пустое и мусор", () => {
  assert.equal(safeColor(""), "");
  assert.equal(safeColor(null), "");
  assert.equal(safeColor(undefined), "");
});

console.log("safeImageUrl:");

check("пропускает CDN платформ", () => {
  const twitch = "https://static-cdn.jtvnw.net/emoticons/v2/25/default/dark/2.0";
  const kick = "https://files.kick.com/emotes/37226/fullsize";
  const giphy = "https://media4.giphy.com/media/abc/giphy.gif";
  assert.equal(safeImageUrl(twitch), twitch);
  assert.equal(safeImageUrl(kick), kick);
  assert.equal(safeImageUrl(giphy), giphy);
});

check("пропускает наши локальные SVG-бейджи", () => {
  const svg = "data:image/svg+xml,%3Csvg%3E%3C/svg%3E";
  assert.equal(safeImageUrl(svg), svg);
});

check("отбрасывает чужой хост", () => {
  assert.equal(safeImageUrl("https://not-a-cdn.example.com/x.gif"), "");
});

check("отбрасывает не-https схемы", () => {
  assert.equal(safeImageUrl("http://static-cdn.jtvnw.net/x.png"), "");
  assert.equal(safeImageUrl("file:///C:/Windows/System32/config"), "");
});

check("отбрасывает мусор и пустое", () => {
  assert.equal(safeImageUrl("не ссылка"), "");
  assert.equal(safeImageUrl(""), "");
  assert.equal(safeImageUrl(null), "");
});

console.log(`\nвсе ${passed} проверок пройдены`);

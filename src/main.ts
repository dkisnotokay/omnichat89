/**
 * Точка входа приложения Omnichat.
 * Монтирует корневой Svelte компонент в DOM.
 */
import App from "./App.svelte";
import { mount } from "svelte";

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;

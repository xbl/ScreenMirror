import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import { i18n, setLocale } from './i18n';
import { invoke } from '@tauri-apps/api/core';

async function bootstrap() {
  const app = createApp(App);
  app.use(createPinia());
  app.use(i18n);
  try {
    const lang = await invoke<string>('get_app_language');
    await setLocale(lang);
  } catch {
    // not running inside Tauri (e.g., vite dev) — keep default 'en'
  }
  app.mount('#app');
}

void bootstrap();
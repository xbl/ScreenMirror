import { createI18n } from 'vue-i18n';
import en from './en';
import zhCN from './zh-CN';

export const i18n = createI18n({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, 'zh-CN': zhCN },
});

export async function setLocale(lang: string): Promise<void> {
  if (lang === 'en' || lang === 'zh-CN') {
    i18n.global.locale.value = lang;
  }
}
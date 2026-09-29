/**
 * Languages: which one, installing it, and reading words outside components.
 *
 * `initI18n` must run during the root component's initialisation, before any
 * child reads a dictionary. Components read words with `useIntlayer(key)` and
 * `$c.word`; code that is not a component (actions, toasts) reads the current
 * language's words with `words(key)`.
 *
 * The choice lives in one `localStorage` key: this is a desktop app with no
 * server and no routing. A first run follows the OS language when it is one
 * we ship, English otherwise.
 */
import { getIntlayer, setupIntlayer } from "svelte-intlayer";
import type { DictionaryKeys } from "@intlayer/types/module_augmentation";

export const LOCALES = ["en", "fr", "de", "es", "ru"] as const;
export type Locale = (typeof LOCALES)[number];

/** Each language named in itself, which is what a picker should show. */
export const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  fr: "Français",
  de: "Deutsch",
  es: "Español",
  ru: "Русский",
};

const KEY = "dzch.locale";
// The key the previous interface (paraglide) stored the choice under.
const LEGACY_KEY = "PARAGLIDE_LOCALE";

const isLocale = (v: string | null | undefined): v is Locale =>
  !!v && (LOCALES as readonly string[]).includes(v);

function initialLocale(): Locale {
  try {
    const saved = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY);
    if (isLocale(saved)) return saved;
  } catch {
    // A disabled store is not worth failing startup over.
  }
  const os = (navigator.language || "en").slice(0, 2).toLowerCase();
  return isLocale(os) ? os : "en";
}

let current: Locale = initialLocale();
let setter: ((l: Locale) => void) | null = null;

/** Install the locale context. Call once, at the root. */
export function initI18n() {
  const ctx = setupIntlayer(current);
  setter = (l) => ctx.setLocale(l);
  document.documentElement.lang = current;
  return ctx;
}

export function getLocale(): Locale {
  return current;
}

/** Switch language, and remember it across launches. */
export function setLocale(locale: Locale) {
  current = locale;
  setter?.(locale);
  document.documentElement.lang = locale;
  try {
    localStorage.setItem(KEY, locale);
  } catch {
    // The language still changes for this session.
  }
}

/** A dictionary in the current language, for code outside components. */
export function words<K extends DictionaryKeys>(key: K) {
  return getIntlayer(key, current);
}

/** `one` or `other` by count. The dictionaries carry both forms as two keys. */
export function plural<T>(count: number, one: T, other: T): T {
  return count === 1 ? one : other;
}

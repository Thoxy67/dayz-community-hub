/**
 * Languages: which one, installing it, and reading words outside components.
 *
 * `initI18n` must run during the root component's initialisation. Components
 * read words with `dict(key)` and `$c.word`; code that is not a component
 * (actions, toasts) reads the current language's words with `words(key)`.
 *
 * `dict` and not svelte-intlayer's `useIntlayer`: that one builds a new store
 * per component and transforms the whole dictionary again for each. A server
 * row holds eight components reading the same dictionary and rows mount as
 * the list scrolls, so that was hundreds of full dictionary transforms per
 * frame, enough to freeze the window. Here each dictionary is one shared
 * store, transformed once per language.
 *
 * The choice lives in one `localStorage` key: this is a desktop app with no
 * server and no routing. A first run follows the OS language when it is one
 * we ship, English otherwise.
 */
import { derived, type Readable } from "svelte/store";
import { getIntlayer, intlayerStore, setupIntlayer, type useIntlayer } from "svelte-intlayer";
import type { DictionaryKeys } from "@intlayer/types/module_augmentation";

import { pickLocale, type Locale } from "./locale";

export { LOCALES, isLocale, pickLocale, plural, type Locale } from "./locale";

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

function initialLocale(): Locale {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY);
  } catch {
    // A disabled store is not worth failing startup over.
  }
  return pickLocale(saved, navigator.language);
}

let current: Locale = initialLocale();
let setter: ((l: Locale) => void) | null = null;

/** Install the locale context. Call once, at the root. */
export function initI18n() {
  const ctx = setupIntlayer(current);
  intlayerStore.setLocale(current);
  setter = (l) => {
    ctx.setLocale(l);
    intlayerStore.setLocale(l);
  };
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

const shared = new Map<string, Readable<unknown>>();

/** A dictionary as a store that follows the language: one per key, shared by every component. */
export function dict<K extends DictionaryKeys>(key: K): ReturnType<typeof useIntlayer<K>> {
  let store = shared.get(key);
  if (!store) {
    store = derived(intlayerStore, ($s) => getIntlayer(key, $s.locale));
    shared.set(key, store);
  }
  return store as ReturnType<typeof useIntlayer<K>>;
}

/** A dictionary in the current language, for code outside components. */
export function words<K extends DictionaryKeys>(key: K) {
  return getIntlayer(key, current);
}


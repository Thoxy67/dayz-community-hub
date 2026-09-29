/** Which language to start in. Pure, so it is tested on its own. */
export const LOCALES = ["en", "fr", "de", "es", "ru"] as const;
export type Locale = (typeof LOCALES)[number];

export const isLocale = (v: string | null | undefined): v is Locale =>
  !!v && (LOCALES as readonly string[]).includes(v);

/**
 * The saved choice when there is one, else the OS language when it is one
 * we ship ("fr-CA" counts as French), else English.
 */
export function pickLocale(saved: string | null | undefined, osLanguage: string | null | undefined): Locale {
  if (isLocale(saved)) return saved;
  const os = (osLanguage || "en").slice(0, 2).toLowerCase();
  return isLocale(os) ? os : "en";
}

/** `one` or `other` by count. The dictionaries carry both forms as two keys. */
export function plural<T>(count: number, one: T, other: T): T {
  return count === 1 ? one : other;
}

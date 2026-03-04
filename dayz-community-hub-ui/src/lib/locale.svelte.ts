import { getLocale, setLocale as paraglidSetLocale, type Locale } from '$lib/paraglide/runtime.js';

/**
 * Reactive locale state for Svelte 5.
 * This wrapper ensures UI components re-render when locale changes.
 *
 * We use a simple object with $state rather than a class to ensure
 * the {#key} block properly detects changes.
 */
function createLocaleState() {
  let locale = $state<Locale>(getLocale());

  return {
    get locale() {
      return locale;
    },
    setLocale(newLocale: Locale) {
      // Update Paraglide's internal state FIRST
      paraglidSetLocale(newLocale, { reload: false });
      // Then update reactive state to trigger re-renders
      locale = newLocale;
    },
  };
}

export const localeState = createLocaleState();

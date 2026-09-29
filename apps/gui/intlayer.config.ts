import { Locales, type IntlayerConfig } from "intlayer";

// The five shipped languages. English is the source the others are written
// against and the fallback for anything not translated yet. Dictionaries are
// the `*.content.ts` files in `src/content/`, bundled at build time: nothing
// is fetched at run time.
const config: IntlayerConfig = {
  internationalization: {
    locales: [Locales.ENGLISH, Locales.FRENCH, Locales.GERMAN, Locales.SPANISH, Locales.RUSSIAN],
    defaultLocale: Locales.ENGLISH,
  },
  routing: { mode: "no-prefix" },
  log: { mode: "disabled" },
  build: {
    chunkGrouping: false,
    // Code outside components reads words through `words(key)` (toasts,
    // actions): the build's per-call optimisation cannot see those keys and
    // would prune the dictionaries or rewrite the calls. Everything is
    // bundled whole instead; the five languages are a few hundred KB.
    optimize: false,
  },
};

export default config;

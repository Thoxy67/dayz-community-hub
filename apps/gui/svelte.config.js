import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  compilerOptions: { runes: true },
  // Runes in our own code only: third-party components (svelte-intlayer ships
  // a few written with `export let`) keep Svelte's per-file detection.
  vitePlugin: {
    /** @param {{ filename: string; compileOptions: Record<string, unknown> }} data */
    dynamicCompileOptions({ filename, compileOptions }) {
      if (filename.includes("/node_modules/")) return { ...compileOptions, runes: undefined };
      return compileOptions;
    },
  },
};

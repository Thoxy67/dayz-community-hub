import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import Icons from "unplugin-icons/vite";
import { intlayer } from "vite-intlayer";
import { fileURLToPath } from "node:url";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const host = process.env.TAURI_DEV_HOST;
const dir = (p: string) => fileURLToPath(new URL(p, import.meta.url));

const WORDS = "virtual:dzch-words/";

/**
 * `virtual:dzch-words/<locale>`: every dictionary in one language, as
 * `{ [key]: dictionary }`. intlayer's own entry bundles the five languages of
 * every dictionary in one chunk (~470 kB) that the first store import parsed;
 * its per-locale loading needs the build optimiser, which cannot see the keys
 * `words(key)` reads. Here each language is its own chunk (~90 kB), loaded
 * by `$lib/i18n` before the app mounts or a language is switched to.
 * Translations are resolved the way intlayer resolves them at run time (a
 * missing word falls back to English); insertions are left for the
 * interpreter.
 */
function words(): Plugin {
  const src = dir("./.intlayer/dictionary");
  const content = dir("./src/content");
  return {
    name: "dzch-words",
    resolveId: (id) => (id.startsWith(WORDS) ? "\0" + id : undefined),
    async load(id) {
      if (!id.startsWith("\0" + WORDS)) return;
      const locale = id.slice(WORDS.length + 1);
      const { getPerLocaleDictionary } = await import("@intlayer/core/plugins");
      // intlayer rebuilds its folder from scratch (`clean`) whenever a build
      // or a check starts, here or in another process: wait until it holds
      // one readable dictionary per content file rather than ship none.
      const expected = readdirSync(content).filter((f) => f.endsWith(".content.ts")).length;
      let dicts: { key: string }[] = [];
      for (let i = 0; i < 100; i++) {
        if (i) await new Promise((r) => setTimeout(r, 100));
        try {
          const files = readdirSync(src).filter((f) => f.endsWith(".json"));
          dicts = files.map((f) => JSON.parse(readFileSync(join(src, f), "utf8")));
          if (dicts.length >= expected) break;
        } catch {
          // Being written.
        }
      }
      if (dicts.length === 0) this.error(`no dictionaries in ${src}`);
      const out: Record<string, unknown> = {};
      for (const d of dicts) {
        const content = getPerLocaleDictionary(d as never, locale, "en").content;
        out[d.key] = { key: d.key, locale, content };
      }
      for (const f of readdirSync(src)) this.addWatchFile(join(src, f));
      // A JSON string parses faster than the same object as a JS literal.
      return `export default JSON.parse(${JSON.stringify(JSON.stringify(out))});`;
    },
    configureServer(server) {
      // A dictionary rebuilt by intlayer reloads the window with its words.
      server.watcher.on("change", (path) => {
        if (!path.startsWith(src)) return;
        for (const m of server.moduleGraph.idToModuleMap.values())
          if (m.id?.startsWith("\0" + WORDS)) server.moduleGraph.invalidateModule(m);
        server.ws.send({ type: "full-reload" });
      });
    },
  };
}

/**
 * The circle-flags set, cut to the two-letter country codes `ui/flag` draws
 * (316 of 696 icons): its lazily loaded chunk drops from ~500 kB to ~200 kB.
 * Region, language and alias entries are never looked up.
 */
function countryFlags(): Plugin {
  const ID = "@iconify-json/circle-flags/icons.json";
  return {
    name: "dzch-country-flags",
    enforce: "pre",
    resolveId: (id) => (id === ID ? "\0country-flags" : undefined),
    load(id) {
      if (id !== "\0country-flags") return;
      const path = fileURLToPath(import.meta.resolve(ID));
      this.addWatchFile(path);
      const set = JSON.parse(readFileSync(path, "utf8"));
      const icons = Object.fromEntries(
        Object.entries(set.icons).filter(([k]) => /^[a-z]{2}$/.test(k)),
      );
      const out = { width: set.width, height: set.height, icons };
      return `export default JSON.parse(${JSON.stringify(JSON.stringify(out))});`;
    },
  };
}

export default defineConfig({
  plugins: [
    // First, so the dictionaries are built before the modules that read them.
    intlayer(),
    words(),
    countryFlags(),
    svelte(),
    tailwindcss(),
    // Icons are compiled into the bundle as Svelte components, never fetched:
    // the app works offline and loads nothing from a CDN.
    Icons({ compiler: "svelte", scale: 1 }),
  ],
  resolve: {
    alias: {
      $lib: dir("./src/lib"),
      $features: dir("./src/features"),
      $shell: dir("./src/shell"),
      $content: dir("./src/content"),
    },
  },
  // Tauri drives these: devUrl in tauri.conf.json points at the fixed port.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "esnext",
    // The window loads its scripts from the binary it ships in, so chunk size
    // costs a parse, not a download. Kept so a chunk that doubles gets said.
    chunkSizeWarningLimit: 1000,
  },
});

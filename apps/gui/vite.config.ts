import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import Icons from "unplugin-icons/vite";
import { intlayer } from "vite-intlayer";
import { fileURLToPath } from "node:url";

const host = process.env.TAURI_DEV_HOST;
const dir = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  plugins: [
    // First, so the dictionaries are built before the modules that read them.
    intlayer(),
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

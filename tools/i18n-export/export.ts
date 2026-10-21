#!/usr/bin/env bun
/**
 * Export the Svelte interface's words (apps/gui/src/content/*.content.ts,
 * intlayer dictionaries) into apps/desktop/i18n/<lang>.json, the egui
 * window's words: `{ "<dictionary>": { "<word>": "text" } }`.
 *
 * Re-runnable: `bun tools/i18n-export/export.ts` (or `--check` to fail when
 * the JSON is stale). The Svelte content is the source until M4.2; after
 * that the JSON is edited by hand and this script retires.
 *
 * `intlayer` is replaced by a shim so the export needs no node_modules:
 * `t({en,fr,…})` is a translation, `insert(x)` marks x as having inserts.
 * Insert placeholders `{{ name }}` become `{name}`. Nested objects are
 * flattened with dots (`a.b`). A missing translation is left out of that
 * language (the window falls back to English), never filled with English.
 */
import { plugin } from "bun";
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const LOCALES = ["en", "fr", "de", "es", "ru"] as const;
type Locale = (typeof LOCALES)[number];

plugin({
  name: "intlayer-shim",
  setup(build) {
    build.module("intlayer", () => ({
      loader: "object",
      exports: {
        t: (translation: Record<string, unknown>) => ({ __t: translation }),
        insert: (inner: unknown) => inner,
      },
    }));
  },
});

const root = resolve(import.meta.dir, "../..");
const contentDir = join(root, "apps/gui/src/content");
const outDir = join(root, "apps/desktop/i18n");
const check = process.argv.includes("--check");

const placeholders = (s: string) => s.replace(/\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g, "{$1}");

type Flat = Record<string, string>;
const out: Record<Locale, Record<string, Flat>> = { en: {}, fr: {}, de: {}, es: {}, ru: {} };
const problems: string[] = [];

function walk(dict: string, prefix: string, node: unknown) {
  if (node && typeof node === "object" && "__t" in node) {
    const tr = (node as { __t: Record<string, unknown> }).__t;
    for (const l of LOCALES) {
      const v = tr[l];
      if (typeof v === "string") (out[l][dict] ??= {})[prefix] = placeholders(v);
      else if (v !== undefined) problems.push(`${dict}.${prefix}: ${l} is not a string`);
    }
    return;
  }
  if (typeof node === "string") {
    for (const l of LOCALES) (out[l][dict] ??= {})[prefix] = placeholders(node);
    return;
  }
  if (node && typeof node === "object" && !Array.isArray(node)) {
    for (const [k, v] of Object.entries(node)) walk(dict, prefix ? `${prefix}.${k}` : k, v);
    return;
  }
  problems.push(`${dict}.${prefix}: unsupported value ${JSON.stringify(node)}`);
}

const files = readdirSync(contentDir)
  .filter((f) => f.endsWith(".content.ts"))
  .sort();
for (const f of files) {
  const mod = await import(join(contentDir, f));
  const d = mod.default as { key: string; content: unknown };
  for (const l of LOCALES) out[l][d.key] ??= {};
  walk(d.key, "", d.content);
}

if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}

// Sorted keys: stable diffs whatever the source order.
const sorted = (o: Record<string, Flat>) =>
  Object.fromEntries(
    Object.keys(o)
      .sort()
      .map((k) => [k, Object.fromEntries(Object.keys(o[k]!).sort().map((w) => [w, o[k]![w]]))]),
  );

mkdirSync(outDir, { recursive: true });
let stale = false;
for (const l of LOCALES) {
  const path = join(outDir, `${l}.json`);
  const text = JSON.stringify(sorted(out[l]), null, 2) + "\n";
  let old = "";
  try {
    old = readFileSync(path, "utf8");
  } catch {
    // First export.
  }
  if (old === text) continue;
  stale = true;
  if (!check) writeFileSync(path, text);
}
const words = Object.values(out.en).reduce((n, d) => n + Object.keys(d).length, 0);
if (check && stale) {
  console.error("apps/desktop/i18n is stale: run bun tools/i18n-export/export.ts");
  process.exit(1);
}
console.log(`${files.length} dictionaries, ${words} English words${stale && !check ? ", written" : ", unchanged"}`);

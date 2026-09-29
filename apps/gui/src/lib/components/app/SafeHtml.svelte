<script lang="ts" module>
  /**
   * HTML from elsewhere, made safe to insert: no scripts, styles, frames or
   * forms, no inline handlers, no `javascript:` links.
   */
  export function sanitize(html: string): string {
    const doc = new DOMParser().parseFromString(html, "text/html");
    doc.querySelectorAll("script, style, iframe, object, embed, form, link, meta, base").forEach((n) => n.remove());
    for (const el of doc.body.querySelectorAll("*")) {
      for (const attr of [...el.attributes]) {
        const name = attr.name.toLowerCase();
        const value = attr.value.trim().toLowerCase();
        if (name.startsWith("on") || name === "style" || name === "srcset") el.removeAttribute(attr.name);
        else if ((name === "href" || name === "src") && value.startsWith("javascript:")) el.removeAttribute(attr.name);
      }
    }
    return doc.body.innerHTML;
  }
</script>

<script lang="ts">
  import { openUrl } from "$lib/ipc/native";
  import { cn } from "$lib/cx";

  /**
   * Somebody else's article, in the app's type: sanitised, links opened in
   * the system browser, and pictures passed through `resolveImage` (a remote
   * picture does not load in the app's window; the backend caches it). A
   * click on a picture calls `onimage` with what is shown and the original.
   */
  let {
    html,
    resolveImage,
    onimage,
    base = "https://dayz.com/",
    class: klass = "",
  }: {
    html: string;
    /** What relative links are resolved against: the page the HTML came from. */
    base?: string;
    resolveImage?: (url: string) => Promise<string>;
    onimage?: (shown: string, original: string) => void;
    class?: string;
  } = $props();

  const safe = $derived(sanitize(html));

  function wire(node: HTMLElement) {
    const done = new WeakSet<HTMLImageElement>();
    function rewrite() {
      if (!resolveImage) return;
      for (const img of node.querySelectorAll<HTMLImageElement>("img[src]")) {
        const src = img.getAttribute("src");
        if (!src || done.has(img) || /^(data|blob|asset|https?:\/\/asset)/.test(src)) continue;
        done.add(img);
        img.dataset.original = img.dataset.full ?? src;
        img.removeAttribute("src");
        resolveImage(src)
          .then((local) => (img.src = local))
          .catch(() => (img.style.display = "none"));
      }
    }
    function onclick(e: MouseEvent) {
      const t = e.target as HTMLElement;
      const a = t.closest("a");
      const raw = a?.getAttribute("href");
      if (a && raw) {
        e.preventDefault();
        // `a.href` would resolve a relative link against the window's own
        // address; the backend only opens absolute http(s) links.
        try {
          void openUrl(new URL(raw, base).href);
        } catch {
          // Not a link that can be opened.
        }
        return;
      }
      const img = t.closest("img");
      if (img instanceof HTMLImageElement && img.src) onimage?.(img.src, img.dataset.original ?? img.src);
    }
    rewrite();
    const mo = new MutationObserver(rewrite);
    mo.observe(node, { childList: true, subtree: true });
    node.addEventListener("click", onclick);
    return {
      destroy() {
        mo.disconnect();
        node.removeEventListener("click", onclick);
      },
    };
  }
</script>

{#key safe}
  <div class={cn("safe-html", onimage && "zoomable", klass)} use:wire data-selectable>{@html safe}</div>
{/key}

<style>
  .safe-html {
    font-size: var(--text-sm);
    line-height: 1.7;
    color: var(--color-fg-muted);
  }
  .safe-html :global(:is(h1, h2, h3, h4)) {
    margin: 1.6em 0 0.5em;
    font-family: var(--font-display);
    font-weight: 800;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    line-height: 1.05;
    color: var(--color-fg);
  }
  .safe-html :global(h1) {
    font-size: var(--text-2xl);
  }
  .safe-html :global(h2) {
    font-size: var(--text-xl);
  }
  .safe-html :global(:is(h3, h4)) {
    font-size: var(--text-lg);
  }
  .safe-html :global(p) {
    margin: 0 0 1em;
  }
  .safe-html :global(a) {
    color: var(--color-accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .safe-html :global(:is(strong, b)) {
    color: var(--color-fg);
    font-weight: 600;
  }
  .safe-html :global(:is(ul, ol)) {
    margin: 0 0 1em;
    padding-left: 1.4em;
  }
  .safe-html :global(li) {
    margin: 0.25em 0;
  }
  .safe-html :global(li::marker) {
    color: var(--color-accent);
  }
  .safe-html :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 1.2em 0;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
  }
  .zoomable :global(img) {
    cursor: zoom-in;
  }
  .safe-html :global(blockquote) {
    margin: 1em 0;
    padding: 0.5em 1em;
    border-left: 2px solid var(--color-accent);
    background: color-mix(in oklch, var(--color-raised) 50%, transparent);
    color: var(--color-fg);
  }
  .safe-html :global(code) {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    background: var(--color-plot);
    padding: 0.1em 0.35em;
    border-radius: var(--radius-xs);
  }
  .safe-html :global(hr) {
    border: 0;
    border-top: 1px solid var(--color-border);
    margin: 2em 0;
  }
  .safe-html :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-xs);
    margin: 1em 0;
  }
  .safe-html :global(:is(th, td)) {
    border: 1px solid var(--color-border);
    padding: 0.4em 0.6em;
    text-align: left;
  }
</style>

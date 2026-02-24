<script lang="ts">
  import type { ArticleDto } from '$lib/types';
  import Icon from '@iconify/svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { convertFileSrc } from '@tauri-apps/api/core';

  interface Props {
    articles: ArticleDto[];
    loading: boolean;
    onRefresh: () => void;
    onOpenUrl: (url: string) => void;
  }

  let { articles, loading, onRefresh, onOpenUrl }: Props = $props();

  let selectedIndex = $state(0);
  let selected = $derived(articles[selectedIndex] ?? null);

  // ── Image fetching (disk cache + Tauri asset protocol) ────────────────────
  //
  // fetch_image downloads to ~/.local/share/dayz-community-hub/cache/images/
  // and returns the local file path. convertFileSrc() turns that into an
  // asset:// URL the webview can load directly — no base64 overhead.
  //
  // On first load, resolve_cached_images bulk-resolves every article image
  // that's already on disk in a single synchronous IPC call — no network,
  // no per-image round-trips. Hero & thumbnails for cached images are
  // available instantly on the same tick.

  // In-memory map: remote URL → asset:// URI.
  const imgCache = new Map<string, string>();

  // Concurrency limiter for network fetches.
  const MAX_CONCURRENT = 6;
  let activeCount = 0;
  const fetchQueue: Array<() => void> = [];

  function drainQueue() {
    while (activeCount < MAX_CONCURRENT && fetchQueue.length > 0) {
      activeCount++;
      fetchQueue.shift()!();
    }
  }

  async function fetchImage(url: string): Promise<string> {
    if (imgCache.has(url)) return imgCache.get(url)!;

    await new Promise<void>((resolve) => {
      fetchQueue.push(resolve);
      drainQueue();
    });

    try {
      const localPath = await invoke<string>('fetch_image', { url });
      const assetUrl = convertFileSrc(localPath);
      imgCache.set(url, assetUrl);
      return assetUrl;
    } finally {
      activeCount--;
      drainQueue();
    }
  }

  /** Svelte action: rewrite every <img src> inside the node to an asset:// URI. */
  function rustImages(node: HTMLElement) {
    const rewritten = new WeakSet<HTMLImageElement>();

    function rewrite() {
      node.querySelectorAll<HTMLImageElement>('img[src]').forEach((img) => {
        const src = img.getAttribute('src');
        if (!src || src.startsWith('data:') || src.startsWith('blob:')
            || src.startsWith('asset:') || src.startsWith('http://asset')
            || rewritten.has(img)) return;
        rewritten.add(img);
        img.removeAttribute('src');
        fetchImage(src)
          .then((uri) => { img.src = uri; })
          .catch(() => { img.style.display = 'none'; });
      });
    }
    rewrite();
    const mo = new MutationObserver(rewrite);
    mo.observe(node, { childList: true, subtree: true });
    return { destroy() { mo.disconnect(); } };
  }

  // Thumbnail URIs — $state array so Svelte tracks per-index mutations
  let thumbs = $state<(string | null)[]>([]);
  const thumbFetching = new Set<number>();

  // Bulk-resolve cached images on disk in one synchronous IPC call, then
  // kick off network fetches only for the ones that are missing.
  $effect(() => {
    if (thumbs.length !== articles.length) {
      thumbs = Array(articles.length).fill(null);
      thumbFetching.clear();
    }

    const imageUrls = articles
      .map((a) => a.image_url)
      .filter((u): u is string => !!u);

    if (imageUrls.length === 0) return;

    // 1) Bulk-resolve everything already on disk (single IPC, sync on Rust side).
    invoke<[string, string][]>('resolve_cached_images', { urls: imageUrls })
      .then((cached) => {
        for (const [url, localPath] of cached) {
          const assetUrl = convertFileSrc(localPath);
          imgCache.set(url, assetUrl);
        }
        // Apply resolved thumbnails immediately
        articles.forEach((article, i) => {
          if (!article.image_url) return;
          const hit = imgCache.get(article.image_url);
          if (hit) {
            thumbs[i] = hit;
            thumbFetching.add(i);
          }
        });
        // Force hero to update if the selected article was resolved
        updateHero();
      })
      .catch(() => {})
      .finally(() => {
        // 2) Fetch anything still missing from the network.
        articles.forEach((article, i) => {
          if (thumbFetching.has(i) || !article.image_url) return;
          thumbFetching.add(i);
          fetchImage(article.image_url)
            .then((uri) => { thumbs[i] = uri; })
            .catch(() => {});
        });
      });
  });

  // Hero image for reading pane — synchronous lookup first, async fallback.
  let heroDataUri = $state<string | null>(null);

  function updateHero() {
    const url = selected?.image_url ?? null;
    if (!url) { heroDataUri = null; return; }
    const cached = imgCache.get(url);
    if (cached) { heroDataUri = cached; return; }
    heroDataUri = null;
    fetchImage(url).then((uri) => { heroDataUri = uri; }).catch(() => {});
  }

  $effect(() => {
    // Re-run whenever selected article changes
    void selected;
    updateHero();
  });
</script>

<div class="flex h-full overflow-hidden">

  <!-- ── Sidebar: article list ──────────────────────────────────────────────── -->
  <div class="w-68 flex-shrink-0 flex flex-col border-r border-base-300 bg-base-100 overflow-hidden">

    <!-- Sidebar header -->
    <div class="flex items-center gap-2 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
      <Icon icon="ph:newspaper" class="size-3.5 text-primary" />
      <span class="text-xs font-semibold flex-1">DayZ News</span>
      {#if articles.length > 0}
        <span class="text-xs text-base-content/30">{articles.length}</span>
      {/if}
      <button
        class="btn btn-ghost btn-xs p-1"
        onclick={onRefresh}
        disabled={loading}
        title="Refresh"
      >
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
        {/if}
      </button>
    </div>

    {#if loading && articles.length === 0}
      <!-- Skeleton list while loading -->
      <div class="flex-1 overflow-y-auto p-2 space-y-2">
        {#each [1,2,3,4] as _}
          <div class="rounded-lg bg-base-200 animate-pulse h-20"></div>
        {/each}
      </div>
    {:else}
      <div class="flex-1 overflow-y-auto">
        {#each articles as article, i}
          {@const thumb = thumbs[i] ?? null}
          {@const isSel = i === selectedIndex}
          <button
            class="w-full text-left transition-colors border-b border-base-300/50 relative
                   {isSel
                     ? 'bg-primary/10 border-l-2 border-l-primary'
                     : 'hover:bg-base-200/60 border-l-2 border-l-transparent'}"
            onclick={() => (selectedIndex = i)}
          >
            <div class="flex gap-2.5 p-2.5">
              <!-- Thumbnail -->
              <div class="w-16 h-12 rounded-md overflow-hidden flex-shrink-0 bg-base-300">
                {#if thumb}
                  <img src={thumb} alt="" class="w-full h-full object-cover" />
                {:else if article.image_url}
                  <div class="w-full h-full bg-base-200 animate-pulse"></div>
                {:else}
                  <div class="w-full h-full flex items-center justify-center text-base-content/20">
                    <Icon icon="ph:image" class="size-5" />
                  </div>
                {/if}
              </div>

              <!-- Text -->
              <div class="flex-1 min-w-0 flex flex-col justify-between">
                <p class="text-xs font-medium leading-snug text-base-content/90 line-clamp-2">
                  {article.title}
                </p>
                <div class="flex items-center gap-1.5 mt-1">
                  {#if article.category}
                    <span class="text-primary/70 font-semibold uppercase" style="font-size:9px; letter-spacing:0.05em;">
                      {article.category}
                    </span>
                    <span class="text-base-content/20" style="font-size:9px;">·</span>
                  {/if}
                  <span class="text-base-content/35" style="font-size:10px;">{article.date}</span>
                </div>
              </div>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ── Reading pane ────────────────────────────────────────────────────────── -->
  <div class="flex-1 overflow-hidden flex flex-col bg-base-100">
    {#if selected}

      <!-- Hero image -->
      {#if heroDataUri || selected.image_url}
        <div class="w-full h-48 flex-shrink-0 relative overflow-hidden bg-base-200">
          {#if heroDataUri}
            <img src={heroDataUri} alt="" class="w-full h-full object-cover" />
            <!-- Gradient overlay so title text reads well -->
            <div class="absolute inset-0 bg-gradient-to-t from-base-100 via-base-100/30 to-transparent"></div>
          {:else}
            <div class="w-full h-full animate-pulse bg-base-300"></div>
          {/if}

          <!-- Title overlaid on hero -->
          {#if heroDataUri}
            <div class="absolute bottom-0 left-0 right-0 px-6 pb-4">
              <h1 class="text-lg font-bold leading-tight text-base-content drop-shadow">
                {selected.title}
              </h1>
              <div class="flex items-center gap-2 mt-1">
                {#if selected.category}
                  <span class="text-primary font-semibold uppercase" style="font-size:10px; letter-spacing:0.06em;">
                    {selected.category}
                  </span>
                  <span class="text-base-content/30">·</span>
                {/if}
                {#if selected.author}
                  <span class="text-base-content/60 text-xs">{selected.author}</span>
                  <span class="text-base-content/30">·</span>
                {/if}
                <span class="text-base-content/50 text-xs">{selected.date}</span>
                <button
                  class="ml-auto flex items-center gap-1 text-xs text-base-content/50 hover:text-primary transition-colors"
                  onclick={() => onOpenUrl(selected.url)}
                  title="Open in browser"
                >
                  <Icon icon="ph:arrow-square-out" class="size-3.5" />
                  dayz.com
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- Title bar (when no hero image) -->
      {#if !selected.image_url}
        <div class="px-6 pt-5 pb-3 border-b border-base-300 flex-shrink-0">
          <h1 class="text-lg font-bold text-base-content leading-tight">{selected.title}</h1>
          <div class="flex items-center gap-2 mt-2">
            {#if selected.category}
              <span class="text-primary font-semibold uppercase" style="font-size:10px; letter-spacing:0.06em;">
                {selected.category}
              </span>
              <span class="text-base-content/30">·</span>
            {/if}
            {#if selected.author}
              <span class="text-base-content/60 text-xs">{selected.author}</span>
              <span class="text-base-content/30">·</span>
            {/if}
            <span class="text-base-content/50 text-xs">{selected.date}</span>
            <button
              class="ml-auto flex items-center gap-1 text-xs text-base-content/50 hover:text-primary transition-colors"
              onclick={() => onOpenUrl(selected.url)}
              title="Open in browser"
            >
              <Icon icon="ph:arrow-square-out" class="size-3.5" />
              dayz.com
            </button>
          </div>
        </div>
      {/if}

      <!-- Article body -->
      <div class="flex-1 overflow-y-auto">
        <div class="max-w-2xl mx-auto px-6 py-5">

          {#if selected.excerpt}
            <p class="text-sm text-base-content/60 italic leading-relaxed mb-5 pb-5 border-b border-base-300/60">
              {selected.excerpt}
            </p>
          {/if}

          {#if selected.content_html}
            <div class="article-body" use:rustImages>
              {@html selected.content_html}
            </div>
          {:else}
            <p class="text-sm text-base-content/75 leading-relaxed whitespace-pre-wrap">
              {selected.content_text || 'No content available.'}
            </p>
          {/if}

        </div>
      </div>

    {:else if !loading}
      <div class="flex flex-col items-center justify-center h-full gap-3 text-base-content/30">
        <Icon icon="ph:newspaper" class="size-12 opacity-20" />
        <span class="text-sm">Select an article to read</span>
      </div>
    {/if}
  </div>

</div>

<style>
  .article-body :global(p) {
    margin-bottom: 0.9rem;
    line-height: 1.7;
    font-size: 0.875rem;
    color: oklch(var(--bc) / 0.82);
  }

  .article-body :global(h1),
  .article-body :global(h2),
  .article-body :global(h3),
  .article-body :global(h4) {
    font-weight: 700;
    margin-top: 1.5rem;
    margin-bottom: 0.5rem;
    color: oklch(var(--bc));
    line-height: 1.3;
  }

  .article-body :global(h1) { font-size: 1.2rem; }
  .article-body :global(h2) { font-size: 1.05rem; }
  .article-body :global(h3) { font-size: 0.95rem; letter-spacing: 0.01em; }

  .article-body :global(ul),
  .article-body :global(ol) {
    margin-bottom: 0.9rem;
    padding-left: 1.4rem;
  }

  .article-body :global(li) {
    margin-bottom: 0.3rem;
    font-size: 0.875rem;
    line-height: 1.6;
    color: oklch(var(--bc) / 0.82);
  }

  .article-body :global(ul > li) { list-style-type: disc; }
  .article-body :global(ol > li) { list-style-type: decimal; }

  .article-body :global(strong),
  .article-body :global(b) {
    font-weight: 600;
    color: oklch(var(--bc));
  }

  .article-body :global(a) {
    color: oklch(var(--p));
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .article-body :global(a:hover) {
    color: oklch(var(--pf, var(--p)) / 0.8);
  }

  .article-body :global(img) {
    max-width: 100%;
    border-radius: 8px;
    margin: 1rem 0;
    display: block;
  }

  .article-body :global(blockquote) {
    border-left: 3px solid oklch(var(--p) / 0.5);
    padding: 0.5rem 0 0.5rem 1rem;
    margin: 1rem 0;
    color: oklch(var(--bc) / 0.6);
    font-style: italic;
    background: oklch(var(--b2));
    border-radius: 0 6px 6px 0;
  }

  .article-body :global(hr) {
    border: none;
    border-top: 1px solid oklch(var(--bc) / 0.1);
    margin: 1.25rem 0;
  }

  .article-body :global(code) {
    font-family: monospace;
    font-size: 0.8rem;
    background: oklch(var(--b2));
    padding: 0.1em 0.35em;
    border-radius: 3px;
    color: oklch(var(--p));
  }

  .article-body :global(app-picture),
  .article-body :global(app-imgur) {
    display: none;
  }
</style>

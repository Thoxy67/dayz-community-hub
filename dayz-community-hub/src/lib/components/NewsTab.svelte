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

  /** Svelte action: rewrite every <img src> inside the node to an asset:// URI,
   *  and attach a click handler so clicking any image opens the lightbox. */
  function rustImages(node: HTMLElement) {
    const rewritten = new WeakSet<HTMLImageElement>();

    function rewrite() {
      node.querySelectorAll<HTMLImageElement>('img[src]').forEach((img) => {
        const src = img.getAttribute('src');
        if (
          !src ||
          src.startsWith('data:') ||
          src.startsWith('blob:') ||
          src.startsWith('asset:') ||
          src.startsWith('http://asset') ||
          rewritten.has(img)
        )
          return;
        rewritten.add(img);
        img.removeAttribute('src');
        // Fetch the display version (640 px) for inline rendering
        const fullUrl = img.getAttribute('data-full') ?? src;
        fetchImage(src)
          .then((uri) => {
            img.src = uri;
            img.style.cursor = 'zoom-in';
            img.addEventListener('click', () => {
              if (lightboxSrc) {
                closeLightbox();
                return;
              }
              // Try the full-size version first; fall back to the display URI
              fetchImage(fullUrl)
                .then((fullUri) => openLightbox(fullUri))
                .catch(() => openLightbox(uri));
            });
          })
          .catch(() => {
            img.style.display = 'none';
          });
      });
    }
    rewrite();
    const mo = new MutationObserver(rewrite);
    mo.observe(node, { childList: true, subtree: true });
    return {
      destroy() {
        mo.disconnect();
      },
    };
  }

  // Thumbnail URIs — $state array so Svelte tracks per-index mutations
  let thumbs = $state<(string | null)[]>([]);
  const thumbFetching = new Set<number>();
  // Generation counter: incremented when articles list changes so that
  // stale in-flight fetches from a previous article list are discarded.
  let thumbGeneration = 0;

  // Bulk-resolve cached images on disk in one synchronous IPC call, then
  // kick off network fetches only for the ones that are missing.
  $effect(() => {
    if (thumbs.length !== articles.length) {
      thumbs = Array(articles.length).fill(null);
      thumbFetching.clear();
      thumbGeneration++;
    }

    const gen = thumbGeneration;
    const imageUrls = articles.map((a) => a.image_url).filter((u): u is string => !!u);

    if (imageUrls.length === 0) return;

    // 1) Bulk-resolve everything already on disk (single IPC, sync on Rust side).
    invoke<[string, string][]>('resolve_cached_images', { urls: imageUrls })
      .then((cached) => {
        if (thumbGeneration !== gen) return; // articles changed — discard
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
        if (thumbGeneration !== gen) return; // articles changed — discard
        // 2) Fetch anything still missing from the network.
        articles.forEach((article, i) => {
          if (thumbFetching.has(i) || !article.image_url) return;
          thumbFetching.add(i);
          fetchImage(article.image_url)
            .then((uri) => {
              if (thumbGeneration === gen) thumbs[i] = uri;
            })
            .catch(() => {});
        });
      });
  });

  // Hero image for reading pane — synchronous lookup first, async fallback.
  let heroDataUri = $state<string | null>(null);

  // Lightbox
  let lightboxSrc = $state<string | null>(null);

  function openLightbox(src: string) {
    lightboxSrc = src;
  }
  function closeLightbox() {
    lightboxSrc = null;
  }

  function onLightboxKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') closeLightbox();
  }

  function updateHero() {
    const url = selected?.image_url ?? null;
    if (!url) {
      heroDataUri = null;
      return;
    }
    const cached = imgCache.get(url);
    if (cached) {
      heroDataUri = cached;
      return;
    }
    heroDataUri = null;
    fetchImage(url)
      .then((uri) => {
        heroDataUri = uri;
      })
      .catch(() => {});
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
      <button class="btn btn-ghost btn-xs p-1" onclick={onRefresh} disabled={loading} title="Refresh">
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
        {#each [1, 2, 3, 4] as _}
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
                <div class="flex flex-col gap-0.5 mt-1">
                  {#if article.category}
                    <span
                      class="text-primary/70 font-semibold uppercase truncate leading-none"
                      style="font-size:9px; letter-spacing:0.05em;"
                    >
                      {article.category}
                    </span>
                  {/if}
                  <span class="text-base-content/35 truncate leading-none" style="font-size:9px;">{article.date}</span>
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
            <button
              type="button"
              class="w-full h-full p-0 border-0 bg-transparent cursor-zoom-in"
              onclick={() => {
                if (heroDataUri) lightboxSrc === heroDataUri ? closeLightbox() : openLightbox(heroDataUri);
              }}
            >
              <img src={heroDataUri} alt="" class="w-full h-full object-cover" />
            </button>
            <!-- Gradient overlay so title text reads well -->
            <div
              class="absolute inset-0 bg-gradient-to-t from-base-100 via-base-100/30 to-transparent pointer-events-none"
            ></div>
          {:else}
            <div class="w-full h-full animate-pulse bg-base-300"></div>
          {/if}

          <!-- Title overlaid on hero — pointer-events-none so clicks pass through to the zoom button -->
          {#if heroDataUri}
            <div class="absolute bottom-0 left-0 right-0 px-6 pb-4 pointer-events-none">
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
                  class="ml-auto flex items-center gap-1 text-xs text-base-content/50 hover:text-primary transition-colors pointer-events-auto"
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

<!-- ── Lightbox ──────────────────────────────────────────────────────────────── -->
{#if lightboxSrc}
  <div
    class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/85 backdrop-blur-sm"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={closeLightbox}
    onkeydown={onLightboxKeydown}
  >
    <button
      type="button"
      class="p-0 border-0 bg-transparent cursor-zoom-out max-w-[90vw] max-h-[90vh] flex items-center justify-center"
      onclick={(e) => {
        e.stopPropagation();
        closeLightbox();
      }}
    >
      <img src={lightboxSrc} alt="" class="max-w-full max-h-[90vh] object-contain rounded shadow-2xl select-none" />
    </button>
    <button
      class="absolute top-3 right-3 btn btn-ghost btn-sm btn-circle text-white/70 hover:text-white hover:bg-white/10"
      onclick={closeLightbox}
      title="Close"
    >
      <Icon icon="ph:x" class="size-5" />
    </button>
  </div>
{/if}

<style></style>

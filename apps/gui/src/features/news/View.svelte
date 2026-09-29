<script lang="ts">
  import { dict } from "$lib/i18n";
  import Newspaper from "~icons/lucide/newspaper";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import ExternalLink from "~icons/lucide/external-link";
  import CloudOff from "~icons/lucide/cloud-off";
  import ImageIcon from "~icons/lucide/image";
  import Clock from "~icons/lucide/clock";
  import CalendarDays from "~icons/lucide/calendar-days";
  import PenLine from "~icons/lucide/pen-line";
  import { Button } from "$lib/components/ui/button";
  import { Empty } from "$lib/components/app";
  import SafeHtml from "$lib/components/app/SafeHtml.svelte";
  import Lightbox from "$lib/components/app/Lightbox.svelte";
  import { Tag } from "$lib/components/ui/tag";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { Topo } from "$lib/components/ui/topo";
  import { openUrl } from "$lib/ipc/native";
  import { date, relative } from "$lib/format";
  import { cn } from "$lib/cx";
  import { news, readMinutes } from "./news.svelte";

  const n = dict("news");
  void news.load();

  let category = $state<string | null>(null);
  const categories = $derived([
    ...new Set(news.articles.map((a) => a.category).filter((c): c is string => !!c)),
  ]);
  const shown = $derived(
    category ? news.articles.filter((a) => a.category === category) : news.articles,
  );
  const article = $derived(shown[news.selected] ?? shown[0] ?? null);

  // The hero picture, once cached; asking for it fills the cache.
  const hero = $derived(article?.image_url ? (news.images.get(article.image_url) ?? null) : null);
  $effect(() => {
    const url = article?.image_url;
    if (url && !news.images.has(url)) news.image(url).catch(() => {});
  });

  let lightbox = $state<string | null>(null);
  let reader = $state<HTMLDivElement>();
  $effect(() => {
    void article;
    reader?.scrollTo({ top: 0 });
  });

  const ts = (d: string) => Math.floor(new Date(d).getTime() / 1000);

  function zoom(shown: string, original: string) {
    lightbox = shown;
    if (original !== shown)
      news
        .image(original)
        .then((src) => lightbox && (lightbox = src))
        .catch(() => {});
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, textarea")) return;
    if (lightbox) return;
    if (e.key === "ArrowDown" || e.key === "j")
      news.selected = Math.min(shown.length - 1, news.selected + 1);
    else if (e.key === "ArrowUp" || e.key === "k") news.selected = Math.max(0, news.selected - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

<div class="flex min-h-0 flex-1">
  <!-- The list of articles. -->
  <aside class="flex w-[22rem] shrink-0 flex-col border-r border-border bg-bg/40">
    <header class="flex h-control-lg shrink-0 items-center gap-2 border-b border-border px-pad">
      <Newspaper class="size-icon text-accent" />
      <h1 class="m-0 title-display text-lg text-fg">{$n.title.value}</h1>
      {#if news.articles.length}
        <span class="font-mono text-3xs text-fg-faint"
          >{$n.articles({ count: news.articles.length }).value}</span
        >
      {/if}
      <div class="ml-auto flex items-center gap-0.5">
        <Tooltip text={$n.refresh.value} side="bottom">
          <button
            class="grid size-control place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
            aria-label={$n.refresh.value}
            disabled={news.loading}
            onclick={() => news.load(true)}
          >
            <RefreshCw class={cn("size-icon-sm", news.loading && "animate-spin")} />
          </button>
        </Tooltip>
        <Tooltip text={$n.openAllOnSite.value} side="bottom">
          <button
            class="grid size-control place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            aria-label={$n.openAllOnSite.value}
            onclick={() => openUrl("https://dayz.com/news")}
            ><ExternalLink class="size-icon-sm" /></button
          >
        </Tooltip>
      </div>
    </header>

    {#if categories.length > 1}
      <div class="flex shrink-0 flex-wrap gap-1 border-b border-border/60 px-pad py-1.5">
        {#each [null, ...categories] as c (c ?? "")}
          <button
            class={cn(
              "h-control-sm rounded-sm border px-2 text-2xs",
              category === c
                ? "border-accent/60 bg-accent/10 text-accent"
                : "border-border text-fg-muted hover:border-border-strong hover:text-fg",
            )}
            onclick={() => {
              category = c;
              news.selected = 0;
            }}>{c ?? $n.allCategories.value}</button
          >
        {/each}
      </div>
    {/if}

    <div class="min-h-0 flex-1 overflow-y-auto" role="listbox" aria-label={$n.title.value}>
      {#if news.loading && news.articles.length === 0}
        {#each Array(7) as _, i (i)}
          <div class="flex gap-3 border-b border-border/50 px-pad py-2.5">
            <div class="h-14 w-20 shrink-0 animate-pulse rounded-sm bg-raised"></div>
            <div class="flex-1 space-y-1.5 pt-1">
              <div class="h-2.5 w-16 animate-pulse rounded-xs bg-raised"></div>
              <div class="h-3 w-full animate-pulse rounded-xs bg-raised"></div>
              <div class="h-3 w-2/3 animate-pulse rounded-xs bg-raised"></div>
            </div>
          </div>
        {/each}
      {:else}
        {#each shown as a, i (a.slug)}
          {@const on = article?.slug === a.slug}
          {@const thumb = a.image_url ? news.images.get(a.image_url) : null}
          <button
            role="option"
            aria-selected={on}
            class={cn(
              "relative flex w-full gap-3 border-b border-border/50 px-pad py-2.5 text-left transition-colors",
              on ? "bg-raised" : "hover:bg-raised/50",
            )}
            onclick={() => (news.selected = i)}
          >
            {#if on}<span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-accent"
              ></span>{/if}
            <div
              class="grid h-14 w-20 shrink-0 place-items-center overflow-hidden rounded-sm border border-border bg-raised"
            >
              {#if thumb}
                <img src={thumb} alt="" class="size-full object-cover" loading="lazy" />
              {:else}
                <ImageIcon class="size-icon text-fg-faint/50" />
              {/if}
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 text-3xs text-fg-faint">
                {#if i === 0 && !category}<span class="label-stencil text-accent"
                    >{$n.latest.value}</span
                  >{/if}
                {#if a.category}<span class="truncate uppercase">{a.category}</span>{/if}
                <span class="ml-auto shrink-0 font-mono" title={date(a.date)}
                  >{relative(ts(a.date))}</span
                >
              </div>
              <div
                class={cn(
                  "mt-0.5 line-clamp-2 text-xs leading-snug font-semibold",
                  on ? "text-fg" : "text-fg-muted",
                )}
              >
                {a.title}
              </div>
              {#if a.excerpt}
                <div class="mt-0.5 line-clamp-1 text-2xs text-fg-faint">{a.excerpt}</div>
              {/if}
            </div>
          </button>
        {:else}
          {#if !news.error}<Empty icon={Newspaper} title={$n.empty.value} compact />{/if}
        {/each}
      {/if}
    </div>
  </aside>

  <!-- The article being read. -->
  <div bind:this={reader} class="relative min-w-0 flex-1 overflow-y-auto">
    {#if news.error && news.articles.length === 0}
      <Empty icon={CloudOff} title={$n.loadFailedTitle.value}>
        {$n.loadFailedHint.value}
        <span class="mt-1 block font-mono text-3xs text-fg-faint" data-selectable>{news.error}</span
        >
        {#snippet action()}
          <Button variant="accent" onclick={() => news.load(true)}>
            <RefreshCw class="size-icon-sm" />{$n.retry.value}
          </Button>
        {/snippet}
      </Empty>
    {:else if article}
      <header class="relative">
        {#if hero}
          <div class="relative h-72 overflow-hidden">
            <img src={hero} alt="" class="size-full object-cover" />
            <div
              class="absolute inset-0 bg-linear-to-t from-panel via-panel/60 to-transparent"
            ></div>
            <button
              class="absolute inset-0 cursor-zoom-in"
              aria-label={$n.zoomImage.value}
              onclick={() => (lightbox = hero)}
            ></button>
          </div>
        {:else}
          <div class="relative h-24 overflow-hidden border-b border-border">
            <Topo opacity={0.7} />
          </div>
        {/if}
        <div class={cn("relative mx-auto max-w-3xl px-8", hero ? "-mt-24" : "pt-4")}>
          <div class="flex flex-wrap items-center gap-2">
            {#if article.category}<Tag tone="accent">{article.category}</Tag>{/if}
          </div>
          <h1 class="m-0 mt-2 title-display text-3xl leading-[0.95] text-fg">{article.title}</h1>
          <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-2xs text-fg-muted">
            <span class="flex items-center gap-1"
              ><CalendarDays class="size-3.5" />{date(article.date)}</span
            >
            <span class="flex items-center gap-1"
              ><Clock class="size-3.5" />{$n.readTime({ minutes: readMinutes(article) })
                .value}</span
            >
            {#if article.author}
              <span class="flex items-center gap-1"
                ><PenLine class="size-3.5" />{$n.by({ author: article.author }).value}</span
              >
            {/if}
            <Button size="xs" variant="ghost" class="ml-auto" onclick={() => openUrl(article.url)}>
              <ExternalLink class="size-icon-sm" />{$n.openBrowser.value}
            </Button>
          </div>
        </div>
      </header>

      <div class="mx-auto max-w-3xl px-8 pt-5 pb-12">
        {#if article.excerpt}
          <p
            class="m-0 mb-5 border-l-2 border-accent pl-3 text-sm leading-relaxed text-fg-muted italic"
          >
            {article.excerpt}
          </p>
        {/if}
        {#if article.content_html}
          <SafeHtml
            html={article.content_html}
            base={article.url}
            resolveImage={(u) => news.image(u)}
            onimage={zoom}
          />
        {:else}
          <p class="text-sm leading-relaxed whitespace-pre-wrap text-fg-muted" data-selectable>
            {article.content_text || $n.noContent.value}
          </p>
        {/if}
      </div>
    {:else if !news.loading}
      <Empty icon={Newspaper} title={$n.selectArticle.value} />
    {/if}
  </div>
</div>

<Lightbox bind:src={lightbox} closeLabel={$n.close.value} />

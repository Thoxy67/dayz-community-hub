<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { dict } from "$lib/i18n";
  import type { Component } from "svelte";
  import ServerIcon from "~icons/lucide/server";
  import Puzzle from "~icons/lucide/puzzle";
  import ChartLine from "~icons/lucide/chart-line";
  import Rocket from "~icons/lucide/rocket";
  import GitBranch from "~icons/lucide/git-branch";
  import Tag from "~icons/lucide/tag";
  import Bug from "~icons/lucide/bug";
  import BookOpen from "~icons/lucide/book-open";
  import Scale from "~icons/lucide/scale";
  import Cpu from "~icons/lucide/cpu";
  import MemoryStick from "~icons/lucide/memory-stick";
  import Keyboard from "~icons/lucide/keyboard";
  import Lightbulb from "~icons/lucide/lightbulb";
  import Share from "~icons/lucide/share-2";
  import ListChecks from "~icons/lucide/list-checks";
  import Layers from "~icons/lucide/layers";
  import Heart from "~icons/lucide/heart";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Copy } from "$lib/components/ui/copy";
  import { Topo } from "$lib/components/ui/topo";
  import { inTauri } from "$lib/ipc/core";
  import { openUrl } from "$lib/ipc/native";
  import { getSystemSpecs } from "$lib/ipc/system";
  import type { SystemSpecsDto } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { num } from "$lib/format";
  import UpdateCard from "./UpdateCard.svelte";
  import { AUTHOR, ISSUES_URL, LICENSE_URL, RELEASES_URL, REPO_URL, WIKI_URL } from "./links";

  /**
   * What this launcher is, how to use it, and whether it is current: the
   * version and its update, the ways in, the keys, the machine it runs on.
   */
  const a = dict("about");

  let version = $state("");
  let specs = $state<SystemSpecsDto | null>(null);
  if (inTauri) getVersion().then((v) => (version = v)).catch(() => {});
  getSystemSpecs().then((s) => (specs = s)).catch(() => {});

  // Opened from the title bar's update badge: bring the update card to the eye.
  let highlight = $state(false);
  $effect(() => {
    if (app.view === "about" && app.focus === "update") {
      highlight = true;
      app.focus = null;
      requestAnimationFrame(() => document.getElementById("update")?.scrollIntoView({ behavior: "smooth", block: "center" }));
      setTimeout(() => (highlight = false), 2500);
    }
  });
  $effect(() => {
    if (updater.state === "idle") void updater.check();
  });

  type Feature = { icon: Component<{ class?: string }>; title: () => string; tone: string };
  const FEATURES: Feature[] = [
    { icon: ServerIcon, title: () => $a.featBrowser.value, tone: "text-info" },
    { icon: Puzzle, title: () => $a.featMods.value, tone: "text-mods" },
    { icon: ChartLine, title: () => $a.featBm.value, tone: "text-ok" },
    { icon: Rocket, title: () => $a.featLaunch.value, tone: "text-accent" },
  ];

  const LINKS = [
    { icon: GitBranch, label: () => $a.sourceRepo.value, url: REPO_URL },
    { icon: Tag, label: () => $a.linkReleases.value, url: RELEASES_URL },
    { icon: Bug, label: () => $a.linkBug.value, url: ISSUES_URL },
    { icon: BookOpen, label: () => $a.linkWiki.value, url: WIKI_URL },
    { icon: Scale, label: () => $a.linkLicense.value, url: LICENSE_URL },
  ];

  const SHORTCUTS: { group: () => string; rows: { keys: string[]; label: () => string }[] }[] = [
    {
      group: () => $a.shortcutsGlobal.value,
      rows: [
        { keys: ["Ctrl", "1…9"], label: () => $a.shortcutTab.value },
        { keys: ["Ctrl", "R"], label: () => $a.shortcutRefresh.value },
        { keys: ["Ctrl", "U"], label: () => $a.shortcutUpdate.value },
        { keys: ["Ctrl", "L"], label: () => $a.shortcutReconnect.value },
      ],
    },
    {
      group: () => $a.shortcutsServers.value,
      rows: [
        { keys: ["↑", "↓"], label: () => $a.shortcutNav.value },
        { keys: ["Enter"], label: () => $a.shortcutConnect.value },
        { keys: ["Dbl-click"], label: () => $a.shortcutDblclick.value },
        { keys: ["F"], label: () => $a.shortcutFav.value },
        { keys: ["I"], label: () => $a.shortcutInfo.value },
        { keys: ["P"], label: () => $a.shortcutPing.value },
        { keys: ["D"], label: () => $a.shortcutDirect.value },
        { keys: ["Esc"], label: () => $a.shortcutClose.value },
      ],
    },
    {
      group: () => $a.shortcutsMods.value,
      rows: [
        { keys: ["Space"], label: () => $a.shortcutSpace.value },
        { keys: ["M"], label: () => $a.shortcutManaged.value },
        { keys: ["Dbl-click"], label: () => $a.shortcutOpenfolder.value },
      ],
    },
  ];

  const STACK = [
    { name: "Tauri", url: "https://tauri.app" },
    { name: "Rust", url: "https://www.rust-lang.org" },
    { name: "Svelte", url: "https://svelte.dev" },
    { name: "bits-ui", url: "https://bits-ui.com" },
    { name: "Tailwind CSS", url: "https://tailwindcss.com" },
    { name: "intlayer", url: "https://intlayer.org" },
  ];
</script>

{#snippet card(title: string, icon: Component<{ class?: string }>, body: import("svelte").Snippet)}
  {@const I = icon}
  <section class="overflow-hidden rounded-md border border-border bg-bg/60">
    <h2 class="m-0 flex items-center gap-2 border-b border-border/60 px-pad py-2">
      <I class="size-icon-sm text-accent" /><span class="label-stencil text-fg-muted">{title}</span>
    </h2>
    <div class="px-pad py-2.5">{@render body()}</div>
  </section>
{/snippet}

{#snippet steps(list: string[])}
  <ol class="m-0 flex list-none flex-col gap-2 p-0">
    {#each list as text, i (i)}
      <li class="flex gap-2.5 text-xs leading-relaxed text-fg-muted">
        <span class="grid size-5 shrink-0 place-items-center rounded-full border border-accent/50 font-mono text-3xs text-accent">{i + 1}</span>
        <span class="pt-0.5">{text}</span>
      </li>
    {/each}
  </ol>
{/snippet}

<div class="h-full overflow-y-auto">
  <div class="mx-auto flex max-w-6xl flex-col gap-4 px-6 py-5">
    <!-- Who this is. -->
    <header class="relative overflow-hidden rounded-md border border-border bg-bg">
      <Topo opacity={0.55} />
      <div class="relative flex items-center gap-5 px-6 py-5">
        <img src="/icon.svg" alt="" class="size-16 shrink-0" />
        <div class="min-w-0 flex-1">
          <h1 class="m-0 title-display text-3xl leading-none text-fg">DayZ <span class="text-accent">Community Hub</span></h1>
          <p class="m-0 mt-1.5 text-sm text-fg-muted">{$a.heroTagline.value}</p>
          <p class="m-0 mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 font-mono text-2xs text-fg-faint">
            <span>v{version || "—"}</span><span>·</span><span>{$a.licenseMit.value}</span><span>·</span>
            <span class="inline-flex items-center gap-1 whitespace-nowrap">{$a.madeWith.value} <Heart class="size-3 text-err" /> {$a.by.value} {AUTHOR}</span>
          </p>
        </div>
        <div class="grid shrink-0 grid-cols-2 gap-1.5 max-xl:hidden">
          {#each FEATURES as f, fi (fi)}
            {@const I = f.icon}
            <span class="flex items-center gap-1.5 rounded-sm border border-border bg-panel/70 px-2 py-1 text-2xs text-fg-muted">
              <I class="size-3.5 {f.tone}" />{f.title()}
            </span>
          {/each}
        </div>
      </div>
      <nav class="relative flex flex-wrap gap-1 border-t border-border/70 bg-panel/60 px-4 py-1.5">
        {#each LINKS as l (l.url)}
          {@const I = l.icon}
          <button
            type="button"
            class="flex h-control items-center gap-1.5 rounded-sm px-2 text-xs text-fg-muted hover:bg-raised hover:text-fg"
            onclick={() => openUrl(l.url)}><I class="size-icon-sm" />{l.label()}</button
          >
        {/each}
      </nav>
    </header>

    <UpdateCard {version} {highlight} />

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <div class="flex min-w-0 flex-col gap-4">
        {#snippet quick()}
          {@render steps([
            `${$a.qsStep1Title.value} — ${$a.qsStep1Body.value}`,
            `${$a.qsStep2Title.value} — ${$a.qsStep2Body.value}`,
            `${$a.qsStep3Title.value} — ${$a.qsStep3Body.value}`,
          ])}
        {/snippet}
        {@render card($a.quickstart.value, ListChecks, quick)}

        {#snippet modflow()}
          <p class="m-0 mb-2 text-xs text-fg-muted"><span class="font-medium text-fg">{$a.steamcmdWhat.value}</span> {$a.steamcmdDesc({ notFound: $a.steamcmdNotFound.value }).value}</p>
          {@render steps([$a.modStep1.value, $a.modStep2.value, $a.modStep3.value])}
        {/snippet}
        {@render card($a.modWorkflow.value, Puzzle, modflow)}

        {#snippet sharing()}
          <p class="m-0 mb-2.5 text-xs text-fg-muted">{$a.sharingDesc({ url: "dzch://", file: ".dzch" }).value}</p>
          <div class="grid gap-1.5 rounded-sm border border-border bg-bg p-2">
            {#each [{ label: $a.sharingBasic.value, url: "dzch://1.2.3.4:2302" }, { label: $a.sharingWithMods.value, url: "dzch://1.2.3.4:2302?mods=1559212036,1564026768" }, { label: $a.sharingFull.value, url: "dzch://1.2.3.4:2302?qport=27016&name=My%20Server&password=secret&mods=1559212036" }] as ex (ex.url)}
              <div class="flex items-center gap-2">
                <span class="w-24 shrink-0 text-2xs text-fg-faint">{ex.label}</span>
                <Copy text={ex.url} class="min-w-0 text-fg-muted" />
              </div>
            {/each}
          </div>
          <dl class="m-0 mt-2 grid grid-cols-[5.5rem_1fr] gap-x-3 gap-y-1 text-2xs">
            <dt class="font-mono text-accent">qport</dt><dd class="m-0 text-fg-muted">{$a.sharingParamQport.value}</dd>
            <dt class="font-mono text-accent">name</dt><dd class="m-0 text-fg-muted">{$a.sharingParamName.value}</dd>
            <dt class="font-mono text-accent">password</dt><dd class="m-0 text-fg-muted">{$a.sharingParamPassword.value}</dd>
            <dt class="font-mono text-accent">mods</dt><dd class="m-0 text-fg-muted">{$a.sharingParamMods.value}</dd>
          </dl>
          <p class="m-0 mt-3 mb-1.5 text-2xs font-medium text-fg">{$a.sharingFiles.value}</p>
          <p class="m-0 mb-2 text-2xs text-fg-muted">{$a.sharingFilesDesc({ button: $a.sharingFilesButton.value }).value}</p>
          <p class="m-0 mb-1.5 text-2xs font-medium text-fg">{$a.sharingFromDc.value}</p>
          {@render steps([$a.sharingDcStep1.value, $a.sharingDcStep2.value, $a.sharingDcStep3.value])}
        {/snippet}
        {@render card($a.sharing.value, Share, sharing)}

        {#snippet tips()}
          <p class="m-0 mb-1.5 text-xs font-medium text-fg">{$a.tipAuthTitle.value}</p>
          {@render steps([$a.tipAuthStep1.value, $a.tipAuthStep2.value, $a.tipAuthStep3.value, $a.tipAuthStep4.value])}
          <p class="m-0 mt-3 mb-1 text-xs font-medium text-fg">{$a.tipPerfTitle.value}</p>
          <p class="m-0 text-xs text-fg-muted">{$a.tipPerfDesc.value}</p>
        {/snippet}
        {@render card($a.tips.value, Lightbulb, tips)}
      </div>

      <div class="flex min-w-0 flex-col gap-4">
        {#snippet keys()}
          <div class="flex flex-col gap-3">
            {#each SHORTCUTS as g, gi (gi)}
              <div>
                <p class="m-0 mb-1 font-mono text-3xs tracking-[0.08em] text-fg-faint uppercase">{g.group()}</p>
                <ul class="m-0 flex list-none flex-col p-0">
                  {#each g.rows as r, ri (ri)}
                    <li class="flex items-center gap-2 border-b border-border/40 py-1 last:border-b-0">
                      <span class="flex-1 text-2xs text-fg-muted">{r.label()}</span>
                      <span class="flex shrink-0 items-center gap-0.5">
                        {#each r.keys as k, i (i)}{#if i > 0}<span class="text-3xs text-fg-faint">{r.keys[0] === "Ctrl" ? "+" : "/"}</span>{/if}<Kbd>{k}</Kbd>{/each}
                      </span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        {/snippet}
        {@render card($a.shortcuts.value, Keyboard, keys)}

        {#snippet machine()}
          {#if specs}
            <dl class="m-0 grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-1.5 text-xs">
              <dt class="flex items-center gap-1.5 text-fg-faint"><Cpu class="size-3.5" />{$a.systemCpu.value}</dt>
              <dd class="m-0 text-right font-mono text-fg">{$a.systemCores({ physical: specs.physical_cores, logical: specs.logical_cores }).value}</dd>
              <dt class="flex items-center gap-1.5 text-fg-faint"><MemoryStick class="size-3.5" />{$a.systemMemory.value}</dt>
              <dd class="m-0 text-right font-mono text-fg">{num(Math.round(specs.total_memory_mb / 1024))} GB</dd>
            </dl>
            <p class="m-0 mt-2 text-2xs text-fg-faint">{$a.systemHint.value}</p>
          {:else}
            <p class="m-0 text-2xs text-fg-faint">—</p>
          {/if}
        {/snippet}
        {@render card($a.system.value, Cpu, machine)}

        {#snippet stack()}
          <div class="flex flex-wrap gap-1.5">
            {#each STACK as t (t.name)}
              <button
                type="button"
                class="rounded-sm border border-border bg-panel/70 px-2 py-1 font-mono text-2xs text-fg-muted hover:border-border-strong hover:text-fg"
                onclick={() => openUrl(t.url)}>{t.name}</button
              >
            {/each}
          </div>
          <p class="m-0 mt-2 text-2xs text-fg-faint">{$a.openSource.value} · {$a.licenseMit.value} · {$a.forgejo.value}</p>
        {/snippet}
        {@render card($a.builtWith.value, Layers, stack)}
      </div>
    </div>
  </div>
</div>

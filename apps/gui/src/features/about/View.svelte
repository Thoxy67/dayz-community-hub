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
  import Monitor from "~icons/lucide/monitor";
  import Gpu from "~icons/lucide/gpu";
  import ClipboardCopy from "~icons/lucide/clipboard-copy";
  import Database from "~icons/lucide/database";
  import Radar from "~icons/lucide/radar";
  import WifiOff from "~icons/lucide/wifi-off";
  import Newspaper from "~icons/lucide/newspaper";
  import Gamepad from "~icons/lucide/gamepad-2";
  import ExternalLink from "~icons/lucide/external-link";
  import Star from "~icons/lucide/star";
  import { Segmented } from "$lib/components/ui/segmented";
  import { TabStrip } from "$lib/components/ui/tabs";
  import { PadLegend } from "$lib/components/app";
  import { pad } from "$lib/gamepad";
  import { profile } from "$lib/stores/profile.svelte";
  import { say } from "$lib/stores/say";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Copy } from "$lib/components/ui/copy";
  import { Topo } from "$lib/components/ui/topo";
  import { inTauri } from "$lib/ipc/core";
  import { copyText, openUrl } from "$lib/ipc/native";
  import { getSystemSpecs } from "$lib/ipc/system";
  import type { SystemSpecsDto } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { num } from "$lib/format";
  import UpdateCard from "./UpdateCard.svelte";
  import { AUTHOR, ISSUES_URL, LICENSE_URL, RELEASES_URL, REPO_URL, WIKI_URL } from "./links";

  /**
   * What this launcher is and whether it is current, the machine it runs on
   * (and a copy of it for a bug report), where its data comes from, the keys
   * and the controller's buttons, then the guide, one subject at a time.
   */
  const a = dict("about");
  const p = dict("pad");

  let version = $state("");
  let specs = $state<SystemSpecsDto | null>(null);
  if (inTauri)
    getVersion()
      .then((v) => (version = v))
      .catch(() => {});
  getSystemSpecs()
    .then((s) => (specs = s))
    .catch(() => {});

  // Opened from the title bar's update badge: bring the update card to the eye.
  let highlight = $state(false);
  $effect(() => {
    if (app.view === "about" && app.focus === "update") {
      highlight = true;
      app.focus = null;
      requestAnimationFrame(() =>
        document.getElementById("update")?.scrollIntoView({ behavior: "smooth", block: "center" }),
      );
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
    { icon: ChartLine, title: () => $a.featStats.value, tone: "text-ok" },
    { icon: Rocket, title: () => $a.featLaunch.value, tone: "text-accent" },
    { icon: WifiOff, title: () => $a.featOffline.value, tone: "text-warn" },
    { icon: Share, title: () => $a.featShare.value, tone: "text-info" },
    { icon: Gamepad, title: () => $a.featController.value, tone: "text-fg-muted" },
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
        { keys: ["L"], label: () => $a.shortcutLink.value },
        { keys: ["Del"], label: () => $a.shortcutRemove.value },
        { keys: ["Esc"], label: () => $a.shortcutClose.value },
      ],
    },
    {
      group: () => $a.shortcutsMods.value,
      rows: [
        { keys: ["↑", "↓"], label: () => $a.shortcutNav.value },
        { keys: ["Space"], label: () => $a.shortcutSpace.value },
        { keys: ["M"], label: () => $a.shortcutManaged.value },
        { keys: ["U"], label: () => $a.shortcutModUpdate.value },
        { keys: ["Del"], label: () => $a.shortcutModDelete.value },
      ],
    },
    {
      group: () => $a.shortcutsNews.value,
      rows: [{ keys: ["↑", "↓", "J", "K"], label: () => $a.shortcutArticle.value }],
    },
  ];

  type Source = {
    icon: Component<{ class?: string }>;
    title: () => string;
    desc: () => string;
    url?: string;
  };
  const SOURCES: Source[] = [
    {
      icon: ServerIcon,
      title: () => $a.srcServers.value,
      desc: () => $a.srcServersDesc.value,
      url: "https://dayzsalauncher.com",
    },
    {
      icon: Star,
      title: () => $a.srcOfficial.value,
      desc: () => $a.srcOfficialDesc.value,
      url: "https://store.steampowered.com/app/221100",
    },
    {
      icon: Radar,
      title: () => $a.srcLive.value,
      desc: () => $a.srcLiveDesc.value,
    },
    {
      icon: ChartLine,
      title: () => $a.srcStats.value,
      desc: () => $a.srcStatsDesc.value,
      url: "https://dayzmetrics.com",
    },
    {
      icon: Puzzle,
      title: () => $a.srcMods.value,
      desc: () => $a.srcModsDesc.value,
      url: "https://steamcommunity.com/app/221100/workshop/",
    },
    {
      icon: WifiOff,
      title: () => $a.srcOffline.value,
      desc: () => $a.srcOfflineDesc.value,
      url: "https://github.com/Arkensor/DayZCommunityOfflineMode",
    },
    {
      icon: Newspaper,
      title: () => $a.srcNews.value,
      desc: () => $a.srcNewsDesc.value,
      url: "https://dayz.com/news",
    },
  ];

  // The keys, or the controller's buttons when a controller drives.
  let input = $state<"keyboard" | "gamepad">(pad.mode === "gamepad" ? "gamepad" : "keyboard");

  type GuideTab = "start" | "mods" | "share" | "offline" | "tips";
  let guide = $state<GuideTab>("start");

  const gb = (mb: number) => `${num(Math.round(mb / 1024))} GB`;

  /** The machine and the app, as a bug report wants them. */
  async function copyInfo() {
    const lines = [
      `DayZ Community Hub v${version || "?"}`,
      specs?.os ? `OS: ${specs.os}` : null,
      specs
        ? `CPU: ${specs.cpu_name ?? "?"} (${specs.physical_cores}c/${specs.logical_cores}t)`
        : null,
      specs ? `RAM: ${gb(specs.total_memory_mb)}` : null,
      ...(specs?.gpus ?? []).map(
        (g) => `GPU: ${g.name}${g.vram_mb != null ? ` (${gb(g.vram_mb)})` : ""}`,
      ),
      `Mods: ${profile.viaSteam ? "Steam client" : "SteamCMD"}`,
    ].filter(Boolean);
    await copyText(lines.join("\n"));
    say.ok($a.infoCopied.value);
  }

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
        <span
          class="grid size-5 shrink-0 place-items-center rounded-full border border-accent/50 font-mono text-3xs text-accent"
          >{i + 1}</span
        >
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
          <h1 class="m-0 title-display text-3xl leading-none text-fg">
            DayZ <span class="text-accent">Community Hub</span>
          </h1>
          <p class="m-0 mt-1.5 text-sm text-fg-muted">{$a.heroTagline.value}</p>
          <p
            class="m-0 mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 font-mono text-2xs text-fg-faint"
          >
            <span>v{version || "—"}</span><span>·</span><span>{$a.licenseMit.value}</span><span
              >·</span
            >
            <span class="inline-flex items-center gap-1 whitespace-nowrap"
              >{$a.madeWith.value} <Heart class="size-3 text-err" /> {$a.by.value} {AUTHOR}</span
            >
          </p>
        </div>
        <div class="flex max-w-md shrink flex-wrap justify-end gap-1.5 max-xl:hidden">
          {#each FEATURES as f, fi (fi)}
            {@const I = f.icon}
            <span
              class="flex items-center gap-1.5 rounded-sm border border-border bg-panel/70 px-2 py-1 text-2xs text-fg-muted"
            >
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

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <!-- The machine, and a copy of it for a bug report. -->
      {#snippet machine()}
        {#if specs}
          <dl class="m-0 flex flex-col gap-2 text-xs">
            {#if specs.os}
              <div class="flex items-start gap-2">
                <dt class="flex w-24 shrink-0 items-center gap-1.5 text-fg-faint">
                  <Monitor class="size-3.5" />{$a.systemOs.value}
                </dt>
                <dd class="m-0 min-w-0 text-fg" data-selectable>{specs.os}</dd>
              </div>
            {/if}
            <div class="flex items-start gap-2">
              <dt class="flex w-24 shrink-0 items-center gap-1.5 text-fg-faint">
                <Cpu class="size-3.5" />{$a.systemCpu.value}
              </dt>
              <dd class="m-0 min-w-0" data-selectable>
                {#if specs.cpu_name}<span class="block text-fg">{specs.cpu_name}</span>{/if}
                <span class="block font-mono text-2xs text-fg-muted">
                  {$a.systemCores({
                    physical: specs.physical_cores,
                    logical: specs.logical_cores,
                  }).value}
                </span>
              </dd>
            </div>
            <div class="flex items-start gap-2">
              <dt class="flex w-24 shrink-0 items-center gap-1.5 text-fg-faint">
                <MemoryStick class="size-3.5" />{$a.systemMemory.value}
              </dt>
              <dd class="m-0 font-mono text-fg">{gb(specs.total_memory_mb)}</dd>
            </div>
            <div class="flex items-start gap-2">
              <dt class="flex w-24 shrink-0 items-center gap-1.5 text-fg-faint">
                <Gpu class="size-3.5" />{$a.systemGpu.value}
              </dt>
              <dd class="m-0 flex min-w-0 flex-col gap-1" data-selectable>
                {#each specs.gpus as g, gi (gi)}
                  <span>
                    <span class="block text-fg">{g.name}</span>
                    {#if g.vram_mb != null}
                      <span class="block font-mono text-2xs text-fg-muted"
                        >{gb(g.vram_mb)} VRAM</span
                      >
                    {/if}
                  </span>
                {:else}
                  <span class="text-fg-faint">{$a.systemNoGpu.value}</span>
                {/each}
              </dd>
            </div>
          </dl>
          <div class="mt-3 flex items-center gap-2 border-t border-border/50 pt-2.5">
            <button
              type="button"
              class="inline-flex h-control-sm items-center gap-1.5 rounded-sm border border-border bg-panel px-2 text-2xs text-fg-muted hover:border-border-strong hover:text-fg"
              onclick={copyInfo}
            >
              <ClipboardCopy class="size-3.5" />{$a.copyInfo.value}
            </button>
          </div>
          <p class="m-0 mt-2 text-2xs text-fg-faint">{$a.systemHint.value}</p>
        {:else}
          <div class="flex flex-col gap-2">
            <div class="h-4 animate-pulse rounded-xs bg-raised/60"></div>
            <div class="h-4 animate-pulse rounded-xs bg-raised/60"></div>
            <div class="h-4 animate-pulse rounded-xs bg-raised/60"></div>
          </div>
        {/if}
      {/snippet}
      {@render card($a.system.value, Cpu, machine)}

      <!-- Who the app asks, so a player knows what it relies on. -->
      {#snippet sources()}
        <ul class="m-0 flex list-none flex-col gap-0.5 p-0">
          {#each SOURCES as src (src.title())}
            {@const I = src.icon}
            <li>
              <svelte:element
                this={src.url ? "button" : "div"}
                type={src.url ? "button" : undefined}
                class="group flex w-full items-start gap-2.5 rounded-sm px-1.5 py-1.5 text-left {src.url
                  ? 'hover:bg-raised/50'
                  : ''}"
                onclick={src.url ? () => openUrl(src.url!) : undefined}
                role={src.url ? undefined : "group"}
              >
                <I class="mt-0.5 size-3.5 shrink-0 text-accent" />
                <span class="min-w-0 flex-1">
                  <span class="block text-xs text-fg">{src.title()}</span>
                  <span class="block text-2xs leading-snug text-fg-faint">{src.desc()}</span>
                </span>
                {#if src.url}
                  <ExternalLink
                    class="mt-0.5 size-3 shrink-0 text-fg-faint opacity-0 group-hover:opacity-100"
                  />
                {/if}
              </svelte:element>
            </li>
          {/each}
        </ul>
      {/snippet}
      {@render card($a.dataSources.value, Database, sources)}

      <!-- The keys, or the controller's buttons. -->
      {#snippet keys()}
        <Segmented
          bind:value={input}
          size="xs"
          fill
          aria-label={$a.shortcuts.value}
          options={[
            { value: "keyboard", label: $a.keyboard.value, icon: Keyboard },
            { value: "gamepad", label: $a.gamepad.value, icon: Gamepad },
          ]}
          class="mb-3"
        />
        {#if input === "keyboard"}
          <div class="flex flex-col gap-3">
            {#each SHORTCUTS as g, gi (gi)}
              <div>
                <p class="m-0 mb-1 font-mono text-3xs tracking-[0.08em] text-fg-faint uppercase">
                  {g.group()}
                </p>
                <ul class="m-0 flex list-none flex-col p-0">
                  {#each g.rows as r, ri (ri)}
                    <li
                      class="flex items-center gap-2 border-b border-border/40 py-1 last:border-b-0"
                    >
                      <span class="flex-1 text-2xs text-fg-muted">{r.label()}</span>
                      <span class="flex shrink-0 items-center gap-0.5">
                        {#each r.keys as k, i (i)}{#if i > 0}<span class="text-3xs text-fg-faint"
                              >{r.keys[0] === "Ctrl" ? "+" : "/"}</span
                            >{/if}<Kbd>{k}</Kbd>{/each}
                      </span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        {:else}
          <PadLegend />
          <p class="m-0 mt-3 mb-1 font-mono text-3xs tracking-[0.08em] text-fg-faint uppercase">
            {$a.padInServers.value}
          </p>
          <PadLegend
            only
            extra={[
              { buttons: ["x"], text: $p.refresh.value },
              { buttons: ["y"], text: $p.favorite.value },
            ]}
          />
          <p class="m-0 mt-3 mb-1 font-mono text-3xs tracking-[0.08em] text-fg-faint uppercase">
            {$a.padInSaved.value}
          </p>
          <PadLegend
            only
            extra={[
              { buttons: ["x"], text: $p.join.value },
              { buttons: ["y"], text: $p.favorite.value },
            ]}
          />
          {#if pad.pads[0]}
            <p class="m-0 mt-3 flex items-center gap-1.5 text-2xs text-fg-faint">
              <Gamepad class="size-3.5 shrink-0" /><span class="truncate">{pad.pads[0].name}</span>
            </p>
          {/if}
        {/if}
      {/snippet}
      {@render card($a.shortcuts.value, Keyboard, keys)}
    </div>

    <!-- The guide: one subject at a time instead of a wall of text. -->
    <section class="overflow-hidden rounded-md border border-border bg-bg/60">
      <div class="flex items-center gap-2 border-b border-border/60 px-pad pt-1">
        <BookOpen class="size-icon-sm shrink-0 text-accent" />
        <span class="mr-2 label-stencil text-fg-muted">{$a.guide.value}</span>
        <TabStrip
          bind:value={guide}
          size="xs"
          aria-label={$a.guide.value}
          tabs={[
            { id: "start", label: $a.quickstart.value, icon: ListChecks },
            { id: "mods", label: $a.modWorkflow.value, icon: Puzzle },
            { id: "share", label: $a.sharing.value, icon: Share },
            { id: "offline", label: $a.srcOffline.value, icon: WifiOff },
            { id: "tips", label: $a.tips.value, icon: Lightbulb },
          ]}
        />
      </div>
      <div class="px-pad py-3">
        {#if guide === "start"}
          {@render steps([
            `${$a.qsStep1Title.value} — ${$a.qsStep1Body.value}`,
            `${$a.qsStep2Title.value} — ${$a.qsStep2Body.value}`,
            `${$a.qsStep3Title.value} — ${$a.qsStep3Body.value}`,
          ])}
        {:else if guide === "mods"}
          <p class="m-0 mb-2 text-xs text-fg-muted">
            <span class="font-medium text-fg">{$a.steamcmdWhat.value}</span>
            {$a.steamcmdDesc({ notFound: $a.steamcmdNotFound.value }).value}
          </p>
          {@render steps([$a.modStep1.value, $a.modStep2.value, $a.modStep3.value])}
          <p class="m-0 mt-3 text-2xs text-fg-faint">{$a.modDownloaderTip.value}</p>
        {:else if guide === "offline"}
          {@render steps([$a.offStep1.value, $a.offStep2.value, $a.offStep3.value])}
        {:else if guide === "share"}
          <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <div>
              <p class="m-0 mb-2.5 text-xs text-fg-muted">
                {$a.sharingDesc({ url: "dzch://", file: ".dzch" }).value}
              </p>
              <div class="grid gap-1.5 rounded-sm border border-border bg-bg p-2">
                {#each [{ label: $a.sharingBasic.value, url: "dzch://1.2.3.4:2302" }, { label: $a.sharingWithMods.value, url: "dzch://1.2.3.4:2302?mods=1559212036,1564026768" }, { label: $a.sharingFull.value, url: "dzch://1.2.3.4:2302?qport=27016&name=My%20Server&password=secret&mods=1559212036" }] as ex (ex.url)}
                  <div class="flex items-start gap-2">
                    <span class="w-24 shrink-0 text-2xs text-fg-faint">{ex.label}</span>
                    <!-- Wrapped, not cut: the end of a long link is the part worth reading. -->
                    <Copy
                      text={ex.url}
                      class="min-w-0 flex-1 text-left text-fg-muted [&>span]:break-all [&>span]:whitespace-normal"
                    />
                  </div>
                {/each}
              </div>
              <dl class="m-0 mt-2 grid grid-cols-[5.5rem_1fr] gap-x-3 gap-y-1 text-2xs">
                <dt class="font-mono text-accent">qport</dt>
                <dd class="m-0 text-fg-muted">{$a.sharingParamQport.value}</dd>
                <dt class="font-mono text-accent">name</dt>
                <dd class="m-0 text-fg-muted">{$a.sharingParamName.value}</dd>
                <dt class="font-mono text-accent">password</dt>
                <dd class="m-0 text-fg-muted">{$a.sharingParamPassword.value}</dd>
                <dt class="font-mono text-accent">mods</dt>
                <dd class="m-0 text-fg-muted">{$a.sharingParamMods.value}</dd>
              </dl>
            </div>
            <div>
              <p class="m-0 mb-1.5 text-2xs font-medium text-fg">{$a.sharingFiles.value}</p>
              <p class="m-0 mb-3 text-2xs text-fg-muted">
                {$a.sharingFilesDesc({ button: $a.sharingFilesButton.value }).value}
              </p>
              <p class="m-0 mb-1.5 text-2xs font-medium text-fg">{$a.sharingFromDc.value}</p>
              {@render steps([
                $a.sharingDcStep1.value,
                $a.sharingDcStep2.value,
                $a.sharingDcStep3.value,
              ])}
            </div>
          </div>
        {:else}
          <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <div>
              <p class="m-0 mb-1.5 text-xs font-medium text-fg">{$a.tipAuthTitle.value}</p>
              {@render steps([
                $a.tipAuthStep1.value,
                $a.tipAuthStep2.value,
                $a.tipAuthStep3.value,
                $a.tipAuthStep4.value,
              ])}
            </div>
            <div>
              <p class="m-0 mb-1 text-xs font-medium text-fg">{$a.tipPerfTitle.value}</p>
              <p class="m-0 text-xs text-fg-muted">{$a.tipPerfDesc.value}</p>
            </div>
          </div>
        {/if}
      </div>
    </section>

    <!-- What it is built with, quietly at the end. -->
    <footer class="flex flex-wrap items-center gap-1.5 pb-2 text-2xs text-fg-faint">
      <Layers class="size-3.5 text-accent" />
      <span class="mr-1">{$a.builtWith.value}</span>
      {#each STACK as t (t.name)}
        <button
          type="button"
          class="rounded-sm border border-border bg-panel/70 px-1.5 py-0.5 font-mono text-3xs text-fg-muted hover:border-border-strong hover:text-fg"
          onclick={() => openUrl(t.url)}>{t.name}</button
        >
      {/each}
      <span class="ml-auto">{$a.openSource.value} · {$a.licenseMit.value} · {$a.forgejo.value}</span
      >
    </footer>
  </div>
</div>

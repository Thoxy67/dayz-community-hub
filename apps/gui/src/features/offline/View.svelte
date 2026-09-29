<script lang="ts">
  import { dict } from "$lib/i18n";
  import Tent from "~icons/game-icons/camping-tent";
  import Download from "~icons/lucide/download";
  import FolderOpen from "~icons/lucide/folder-open";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Play from "~icons/lucide/play";
  import Trash from "~icons/lucide/trash-2";
  import Eraser from "~icons/lucide/eraser";
  import ExternalLink from "~icons/lucide/external-link";
  import MapIcon from "~icons/lucide/map";
  import CircleCheck from "~icons/lucide/circle-check";
  import CircleAlert from "~icons/lucide/circle-alert";
  import Info from "~icons/lucide/info";
  import { PageHeader, Figure, Empty } from "$lib/components/app";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Panel } from "$lib/components/ui/panel";
  import { Tag } from "$lib/components/ui/tag";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import { openUrl } from "$lib/ipc/native";
  import { cn } from "$lib/cx";
  import { offline, type Mission, type Tone } from "./offline.svelte";

  const REPO_URL = "https://github.com/Arkensor/DayZCommunityOfflineMode";
  const o = dict("offline");

  offline.listen();
  $effect(() => {
    if (!offline.loaded) void offline.load();
  });

  const list = $derived(offline.described);
  const mapCount = $derived(new Set(list.map((m) => m.mapKey)).size);
  const installed = $derived(list.length > 0);
  const busy = $derived(offline.loading || offline.installing);

  const kindLabel = (m: Mission) =>
    ({
      offline: $o.kindOffline.value,
      coop: $o.kindCoop.value,
      pvp: $o.kindPvp.value,
      survival: $o.kindSurvival.value,
      mission: $o.kindMission.value,
    })[m.kind] ?? m.kind;

  const TONE: Record<Tone, string> = {
    neutral: "border-border bg-raised/40 text-fg-muted",
    ok: "border-ok/30 bg-ok/8 text-ok",
    warn: "border-warn/30 bg-warn/8 text-warn",
    err: "border-err/30 bg-err/8 text-err",
  };
</script>

<PageHeader title={$o.title.value}>
  {#snippet stats()}
    <Figure
      label={$o.state.value}
      value={installed ? $o.installed.value : $o.notInstalled.value}
      tone={installed ? "text-ok" : "text-fg-faint"}
    />
    <Figure label={$o.missions.value} value={String(list.length)} tone="text-accent" />
    <Figure label={$o.maps.value} value={String(mapCount)} tone="text-map" />
  {/snippet}
  {#snippet actions()}
    <IconButton
      icon={RefreshCw}
      label={$o.refreshTitle.value}
      disabled={busy}
      onclick={() => offline.load()}
    />
    <Button onclick={offline.openMissionsDir} title={$o.exploreTitle.value}>
      <FolderOpen class="size-icon-sm" />{$o.explore.value}
    </Button>
    <Button variant="accent" disabled={busy} onclick={() => offline.update()}>
      {#if offline.installing}<Spinner class="size-icon-sm text-accent-fg" />{:else}<Download
          class="size-icon-sm"
        />{/if}
      {offline.installing ? $o.installing.value : $o.installUpdate.value}
    </Button>
  {/snippet}
</PageHeader>

{#if offline.status}
  <div
    class={cn(
      "flex shrink-0 items-center gap-2 border-b px-pad py-1.5 text-2xs",
      TONE[offline.tone],
    )}
    role="status"
  >
    {#if offline.installing || offline.loading}
      <Spinner class="size-icon-sm" />
    {:else if offline.tone === "ok"}
      <CircleCheck class="size-icon-sm" />
    {:else if offline.tone === "neutral"}
      <Info class="size-icon-sm" />
    {:else}
      <CircleAlert class="size-icon-sm" />
    {/if}
    <span class="min-w-0 truncate">{offline.status}</span>
    {#if offline.installing}
      <span class="relative ml-auto h-1 w-40 overflow-hidden rounded-full bg-raised">
        <span class="absolute inset-y-0 w-1/3 animate-sweep rounded-full bg-accent"></span>
      </span>
    {/if}
  </div>
{/if}

<div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_18rem] gap-px bg-border">
  <!-- The missions, one card per map. -->
  <div class="min-h-0 overflow-y-auto bg-panel p-pad">
    {#if !offline.loaded}
      <div class="grid h-full place-items-center"><Spinner class="size-6" /></div>
    {:else if list.length === 0}
      <Empty icon={Tent} title={$o.noMissions.value}>
        {$o.noMissionsHint({ installButton: $o.installButton.value }).value}
        {#snippet action()}
          <Button variant="accent" disabled={busy} onclick={() => offline.update()}>
            <Download class="size-icon-sm" />{$o.installButton.value}
          </Button>
        {/snippet}
      </Empty>
    {:else}
      <p class="m-0 mb-2 label-stencil text-fg-faint">{$o.doubleclickHint.value}</p>
      <ul class="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-2.5 p-0">
        {#each list as m (m.id)}
          <li
            class="group relative flex flex-col overflow-hidden rounded-md border border-border bg-bg transition-colors hover:border-border-strong"
            ondblclick={() => offline.launch(m.id)}
          >
            <!-- The map's name set big over contour lines: a survey sheet's label. -->
            <div class="relative h-24 overflow-hidden border-b border-border bg-raised/50">
              <Topo opacity={0.9} class="text-map/35" />
              <div class="absolute inset-x-3 bottom-2 flex items-end justify-between gap-2">
                <span class="title-display text-2xl leading-none text-fg">{m.map}</span>
                <Tag tone="accent" filled>{kindLabel(m)}</Tag>
              </div>
            </div>
            <dl class="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 px-3 py-2 text-2xs">
              <dt class="text-fg-faint">{$o.terrain.value}</dt>
              <dd class="m-0 flex items-center gap-1 text-map">
                <MapIcon class="size-3" />{m.terrain ?? "—"}
              </dd>
              <dt class="text-fg-faint">{$o.mode.value}</dt>
              <dd class="m-0 text-fg-muted">{kindLabel(m)}</dd>
              <dt class="text-fg-faint">{$o.folder.value}</dt>
              <dd class="m-0 truncate font-mono text-fg-muted" title={m.id} data-selectable>
                {m.id}
              </dd>
            </dl>
            <div class="mt-auto flex items-center gap-1 border-t border-border/70 px-2 py-1.5">
              <Button variant="play" class="flex-1" onclick={() => offline.launch(m.id)}>
                <Play class="size-icon-sm" />{$o.launch.value}
              </Button>
              <IconButton
                icon={FolderOpen}
                label={$o.openFolder.value}
                onclick={() => offline.openDir(m.id)}
              />
              <IconButton
                icon={Trash}
                label={$o.removeMission.value}
                variant="danger"
                disabled={busy}
                onclick={() => offline.removeMission(m.id)}
              />
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!-- What this is, how it works, and the housekeeping. -->
  <aside class="flex min-h-0 flex-col gap-px overflow-y-auto bg-border">
    <Panel title={$o.howItWorks.value} scroll={false}>
      <div class="space-y-2.5 p-3 text-2xs leading-relaxed text-fg-muted">
        <p class="m-0">{$o.intro.value}</p>
        <ol class="m-0 space-y-1.5 pl-0">
          {#each [$o.step1.value, $o.step2.value, $o.step3.value] as step, i (i)}
            <li class="flex gap-2">
              <span
                class="grid size-4 shrink-0 place-items-center rounded-full bg-accent/15 font-mono text-3xs text-accent"
                >{i + 1}</span
              >
              <span>{step}</span>
            </li>
          {/each}
        </ol>
        <button
          class="flex w-full items-start gap-2 rounded-sm border border-border bg-bg px-2 py-1.5 text-left hover:border-border-strong"
          onclick={() => openUrl(REPO_URL)}
        >
          <ExternalLink class="mt-0.5 size-icon-sm shrink-0 text-accent" />
          <span class="min-w-0">
            <span class="block text-2xs font-medium text-accent">{$o.project.value}</span>
            <span class="block truncate font-mono text-3xs text-fg-faint"
              >github.com/Arkensor/DayZCommunityOfflineMode</span
            >
          </span>
        </button>
      </div>
    </Panel>
    <Panel title={$o.maintenance.value} scroll={false} class="flex-1">
      <div class="space-y-2 p-3">
        <div>
          <Button
            class="w-full justify-start"
            disabled={busy || !installed}
            onclick={() => offline.clearSaves()}
          >
            <Eraser class="size-icon-sm" />{$o.clearSaves.value}
          </Button>
          <p class="m-0 mt-1 text-3xs leading-snug text-fg-faint">{$o.clearSavesTitle.value}</p>
        </div>
        <div>
          <Button
            variant="danger"
            class="w-full justify-start"
            disabled={busy || !installed}
            onclick={() => offline.removeAll()}
          >
            <Trash class="size-icon-sm" />{$o.removeAll.value}
          </Button>
          <p class="m-0 mt-1 text-3xs leading-snug text-fg-faint">{$o.removeAllTitle.value}</p>
        </div>
      </div>
    </Panel>
  </aside>
</div>

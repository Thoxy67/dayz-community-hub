<script lang="ts">
  import { dict } from "$lib/i18n";
  import PanelLeftClose from "~icons/lucide/panel-left-close";
  import PanelLeftOpen from "~icons/lucide/panel-left-open";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { Topo } from "$lib/components/ui/topo";
  import { cn } from "$lib/cx";
  import { app, type ViewId } from "$lib/stores/app.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { compact } from "$lib/format";
  import { GROUPS, PLACES, type Place } from "./nav";
  import RejoinCard from "./RejoinCard.svelte";
  import Skull from "~icons/lucide/square-x";
  import { game } from "$lib/stores/game.svelte";
  import { pad } from "$lib/gamepad";
  import { confirm } from "$lib/stores/dialogs.svelte";

  /** Close a running DayZ, after asking: it is closed as a crash would close it. */
  async function closeGame() {
    const ok = await confirm({
      title: $n.killGameTitle.value,
      message: $n.killGameMessage.value,
      confirmLabel: $n.killGame.value,
      danger: true,
    });
    if (ok) await game.close();
  }

  const n = dict("nav");
  const collapsed = $derived(prefs.railCollapsed);

  /** The figure beside each place: how many are in there. */
  function countOf(id: ViewId): { value: string; tone?: string } | null {
    switch (id) {
      case "servers":
        return servers.total ? { value: compact(servers.total) } : null;
      case "favorites":
        return profile.data?.favorites.length
          ? { value: String(profile.data.favorites.length) }
          : null;
      case "history":
        return profile.data?.history.length ? { value: String(profile.data.history.length) } : null;
      case "mods":
        return mods.stale.length
          ? { value: `${mods.stale.length}↑`, tone: "text-warn" }
          : mods.installed.length
            ? { value: String(mods.installed.length) }
            : null;
      default:
        return null;
    }
  }

  const shortcut = (p: Place) => {
    const i = PLACES.indexOf(p);
    return i < 9 ? `Ctrl+${i + 1}` : "";
  };
</script>

{#snippet entry(p: Place)}
  {@const on = app.view === p.id}
  {@const c = countOf(p.id)}
  <Tooltip text={collapsed ? String($n[p.label]) : ""} kbd={shortcut(p)} class="flex">
    <button
      class={cn(
        "group relative flex h-control-lg w-full items-center gap-2.5 rounded-sm text-sm transition-colors",
        collapsed ? "justify-center px-0" : "px-2.5",
        on ? "bg-raised text-fg" : "text-fg-muted hover:bg-raised/60 hover:text-fg",
      )}
      aria-current={on ? "page" : undefined}
      aria-label={String($n[p.label])}
      onclick={() => app.go(p.id)}
    >
      {#if on}<span class="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-accent"></span>{/if}
      <p.icon
        class={cn(
          "size-icon shrink-0",
          on ? "text-accent" : "text-fg-faint group-hover:text-fg-muted",
        )}
      />
      {#if !collapsed}
        <span class="min-w-0 flex-1 truncate text-left">{String($n[p.label])}</span>
        {#if c}<span class={cn("num font-mono text-2xs", c.tone ?? "text-fg-faint")}>{c.value}</span
          >{/if}
      {:else if c?.tone}
        <span class="absolute top-1.5 right-2 size-1.5 rounded-full bg-warn"></span>
      {/if}
    </button>
  </Tooltip>
{/snippet}

<nav
  data-pad-region
  class={cn(
    "relative flex shrink-0 flex-col border-r border-border bg-bg transition-[width] duration-150",
    collapsed ? "w-rail-collapsed" : "w-rail",
  )}
  aria-label={$n.navigation.value}
>
  <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-1.5 py-2.5">
    {#each GROUPS as g (g.id)}
      <div class="flex flex-col gap-0.5">
        {#if !collapsed}
          <span class="label-stencil px-2.5 pb-1 text-fg-faint">{String($n[g.label])}</span>
        {:else}
          <span class="mx-auto mb-1 h-px w-5 bg-border"></span>
        {/if}
        {#each PLACES.filter((p) => p.group === g.id) as p (p.id)}{@render entry(p)}{/each}
      </div>
    {/each}

    <!-- The game is open: a way to close it, under the views. Not with a
         controller, where Steam's own overlay quits the game. -->
    {#if game.running && pad.mode !== "gamepad"}
      <div
        class={cn(
          "flex flex-col gap-1 rounded-sm border border-err/30 bg-err/8",
          collapsed ? "items-center p-1" : "px-2.5 py-2",
        )}
      >
        {#if !collapsed}
          <span class="flex items-center gap-1.5 text-2xs text-fg-muted">
            <span class="size-1.5 animate-pulse rounded-full bg-ok"></span>{$n.gameRunning.value}
          </span>
        {/if}
        <Tooltip text={collapsed ? $n.killGame.value : ""} class="flex">
          <button
            class={cn(
              "flex h-control items-center justify-center gap-1.5 rounded-sm text-xs text-err hover:bg-err/15 disabled:opacity-50",
              collapsed ? "w-full" : "w-full border border-err/40",
            )}
            disabled={game.closing}
            aria-label={$n.killGame.value}
            onclick={closeGame}
          >
            <Skull class="size-icon-sm" />{#if !collapsed}{$n.killGame.value}{/if}
          </button>
        </Tooltip>
      </div>
    {/if}
  </div>

  <div class="relative flex flex-col gap-2 border-t border-border pt-2 pb-1.5">
    <Topo class="opacity-40" />
    <div class="relative"><RejoinCard {collapsed} /></div>
    <div class="relative flex flex-col gap-0.5 px-1.5">
      {#each PLACES.filter((p) => p.group === "foot") as p (p.id)}{@render entry(p)}{/each}
      <button
        class={cn(
          "flex h-control items-center gap-2.5 rounded-sm text-2xs text-fg-faint hover:bg-raised/60 hover:text-fg-muted",
          collapsed ? "justify-center" : "px-2.5",
        )}
        onclick={() => (prefs.railCollapsed = !collapsed)}
        aria-label={collapsed ? $n.expand.value : $n.collapse.value}
      >
        {#if collapsed}<PanelLeftOpen class="size-icon" />{:else}<PanelLeftClose
            class="size-icon"
          />{$n.collapse.value}{/if}
      </button>
    </div>
  </div>
</nav>

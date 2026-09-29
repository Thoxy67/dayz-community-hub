<script lang="ts">
  import { Copy } from "$lib/components/ui/copy";
  import {
    ExcludeButton,
    FavoriteButton,
    ModsCount,
    OsIcon,
    PingButton,
    PlayersButton,
    ServerFlags,
    TimeOfDay,
  } from "$lib/components/app";
  import { dict } from "$lib/i18n";
  import { cn } from "$lib/cx";
  import type { ServerRow } from "$lib/ipc/servers";
  import { GRID } from "./columns";

  /** One server in the browser: every cell is a shared component, so favourites and history read the same. */
  let {
    server,
    index,
    selected,
    onselect,
    onjoin,
    onmods,
  }: {
    server: ServerRow;
    index: number;
    selected: boolean;
    onselect: () => void;
    onjoin: () => void;
    onmods: () => void;
  } = $props();

  const c = dict("servers");
  const address = $derived(`${server.ip}:${server.game_port}`);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  role="row"
  tabindex="-1"
  aria-selected={selected}
  class={cn(
    GRID,
    "group h-full border-b border-border/50 px-2 text-xs transition-colors",
    selected ? "bg-accent/10 shadow-[inset_2px_0_0_var(--color-accent)]" : "hover:bg-raised/50",
    server.excluded && "opacity-45",
  )}
  onclick={onselect}
  ondblclick={onjoin}
>
  <span class="num text-right font-mono text-3xs text-fg-faint">{index + 1}</span>
  <span class="flex flex-col items-center gap-0.5">
    <FavoriteButton name={server.name} ip={server.ip} port={server.query_port} />
    <ExcludeButton ip={server.ip} />
  </span>
  <PingButton ip={server.ip} queryPort={server.query_port} />
  <PlayersButton ip={server.ip} queryPort={server.query_port} />
  <span class="flex min-w-0 flex-col gap-0.5">
    <span class="flex min-w-0 items-center gap-1.5">
      <span class="truncate font-medium text-fg" title={server.name}>{server.name}</span>
      <ServerFlags password={server.password} firstPerson={server.first_person_only} battleye={server.battl_eye} />
    </span>
    <span class="flex min-w-0 items-center gap-2">
      <Copy text={address} title={$c.copyIp({ address }).value} />
      <span class="truncate font-mono text-3xs text-fg-faint">{server.version}</span>
    </span>
  </span>
  <span class="truncate text-map" title={server.map}>{server.map}</span>
  <TimeOfDay time={server.time} />
  <ModsCount count={server.mods_count} onclick={onmods} />
  <OsIcon environment={server.environment} />
</div>

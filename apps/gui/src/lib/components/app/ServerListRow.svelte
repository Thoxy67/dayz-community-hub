<script lang="ts">
  import type { Snippet } from "svelte";
  import { dict } from "$lib/i18n";
  import Link from "~icons/lucide/link";
  import { Copy } from "$lib/components/ui/copy";
  import { IconButton } from "$lib/components/ui/button";
  import { connect } from "$lib/stores/connect.svelte";
  import { cn } from "$lib/cx";
  import FavoriteButton from "./FavoriteButton.svelte";
  import JoinButton from "./JoinButton.svelte";
  import ModsCount from "./ModsCount.svelte";
  import OsIcon from "./OsIcon.svelte";
  import PingButton from "./PingButton.svelte";
  import PlayersButton from "./PlayersButton.svelte";
  import ServerFlags from "./ServerFlags.svelte";
  import TimeOfDay from "./TimeOfDay.svelte";
  import { LIST_GRID, LIST_GRID_EXTRA, LIST_NARROW_HIDDEN } from "./server-list";

  /**
   * One server in any list: the browser, favourites, history. The same cells
   * in the same places everywhere; a list adds its own column (`extra`) and
   * its own hover actions (`actions`), which sit in a fixed-width column so
   * they never cover the mods or the date.
   */
  let {
    ip,
    queryPort,
    gamePort,
    joinPort,
    name,
    map = null,
    time = null,
    version = null,
    environment = null,
    modsCount = 0,
    password = false,
    firstPerson = false,
    battleye = null,
    official = false,
    mimicsOfficial = false,
    savedPassword = null,
    listed = true,
    excluded = false,
    selected = false,
    wide = false,
    rowindex,
    tag,
    extra,
    actions,
    onselect,
    onjoin,
    onmods,
  }: {
    ip: string;
    queryPort: number;
    /** The port players connect to; the address shown and copied. */
    gamePort: number;
    /** The port joining goes through (favourites keep the query port). */
    joinPort?: number;
    name: string;
    map?: string | null;
    time?: string | null;
    version?: string | null;
    environment?: string | null;
    modsCount?: number;
    password?: boolean;
    firstPerson?: boolean;
    battleye?: boolean | null;
    official?: boolean;
    /** A community server named like an official one. */
    mimicsOfficial?: boolean;
    /** A password saved with the favourite: joining uses it. */
    savedPassword?: string | null;
    /** In the current list; an unlisted server is shown dimmed but joinable. */
    listed?: boolean;
    excluded?: boolean;
    selected?: boolean;
    /** The list has its own column before the actions (LIST_GRID_EXTRA), filled by `extra`. */
    wide?: boolean;
    rowindex?: number;
    /** After the flags: "not in the list", and the like. */
    tag?: Snippet;
    /** The list's own column, before the actions (with `wide`). */
    extra?: Snippet;
    /** Shown on hover and on the selected row, after "copy the link" and left of Join. */
    actions?: Snippet;
    onselect: () => void;
    onjoin: () => void;
    onmods?: () => void;
  } = $props();

  const sv = dict("servers");
  const address = $derived(`${ip}:${gamePort}`);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  role="row"
  tabindex="-1"
  aria-selected={selected}
  aria-rowindex={rowindex}
  class={cn(
    wide ? LIST_GRID_EXTRA : LIST_GRID,
    "group relative h-full cursor-pointer border-b border-border/50 pr-2 pl-2 text-xs transition-colors",
    selected ? "bg-accent/10" : "hover:bg-raised/50",
    (!listed || excluded) && "opacity-55",
  )}
  onclick={onselect}
  ondblclick={onjoin}
>
  {#if selected}<span class="absolute inset-y-0 left-0 w-0.5 bg-accent"></span>{/if}

  <FavoriteButton {name} {ip} port={joinPort ?? queryPort} />
  <PingButton {ip} {queryPort} />
  <PlayersButton {ip} {queryPort} />

  <div class="flex min-w-0 flex-col gap-0.5">
    <div class="flex min-w-0 items-center gap-1.5">
      <span class="truncate font-semibold text-fg" title={name}>{name}</span>
      <ServerFlags
        {password}
        {firstPerson}
        {battleye}
        {official}
        {mimicsOfficial}
        savedPassword={!!savedPassword}
      />
      {#if tag}{@render tag()}{/if}
    </div>
    <div class="flex min-w-0 items-center gap-2 text-fg-faint">
      <Copy class="shrink-0" text={address} title={$sv.copyIp({ address }).value} />
      {#if version}<span class="min-w-0 truncate font-mono text-3xs">{version}</span>{/if}
      {#if environment}<OsIcon {environment} class="size-3" />{/if}
    </div>
  </div>

  <div class={cn("flex min-w-0 flex-col gap-0.5", LIST_NARROW_HIDDEN)}>
    {#if map}
      <span class="truncate text-map" title={map}>{map}</span>
      {#if time}<TimeOfDay {time} class="text-3xs" />{/if}
    {:else}
      <span class="text-fg-faint/60">—</span>
    {/if}
  </div>

  <ModsCount count={modsCount} onclick={onmods} />

  {#if wide}<div class="min-w-0">{@render extra?.()}</div>{/if}

  <div class="flex items-center justify-end gap-0.5">
    <div
      class={cn(
        "flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100",
        selected && "opacity-100",
      )}
    >
      <IconButton
        icon={Link}
        size="icon-xs"
        label={$sv.copyLink.value}
        kbd="L"
        onclick={(e) => {
          e.stopPropagation();
          void connect.copyLink({ ip, gamePort, queryPort, name });
        }}
      />
      {@render actions?.()}
    </div>
    <JoinButton {ip} port={joinPort ?? queryPort} password={savedPassword} compact />
  </div>
</div>

<script lang="ts" module>
  export type ModRef = { id: number; name: string };
</script>

<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import ExternalLink from "~icons/lucide/external-link";
  import Download from "~icons/lucide/download";
  import { Copy } from "$lib/components/ui/copy";
  import { cn } from "$lib/cx";
  import { mods } from "$lib/stores/mods.svelte";
  import { openUrl } from "$lib/ipc/native";
  import { bytes } from "$lib/format";

  /**
   * Workshop mods against what is installed here: each says whether it is on
   * disk, missing, or behind the Workshop, with its size, its id to copy and
   * its Workshop page. Missing first, then stale, then the rest by name, with
   * a summary line on top. For a server's mods, a .dzch file's, a queue's.
   */
  let {
    items,
    summary = true,
    class: klass = "",
  }: { items: readonly ModRef[]; summary?: boolean; class?: string } = $props();
  const c = useIntlayer("detail");

  const rows = $derived(
    items.map((m) => {
      const have = mods.byId.get(m.id);
      return {
        id: m.id,
        name: m.name || have?.name || `Workshop ${m.id}`,
        state: !have
          ? ("missing" as const)
          : have.update_available
            ? ("stale" as const)
            : ("ok" as const),
        size: have?.size ?? 0,
      };
    }),
  );
  const order = { missing: 0, stale: 1, ok: 2 } as const;
  const sorted = $derived(
    [...rows].sort((a, b) => order[a.state] - order[b.state] || a.name.localeCompare(b.name)),
  );
  const totals = $derived({
    installed: rows.filter((r) => r.state !== "missing").length,
    missing: rows.filter((r) => r.state === "missing").length,
    stale: rows.filter((r) => r.state === "stale").length,
    size: rows.reduce((a, r) => a + r.size, 0),
  });
  const workshop = (id: number) => `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`;
</script>

<div class={cn("flex flex-col gap-1.5", klass)}>
  {#if summary}
    <div class="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-2xs">
      <span class="text-fg-muted">
        {$c.modsSummary({
          installed: totals.installed,
          missing: totals.missing,
          stale: totals.stale,
        }).value}
      </span>
      {#if totals.size > 0}
        <span class="font-mono text-fg-faint"
          >{$c.modsLocalSize({ size: bytes(totals.size) }).value}</span
        >
      {/if}
    </div>
  {/if}
  <ul
    class="m-0 flex max-h-72 list-none flex-col overflow-y-auto rounded-sm border border-border bg-bg p-0"
  >
    {#each sorted as m (m.id)}
      <li class="group flex h-7 items-center gap-2 border-b border-border/50 px-2 last:border-b-0">
        <span
          class={cn(
            "w-[4.75rem] shrink-0 truncate rounded-xs px-1 text-center font-mono text-3xs uppercase",
            m.state === "missing" && "bg-err/12 text-err",
            m.state === "stale" && "bg-warn/12 text-warn",
            m.state === "ok" && "bg-ok/12 text-ok",
          )}
        >
          {m.state === "missing"
            ? $c.modMissing.value
            : m.state === "stale"
              ? $c.modStale.value
              : $c.modInstalled.value}
        </span>
        <span class="min-w-0 flex-1 truncate text-2xs text-fg" title={m.name}>{m.name}</span>
        {#if m.size > 0}<span class="font-mono text-3xs text-fg-faint">{bytes(m.size)}</span>{/if}
        {#if m.state === "missing"}<Download class="size-3 shrink-0 text-err/70" />{/if}
        <Copy
          text={String(m.id)}
          title={$c.copyId.value}
          class="text-3xs opacity-60 group-hover:opacity-100"
        />
        <button
          class="grid size-5 shrink-0 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-accent"
          title={$c.openWorkshop.value}
          aria-label={$c.openWorkshop.value}
          onclick={() => openUrl(workshop(m.id))}><ExternalLink class="size-3" /></button
        >
      </li>
    {/each}
  </ul>
</div>

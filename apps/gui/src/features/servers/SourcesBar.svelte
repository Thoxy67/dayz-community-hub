<script lang="ts">
  import { onMount } from "svelte";
  import { dict } from "$lib/i18n";
  import Info from "~icons/lucide/info";
  import { Meter } from "$lib/components/ui/meter";
  import { Popover } from "$lib/components/ui/popover";
  import { Spinner } from "$lib/components/ui/spinner";
  import type { Phase } from "$lib/ipc/bindings";
  import { sources } from "$lib/stores/sources.svelte";
  import { num, relative } from "$lib/format";
  import { cn } from "$lib/cx";

  /**
   * Under the server list's header: each source of the list, what it has
   * brought, and how far it has got. The breakdown opens on Details.
   */
  const w = dict("sources");

  onMount(() => void sources.start());

  // A clock for the throttle countdown and the elapsed time.
  let clock = $state(Math.floor(Date.now() / 1000));
  onMount(() => {
    const t = setInterval(() => (clock = Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  });

  const s = $derived(sources.now);
  const steam = $derived(s?.steam);
  const phaseWord = (p: Phase) =>
    ({
      idle: $w.idle.value,
      running: $w.running.value,
      done: $w.done.value,
      failed: $w.failed.value,
      off: $w.off.value,
    })[p];
  const dot = (p: Phase) =>
    ({
      idle: "bg-fg-faint/50",
      running: "bg-accent animate-pulse",
      done: "bg-ok",
      failed: "bg-err",
      off: "bg-fg-faint/30",
    })[p];
  const via = $derived(
    steam?.via === "web_api" ? $w.viaWeb.value : steam?.via ? $w.viaClient.value : "",
  );
  const waitLeft = $derived(steam?.waiting_until ? Math.max(0, steam.waiting_until - clock) : null);
  const took = $derived(
    steam?.started ? Math.max(0, (steam.finished ?? clock) - steam.started) : null,
  );
  const secs = (n: number) => (n < 60 ? `${n} s` : `${Math.floor(n / 60)} min ${n % 60} s`);
</script>

{#snippet cell(label: string, phase: Phase, title: string)}
  <span class="flex shrink-0 items-center gap-1.5" {title}>
    <span class={cn("size-1.5 rounded-full", dot(phase))}></span>
    <span class="label-stencil text-fg-muted">{label}</span>
  </span>
{/snippet}

{#snippet line(label: string, value: string, tone = "text-fg")}
  <div class="flex items-baseline justify-between gap-4 py-0.5">
    <span class="text-fg-muted">{label}</span>
    <span class={cn("num font-mono", tone)}>{value}</span>
  </div>
{/snippet}

{#if s}
  <div
    class="flex min-w-0 shrink-0 items-center gap-x-5 gap-y-1 overflow-hidden border-b border-border bg-bg/30 px-pad py-1 text-2xs"
  >
    <span class="flex shrink-0 items-center gap-2">
      {@render cell($w.dzsa.value, s.dzsa.phase, $w.dzsaHint.value)}
      {#if s.dzsa.phase === "running"}<Spinner class="size-3" />{/if}
      <span class="num font-mono text-fg">{num(s.dzsa.count)}</span>
    </span>

    {#if steam}
      <span class="flex min-w-0 items-center gap-2">
        {@render cell($w.steam.value, steam.phase, $w.steamHint.value)}
        {#if via}<span class="shrink-0 text-fg-faint">{via}</span>{/if}
        {#if steam.phase === "running" && steam.requests_total > 0}
          <Meter
            value={steam.requests_done}
            max={steam.requests_total}
            size="xs"
            class="w-20 shrink-0"
            label={$w.requests({ done: steam.requests_done, total: steam.requests_total }).value}
          />
          <span class="shrink-0 font-mono text-fg-muted">
            {steam.requests_done}/{steam.requests_total}
          </span>
        {:else if steam.phase !== "running"}
          <span class="shrink-0 text-fg-faint">{phaseWord(steam.phase)}</span>
        {/if}
        {#if waitLeft != null}
          <span class="truncate text-warn">{$w.waiting({ secs: waitLeft }).value}</span>
        {:else if steam.phase !== "off"}
          <span class="truncate text-fg-muted">
            <span class="num font-mono text-ok">+{num(steam.added)}</span>
            · {num(steam.answered)}/{num(steam.queried)} A2S · {num(steam.listed)}
            {$w.listed.value.toLowerCase()}
          </span>
        {/if}
      </span>
    {/if}

    {#if s.kept.total > 0}
      <span class="flex shrink-0 items-center gap-2">
        {@render cell($w.kept.value, s.kept.phase, $w.keptHint.value)}
        <span class="num font-mono text-fg">{num(s.kept.alive || s.kept.total)}</span>
        {#if s.kept.removed}
          <span class="font-mono text-err">−{num(s.kept.removed)}</span>
        {/if}
      </span>
    {/if}

    <span class="ml-auto flex shrink-0 items-center gap-2">
      <span class="text-fg-muted">{$w.inList.value}</span>
      <span class="num font-mono text-fg">{num(s.total)}</span>
      <span class="text-fg-faint">· {$w.official({ count: num(s.official) }).value}</span>
      {#if s.duplicates}
        <span class="text-fg-faint" title={$w.duplicatesHint.value}>
          · {$w.duplicates({ count: num(s.duplicates) }).value}
        </span>
      {/if}
      <Popover
        label={$w.title.value}
        triggerClass="flex items-center gap-1 rounded-xs px-1 text-fg-muted hover:bg-raised hover:text-fg"
        panelClass="w-80 p-3 text-2xs"
      >
        {#snippet trigger()}<Info class="size-3" />{$w.details.value}{/snippet}
        <p class="m-0 mb-2 font-display text-sm text-fg">{$w.title.value}</p>

        <p class="m-0 mt-1 label-stencil text-fg-muted">
          {$w.dzsa.value} · {phaseWord(s.dzsa.phase)}
        </p>
        <p class="m-0 mb-1 text-fg-faint">{$w.dzsaHint.value}</p>
        {@render line($w.inList.value, num(s.dzsa.count))}
        {#if s.dzsa.at}{@render line($w.fetched.value, relative(s.dzsa.at))}{/if}
        {#if s.dzsa.error}<p class="m-0 text-err" data-selectable>{s.dzsa.error}</p>{/if}

        {#if steam}
          <p class="m-0 mt-3 label-stencil text-fg-muted">
            {$w.steam.value}{via ? ` · ${via}` : ""} · {phaseWord(steam.phase)}
          </p>
          <p class="m-0 mb-1 text-fg-faint">{$w.steamHint.value}</p>
          {#if steam.note}<p class="m-0 mb-1 text-warn" data-selectable>{steam.note}</p>{/if}
          {@render line($w.requestsLabel.value, `${steam.requests_done}/${steam.requests_total}`)}
          {#if steam.client_assists}
            {@render line($w.clientAssists.value, num(steam.client_assists), "text-fg-muted")}
          {/if}
          {#if steam.current && steam.phase === "running"}
            {@render line($w.now.value, steam.current, "text-fg-muted")}
          {/if}
          {@render line($w.listed.value, num(steam.listed))}
          {@render line($w.farms.value, num(steam.farms), "text-fg-faint")}
          {@render line($w.known.value, num(steam.known), "text-fg-faint")}
          {@render line($w.queried.value, num(steam.queried))}
          {@render line($w.answered.value, num(steam.answered))}
          {@render line($w.retried.value, num(steam.retried), "text-fg-muted")}
          {@render line($w.noMods.value, num(steam.no_mods), "text-fg-faint")}
          {@render line($w.fakes.value, num(steam.fakes), "text-fg-faint")}
          {@render line($w.added.value, `+${num(steam.added)}`, "text-ok")}
          {#if took != null}{@render line($w.elapsed.value, secs(took))}{/if}
        {/if}

        {#if s.duplicates}
          <p class="m-0 mt-3 text-fg-muted">
            {$w.duplicates({ count: num(s.duplicates) }).value}
          </p>
          <p class="m-0 mb-1 text-fg-faint">{$w.duplicatesHint.value}</p>
        {/if}

        {#if s.kept.total > 0}
          <p class="m-0 mt-3 label-stencil text-fg-muted">
            {$w.kept.value} · {phaseWord(s.kept.phase)}
          </p>
          <p class="m-0 mb-1 text-fg-faint">{$w.keptHint.value}</p>
          {@render line($w.checked.value, `${num(s.kept.checked)}/${num(s.kept.total)}`)}
          {@render line($w.alive.value, num(s.kept.alive), "text-ok")}
          {@render line($w.removed.value, num(s.kept.removed), "text-err")}
          {@render line($w.fakes.value, num(s.kept.fakes), "text-err")}
        {/if}
      </Popover>
    </span>
  </div>
{/if}

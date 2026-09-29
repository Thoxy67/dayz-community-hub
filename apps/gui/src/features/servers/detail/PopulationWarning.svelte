<script lang="ts">
  import { dict } from "$lib/i18n";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import ShieldAlert from "~icons/lucide/shield-alert";
  import { cn } from "$lib/cx";
  import type { DetailModel } from "./model.svelte";

  /**
   * DayZ Metrics' verdict on the player count, when it is not clean: shown
   * where a player decides to join (the hero, the overview, the stats), since
   * a padded count is exactly what would mislead that decision.
   */
  let { m, compact = false }: { m: DetailModel; compact?: boolean } = $props();
  const c = dict("detail");
  const x = $derived(m.metrics);
  const reasons = $derived(
    [
      ...(x?.fake_reasons ?? []),
      ...(x?.flagged ? [$c.dmFlagged.value] : []),
      ...(x?.mimics_official ? [$c.dmMimics.value] : []),
    ].filter(Boolean),
  );
</script>

{#if m.population === "fake" || m.population === "suspect"}
  {@const fake = m.population === "fake"}
  <div
    role="status"
    class={cn(
      "flex items-start gap-2 rounded-sm border text-2xs leading-snug",
      compact ? "px-2 py-1" : "px-2.5 py-2",
      fake ? "border-err/40 bg-err/10 text-err" : "border-warn/40 bg-warn/10 text-warn",
    )}
  >
    {#if fake}<ShieldAlert class="mt-px size-3.5 shrink-0" />{:else}<TriangleAlert
        class="mt-px size-3.5 shrink-0"
      />{/if}
    <div class="min-w-0 flex-1">
      <p class="m-0 font-semibold">{fake ? $c.dmFake.value : $c.dmSuspect.value}</p>
      {#if !compact}
        {#if fake}<p class="m-0 mt-0.5 text-fg-muted">{$c.dmFakeHint.value}</p>{/if}
        {#if reasons.length}
          <ul class="m-0 mt-1 list-disc pl-4 text-fg-muted">
            {#each reasons as r, i (i)}<li>{r}</li>{/each}
          </ul>
        {/if}
      {/if}
    </div>
  </div>
{/if}

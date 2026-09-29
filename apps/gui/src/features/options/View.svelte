<script lang="ts">
  import { dict } from "$lib/i18n";
  import Wand from "~icons/lucide/wand-sparkles";
  import Check from "~icons/lucide/check";
  import Cpu from "~icons/lucide/cpu";
  import SearchX from "~icons/lucide/search-x";
  import { PageHeader, Figure, Empty } from "$lib/components/app";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import CommandPreview from "$lib/components/app/CommandPreview.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Chip } from "$lib/components/ui/chip";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { getSystemSpecs } from "$lib/ipc/system";
  import type { LaunchOptionDto, SystemSpecsDto } from "$lib/ipc/types";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import { GROUPS, META, OTHER, flagText, recommend, wordOf, type Group } from "./catalog";
  import OptionRow from "./OptionRow.svelte";

  const o = dict("options");

  let search = $state("");
  let activeOnly = $state(false);

  // ── the hardware, and what it suggests ──────────────────────────────────
  let specs = $state<SystemSpecsDto | null>(null);
  let specsLoading = $state(true);
  getSystemSpecs()
    .then((s) => (specs = s))
    .catch(() => {})
    .finally(() => (specsLoading = false));

  const recs = $derived(specs ? recommend(specs) : []);
  const recValue = $derived(new Map(recs.filter((r) => r.value).map((r) => [r.key, r.value!])));

  const options = $derived(profile.data?.options ?? []);
  const byKey = $derived(new Map(options.map((x) => [x.key, x])));

  /** Every recommendation already in place. */
  const recsApplied = $derived(
    recs.length > 0 &&
      recs.every((r) => {
        const x = byKey.get(r.key);
        return !!x && x.enabled && (r.value === undefined || x.value === r.value);
      }),
  );

  async function applyRecommended() {
    for (const r of recs) {
      const x = byKey.get(r.key);
      if (!x) continue;
      if (r.value !== undefined) {
        // Setting a value also turns the option on.
        if (x.value !== r.value || !x.enabled) await profile.setOptionValue(r.key, r.value);
      } else if (!x.enabled) {
        await profile.toggleOption(r.key);
      }
    }
  }

  // ── what is shown ───────────────────────────────────────────────────────
  function matches(x: LaunchOptionDto, q: string): boolean {
    if (activeOnly && !x.enabled) return false;
    if (!q) return true;
    const m = META[x.key];
    const hay = [x.key, x.description, m?.flag ?? "", m ? wordOf($o, m.label) : "", m ? wordOf($o, m.desc) : ""];
    return hay.some((h) => h.toLowerCase().includes(q));
  }

  const groups = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const listed = new Set(GROUPS.flatMap((g) => g.keys));
    const other: Group = { ...OTHER, keys: options.filter((x) => !listed.has(x.key)).map((x) => x.key) };
    return [...GROUPS, other]
      .map((g) => ({
        g,
        opts: g.keys.map((k) => byKey.get(k)).filter((x): x is LaunchOptionDto => !!x && matches(x, q)),
        all: g.keys.map((k) => byKey.get(k)).filter((x): x is LaunchOptionDto => !!x),
      }))
      .filter((e) => e.opts.length > 0);
  });

  const enabled = $derived(options.filter((x) => x.enabled));
  const withValue = $derived(enabled.filter((x) => x.value));
  const command = $derived(enabled.map((x) => flagText(x.key, x.value)));

  const gb = (mb: number) => (mb / 1024).toFixed(mb >= 10240 ? 0 : 1);
</script>

<PageHeader title={$o.title.value}>
  {#snippet stats()}
    <Figure label={$o.statActive.value} value={String(enabled.length)} tone="text-accent" />
    <Figure label={$o.statWithValue.value} value={String(withValue.length)} />
    <Figure label={$o.statAvailable.value} value={String(options.length)} tone="text-fg-muted" />
  {/snippet}
  {#snippet actions()}
    <Chip active={activeOnly} onclick={() => (activeOnly = !activeOnly)}>{$o.showActiveOnly.value}</Chip>
    <Input type="search" class="w-60" placeholder={$o.searchPlaceholder.value} bind:value={search} />
  {/snippet}
</PageHeader>

<div class="flex min-h-0 flex-1">
  <!-- The options, by what they change. -->
  <div class="min-w-0 flex-1 overflow-y-auto p-3">
    {#if groups.length === 0}
      <Empty icon={SearchX} title={$o.noMatch({ search }).value} />
    {:else}
      <div class="grid grid-cols-1 gap-3 xl:grid-cols-2">
        {#each groups as { g, opts, all } (g.id)}
          <SectionCard title={wordOf($o, g.label)} icon={g.icon} tone={g.tone} class="self-start">
            {#snippet actions()}
              <span class="font-mono text-3xs text-fg-faint">
                {$o.enabledCount({ enabled: all.filter((x) => x.enabled).length, total: all.length }).value}
              </span>
            {/snippet}
            <div class="divide-y divide-border/50">
              {#each opts as opt (opt.key)}
                <OptionRow {opt} tone={g.tone} recommended={recValue.get(opt.key)} />
              {/each}
            </div>
          </SectionCard>
        {/each}
      </div>
    {/if}
  </div>

  <!-- What the machine suggests, and what DayZ will be told. -->
  <aside class="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border bg-bg/40 p-3">
    <SectionCard title={$o.hardware.value} icon={Cpu}>
      {#if specsLoading}
        <div class="flex items-center gap-2 px-3 py-3 text-2xs text-fg-faint">
          <Spinner class="size-icon-sm" />{$o.recommendDetecting.value}
        </div>
      {:else if specs}
        <dl class="m-0 grid grid-cols-3 divide-x divide-border/60 border-b border-border">
          {#each [[$o.cores.value, String(specs.physical_cores)], [$o.threads.value, String(specs.logical_cores)], [$o.memory.value, `${gb(specs.total_memory_mb)} GB`]] as [k, v] (k)}
            <div class="px-3 py-2">
              <dd class="m-0 font-display text-xl font-extrabold text-fg">{v}</dd>
              <dt class="text-3xs text-fg-faint uppercase">{k}</dt>
            </div>
          {/each}
        </dl>
        <div class="space-y-2 px-3 py-2.5">
          <p class="m-0 text-2xs leading-snug text-fg-muted">
            {$o.recommendDesc({ cores: specs.physical_cores, ram: Math.round(specs.total_memory_mb / 1024) }).value}
          </p>
          <ul class="m-0 flex list-none flex-wrap gap-1 p-0">
            {#each recs as r (r.key)}
              {@const x = byKey.get(r.key)}
              {@const done = !!x?.enabled && (r.value === undefined || x.value === r.value)}
              <li>
                <code
                  class={cn(
                    "inline-flex items-center gap-1 rounded-xs px-1.5 py-px font-mono text-3xs",
                    done ? "bg-ok/12 text-ok" : "bg-raised text-fg-muted",
                  )}
                >
                  {#if done}<Check class="size-3" />{/if}{flagText(r.key, r.value ?? null)}
                </code>
              </li>
            {/each}
          </ul>
          <p class="m-0 text-3xs leading-snug text-fg-faint">{$o.recommendedHint.value}</p>
          <Button
            variant={recsApplied ? "default" : "accent"}
            class="w-full"
            disabled={recsApplied}
            onclick={applyRecommended}
          >
            {#if recsApplied}<Check class="size-icon-sm" />{$o.recommendApplied.value}{:else}<Wand
                class="size-icon-sm"
              />{$o.recommendApply.value}{/if}
          </Button>
        </div>
      {/if}
    </SectionCard>

    <CommandPreview
      title={$o.commandLine.value}
      program="DayZ_x64"
      flags={command}
      empty={$o.commandLineEmpty.value}
      hint={$o.commandLineHint.value}
      copyLabel={$o.copyCommand.value}
    />
  </aside>
</div>

<script lang="ts" generics="T extends string | number">
  import { ToggleGroup } from "bits-ui";
  import { cn } from "$lib/cx";

  /**
   * A short scale of things that are each on or off, as one strip of cells:
   * the tiers, the enchantments. bits-ui's ToggleGroup, so each cell says
   * whether it is pressed and the arrows move along the strip.
   *
   * One strip rather than a scatter of chips, because the members are a
   * scale: T4 sits between T3 and T5, and a run of them reads as a range
   * ("T4 to T8") at a glance, which eight separate buttons wrapping onto two
   * lines did not. A cell with a colour of its own wears it when it is on, so
   * a heat scale looks like one.
   */
  type Cell = { value: T; label: string; color?: string; title?: string };

  let {
    cells,
    on,
    onchange,
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    cells: readonly Cell[];
    /** The values that are on. */
    on: readonly T[];
    onchange: (value: T, on: boolean) => void;
    class?: string;
    "aria-label": string;
  } = $props();

  // bits-ui speaks strings; the tiers are numbers.
  const key = (v: T) => String(v);
  function changed(next: string[]) {
    for (const c of cells) {
      const now = next.includes(key(c.value));
      if (now !== on.includes(c.value)) onchange(c.value, now);
    }
  }
</script>

<ToggleGroup.Root
  type="multiple"
  bind:value={() => on.map(key), changed}
  aria-label={ariaLabel}
  class={cn("flex w-full overflow-hidden rounded-sm border border-border bg-bg", klass)}
>
  {#each cells as c (c.value)}
    <ToggleGroup.Item
      value={key(c.value)}
      title={c.title}
      aria-label={c.title ?? c.label}
      style={c.color ? `--cell: ${c.color}` : undefined}
      class={cn(
        "relative flex h-control-sm min-w-0 flex-1 items-center justify-center border-r border-border font-mono text-2xs",
        "text-fg-faint last:border-r-0 hover:bg-raised hover:text-fg",
        "focus-visible:ring-1 focus-visible:ring-accent focus-visible:outline-none focus-visible:ring-inset",
        c.color
          ? "data-[state=on]:bg-[color-mix(in_oklch,var(--cell)_18%,transparent)] data-[state=on]:text-(--cell)"
          : "data-[state=on]:bg-accent/15 data-[state=on]:text-accent",
      )}
    >
      {c.label}
      <!-- The colour is on the cell whether it is on or off, as a line under
           it, so the scale can be read before anything is chosen. -->
      {#if c.color}
        <span class="absolute inset-x-1 bottom-0 h-0.5 rounded-full bg-(--cell) opacity-70"></span>
      {/if}
    </ToggleGroup.Item>
  {/each}
</ToggleGroup.Root>

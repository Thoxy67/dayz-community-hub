<script lang="ts" module>
  export type Fact = { label: string; value: string; tone?: string; title?: string };
</script>

<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * Labelled figures in a tight grid: the label faint on the left, the value
   * in mono on the right. Two pairs per line by default, which is how a
   * narrow panel fits a dozen facts without scrolling.
   */
  let {
    items,
    columns = 2,
    class: klass = "",
  }: { items: readonly Fact[]; columns?: 1 | 2; class?: string } = $props();
</script>

<dl
  class={cn(
    "m-0 grid gap-x-3 gap-y-1 text-2xs",
    columns === 2 ? "grid-cols-[1fr_auto_1fr_auto]" : "grid-cols-[1fr_auto]",
    klass,
  )}
>
  {#each items as f (f.label)}
    <dt class="truncate text-fg-faint">{f.label}</dt>
    <dd
      class={cn("m-0 truncate text-right font-mono", f.tone ?? "text-fg")}
      title={f.title || undefined}
    >
      {f.value}
    </dd>
  {/each}
</dl>

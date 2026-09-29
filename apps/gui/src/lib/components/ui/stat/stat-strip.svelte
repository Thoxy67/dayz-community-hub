<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  // A session's headline numbers, as one strip of tiles.
  //
  // The meter views (gathering, loot, combat) each grew their own `dl` for
  // this and drifted: same facts, three layouts. One strip keeps them reading
  // alike -- value over label, hairlines between -- so a number's place and
  // shape mean the same thing whichever tab it is read on.
  let {
    class: klass = "",
    children,
  }: {
    class?: string;
    children: Snippet;
  } = $props();
</script>

<dl
  class={cn(
    "flex shrink-0 flex-wrap items-stretch border-b border-border text-2xs",
    "[&>*]:border-r [&>*]:border-border/50 [&>*:last-child]:border-r-0",
    klass,
  )}
>
  {@render children()}
</dl>

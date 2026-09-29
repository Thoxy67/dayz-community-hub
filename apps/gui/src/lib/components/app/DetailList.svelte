<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /** Facts about one thing, label beside value: fill it with `DetailRow`s. */
  let { class: klass = "", children }: { class?: string; children: Snippet } = $props();
</script>

<dl
  class={cn(
    "m-0 grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-x-4 gap-y-2 text-xs",
    klass,
  )}
>
  {@render children()}
</dl>

<script lang="ts">
  let { children }: { children: import("svelte").Snippet } = $props();
</script>

<kbd
  class="rounded-xs border border-border bg-bg px-1 font-mono text-3xs leading-3.5
         text-fg-faint shadow-[0_1px_0_rgb(0_0_0/0.35),inset_0_1px_0_rgb(255_255_255/0.04)]"
>
  {@render children()}
</kbd>

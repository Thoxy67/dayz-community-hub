<script lang="ts">
  import type { Snippet } from "svelte";
  let { children }: { children: Snippet } = $props();
</script>

<div class="flex min-w-0 items-center gap-1.5 px-1.5 pt-1 pb-1.5 text-2xs text-fg-faint">
  {@render children()}
</div>

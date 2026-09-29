<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Puzzle from "~icons/lucide/puzzle";

  /** How many mods a server runs; a button when there is somewhere to show them. */
  let { count, onclick }: { count: number; onclick?: () => void } = $props();
  const c = useIntlayer("servers");
</script>

{#if count > 0}
  <button
    class="flex items-center gap-1 justify-self-start rounded-xs px-1 font-mono text-2xs text-mods hover:bg-mods/10 disabled:hover:bg-transparent"
    title={$c.showMods.value}
    disabled={!onclick}
    onclick={(e) => {
      e.stopPropagation();
      onclick?.();
    }}
  >
    <Puzzle class="size-3" /><span class="num">{count}</span>
  </button>
{:else}
  <span class="text-fg-faint/60">—</span>
{/if}

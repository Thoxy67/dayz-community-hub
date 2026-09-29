<script lang="ts">
  import X from "~icons/lucide/x";

  /** A picture over everything, closed by a click anywhere or Escape. `src` null is closed. */
  let { src = $bindable(null), closeLabel }: { src?: string | null; closeLabel: string } = $props();
</script>

<svelte:window onkeydown={(e) => src && e.key === "Escape" && (src = null)} />

{#if src}
  <div
    class="fixed inset-0 z-dialog grid animate-fade-in place-items-center bg-plot/90 p-8"
    role="dialog"
    aria-modal="true"
    aria-label={closeLabel}
    tabindex="-1"
    onclick={() => (src = null)}
    onkeydown={(e) => e.key === "Escape" && (src = null)}
  >
    <img {src} alt="" class="max-h-full max-w-full rounded-md object-contain shadow-pop" />
    <button
      class="absolute top-4 right-4 grid size-control-lg place-items-center rounded-full bg-raised text-fg-muted hover:text-fg"
      aria-label={closeLabel}
      onclick={() => (src = null)}><X class="size-icon" /></button
    >
  </div>
{/if}

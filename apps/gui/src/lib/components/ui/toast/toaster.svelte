<script lang="ts">
  import Check from "~icons/lucide/check";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import CircleX from "~icons/lucide/circle-x";
  import Info from "~icons/lucide/info";
  import X from "~icons/lucide/x";
  import { toasts } from "./toasts.svelte";

  // Where the toasts are drawn: once, by the window. `role="status"` on the
  // region, so each is read out when it arrives without taking the focus.
  let { dismissLabel = "Dismiss" }: { dismissLabel?: string } = $props();

  const LOOK = {
    ok: { icon: Check, ink: "text-ok", edge: "border-l-ok" },
    warn: { icon: TriangleAlert, ink: "text-warn", edge: "border-l-warn" },
    err: { icon: CircleX, ink: "text-err", edge: "border-l-err" },
    neutral: { icon: Info, ink: "text-fg-muted", edge: "border-l-border-strong" },
  } as const;
</script>

<div
  role="status"
  aria-live="polite"
  class="pointer-events-none fixed right-3 bottom-9 z-toast flex w-80 max-w-[calc(100vw-1.5rem)] flex-col gap-1.5"
>
  {#each toasts.list as t (t.id)}
    {@const look = LOOK[t.tone]}
    <div
      class="pointer-events-auto flex items-start gap-2 rounded-md border border-l-2 border-border
             bg-overlay py-1.5 pr-1.5 pl-2 text-xs text-fg shadow-pop {look.edge}"
    >
      <look.icon class="mt-0.5 size-icon shrink-0 {look.ink}" />
      <p class="m-0 min-w-0 flex-1 leading-snug select-text">{t.text}</p>
      {#if t.action}
        <button
          type="button"
          class="shrink-0 rounded-sm px-1.5 text-2xs font-medium text-accent hover:bg-raised"
          onclick={() => {
            t.action?.run();
            toasts.dismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button
        type="button"
        aria-label={dismissLabel}
        class="grid size-4 shrink-0 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg"
        onclick={() => toasts.dismiss(t.id)}
      >
        <X class="size-icon-sm" />
      </button>
    </div>
  {/each}
</div>

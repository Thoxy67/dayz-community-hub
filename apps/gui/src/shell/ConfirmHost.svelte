<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Dialog } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /** Draws whatever `confirm()` is waiting on. The safe answer takes the focus. */
  const s = dict("shell");
  const p = $derived(dialogs.pending);
</script>

<Dialog
  bind:open={() => p !== null, (v) => !v && dialogs.answer(false)}
  title={p?.title ?? ""}
  closeLabel={p?.cancelLabel ?? $s.confirmCancel.value}
>
  <p class="m-0 text-xs leading-relaxed whitespace-pre-line text-fg-muted">{p?.message}</p>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => dialogs.answer(false)}
      >{p?.cancelLabel ?? $s.confirmCancel.value}</Button
    >
    <Button variant={p?.danger ? "danger" : "accent"} onclick={() => dialogs.answer(true)}>
      {p?.confirmLabel ?? $s.confirmConfirm.value}
    </Button>
  {/snippet}
</Dialog>

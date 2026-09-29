<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import Dialog from "./dialog.svelte";

  /**
   * "Are you sure", for something that cannot be taken back.
   *
   * The button that acts says what it does ("Delete route", not "OK"), and
   * the one that does nothing is first and is where the focus lands, so the
   * Enter of somebody who was not reading is the safe answer.
   */
  let {
    open = $bindable(false),
    title,
    description = "",
    confirm,
    cancel,
    danger = true,
    onconfirm,
  }: {
    open?: boolean;
    title: string;
    description?: string;
    /** The words on the button that acts. */
    confirm: string;
    cancel: string;
    danger?: boolean;
    onconfirm: () => void;
  } = $props();
</script>

<Dialog bind:open {title} {description} closeLabel={cancel}>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{cancel}</Button>
    <Button
      variant={danger ? "danger" : "accent"}
      onclick={() => {
        open = false;
        onconfirm();
      }}>{confirm}</Button
    >
  {/snippet}
</Dialog>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Dialog } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { SecretInput } from "$lib/components/app";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /** Draws whatever `askPassword()` is waiting on: the field takes the focus. */
  const c = dict("connect");
  const s = dict("shell");
  const p = $derived(dialogs.password);

  let value = $state("");
  let save = $state(true);
  // A fresh question starts empty.
  $effect(() => {
    if (p) {
      value = "";
      save = true;
    }
  });

  function submit(e?: SubmitEvent) {
    e?.preventDefault();
    if (value) dialogs.answerPassword({ password: value, save });
  }
</script>

<Dialog
  bind:open={() => p !== null, (v) => !v && dialogs.answerPassword(null)}
  title={$c.passwordAskTitle.value}
  closeLabel={$s.confirmCancel.value}
>
  <form id="password-ask" class="flex flex-col gap-3" onsubmit={submit}>
    <p class="m-0 text-xs leading-relaxed text-fg-muted">
      {$c.passwordAskMessage({ name: p?.server ?? "" }).value}
    </p>
    <div class="flex items-center gap-1">
      <SecretInput
        bind:value
        placeholder={$c.password.value}
        autocomplete="current-password"
      />
    </div>
    <label class="flex cursor-pointer items-center gap-2 text-xs text-fg-muted">
      <Checkbox bind:checked={save} aria-label={$c.passwordAskSave.value} />
      {$c.passwordAskSave.value}
    </label>
  </form>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => dialogs.answerPassword(null)}
      >{$s.confirmCancel.value}</Button
    >
    <Button variant="accent" disabled={!value} onclick={() => submit()}>
      {$c.passwordAskJoin.value}
    </Button>
  {/snippet}
</Dialog>

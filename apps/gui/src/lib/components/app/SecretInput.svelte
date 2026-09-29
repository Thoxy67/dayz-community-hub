<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Eye from "~icons/lucide/eye";
  import EyeOff from "~icons/lucide/eye-off";
  import { Input } from "$lib/components/ui/input";

  /** A key or a password: hidden until asked, one button to show it. Renders two
      children (the field and the eye), for a flex row such as a kit `Field`. */
  let {
    value = $bindable(""),
    id,
    placeholder = "",
    autocomplete = "off",
  }: { value: string; id?: string; placeholder?: string; autocomplete?: "off" | "current-password" } = $props();

  const s = useIntlayer("settings");
  let shown = $state(false);
</script>

<Input
  {id}
  bind:value
  type={shown ? "text" : "password"}
  {placeholder}
  {autocomplete}
  spellcheck={false}
  class="flex-1 font-mono"
/>
<button
  type="button"
  class="grid size-control shrink-0 place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
  aria-label={shown ? $s.hide.value : $s.show.value}
  title={shown ? $s.hide.value : $s.show.value}
  onclick={() => (shown = !shown)}
>
  {#if shown}<EyeOff class="size-icon-sm" />{:else}<Eye class="size-icon-sm" />{/if}
</button>

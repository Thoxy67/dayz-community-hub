<script lang="ts">
  import { Switch } from "bits-ui";
  import { cn } from "$lib/cx";

  // The switch alone, for a row that already says what it switches: a table's
  // "on" column, a card's corner. The same track and thumb as `Switch`; the
  // name goes to `aria-label` since there is no text beside it.
  let {
    checked = $bindable(),
    disabled = false,
    tone = "accent",
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    checked: boolean;
    disabled?: boolean;
    tone?: "accent" | "warn";
    class?: string;
    "aria-label": string;
  } = $props();
</script>

<Switch.Root
  bind:checked
  {disabled}
  aria-label={ariaLabel}
  class={cn(
    "relative inline-flex h-3.5 w-7 shrink-0 cursor-pointer items-center rounded-full border border-border bg-bg",
    "focus-visible:ring-1 focus-visible:ring-accent focus-visible:outline-none",
    "disabled:cursor-not-allowed disabled:opacity-50",
    tone === "warn" ? "data-[state=checked]:bg-warn" : "data-[state=checked]:bg-accent",
    klass,
  )}
>
  <Switch.Thumb
    class="block size-2.5 translate-x-[0.1875rem] rounded-full bg-fg-faint transition-transform duration-(--duration-fast)
           data-[state=checked]:translate-x-[0.9375rem] data-[state=checked]:bg-accent-fg"
  />
</Switch.Root>

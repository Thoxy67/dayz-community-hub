<script lang="ts">
  import { Switch } from "bits-ui";
  import { cn } from "$lib/cx";

  // A switch, not a checkbox: for the settings where flipping it *does*
  // something ("auto-finish is running"), rather than filtering a view.
  //
  // bits-ui's Switch, which is a `<button role="switch">` and nothing else as
  // long as it is given no `name`. **Never pass one.** With a name it renders
  // a visually hidden checkbox beside the button for form posts, and a clipped
  // checkbox under a label is exactly what made WebKitGTK's web process abort
  // on every flip and black the window out until a restart. This app posts no
  // forms, so there is nothing a name would be for.
  let {
    checked = $bindable(),
    label,
    hint,
    disabled = false,
    tone = "accent",
    class: klass = "",
  }: {
    checked: boolean;
    label: string;
    hint?: string;
    disabled?: boolean;
    /** `warn` for the ones that write to the game. */
    tone?: "accent" | "warn";
    class?: string;
  } = $props();
</script>

<Switch.Root
  bind:checked
  {disabled}
  class={cn(
    "group flex w-full items-center gap-2 px-2 py-1.5 text-left select-none",
    "focus-visible:ring-1 focus-visible:ring-accent focus-visible:outline-none",
    disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer hover:bg-raised",
    klass,
  )}
>
  <span
    class={cn(
      "relative h-3.5 w-7 shrink-0 rounded-full border border-border bg-bg",
      "group-data-[state=checked]:shadow-[var(--glow-accent-soft)]",
      tone === "warn"
        ? "group-data-[state=checked]:bg-warn"
        : "group-data-[state=checked]:bg-accent",
    )}
  >
    <!-- Moved with a transform, not `left`: the thumb sliding is the one
         animation here, and a transform is the kind that lays nothing out. -->
    <Switch.Thumb
      class="absolute top-1/2 left-[0.1875rem] block size-2.5 -translate-y-1/2 rounded-full bg-fg-faint
             transition-transform duration-(--duration-fast)
             data-[state=checked]:translate-x-[0.8125rem] data-[state=checked]:bg-accent-fg"
    />
  </span>
  <span class="min-w-0 flex-1">
    <span class="block truncate text-2xs text-fg-muted group-data-[state=checked]:text-fg"
      >{label}</span
    >
    {#if hint}
      <!-- Wrapped, not cut: the hint is the explanation, and a narrow window
           is where somebody most needs the end of the sentence. -->
      <span class="block text-2xs leading-snug text-fg-faint">{hint}</span>
    {/if}
  </span>
</Switch.Root>

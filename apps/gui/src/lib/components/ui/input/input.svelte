<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Search from "~icons/lucide/search";
  import X from "~icons/lucide/x";
  import { dict } from "$lib/i18n";
  import { cn } from "$lib/cx";

  /**
   * The one text field.
   *
   * There were nine of these written out by hand, in five heights, four radii
   * and on two grounds, so the search box of one list did not look like the
   * search box of the next. A field is as tall as a control (`sm`) or as a
   * control that sits inside a row (`xs`), always on the shell's ground so it
   * reads as a well cut into whatever panel it is in, and that is all there is
   * to choose.
   *
   * `type="search"` wears the glass and, once there is something in it, a
   * button that empties it: a filter nobody can see how to take off is the
   * commonest way a list comes to look broken.
   */
  let {
    value = $bindable(""),
    size = "sm",
    type = "text",
    class: klass = "",
    clearLabel,
    ...rest
  }: Omit<HTMLInputAttributes, "size" | "value"> & {
    value?: string | number | null;
    size?: "sm" | "xs";
    /** The accessible name of the button that empties a search field. */
    clearLabel?: string;
    class?: string;
  } = $props();

  const common = dict("common");
  const search = $derived(type === "search");
  const filled = $derived(value !== "" && value !== null && value !== undefined);
</script>

<span
  class={cn(
    "relative inline-flex min-w-0 items-center",
    size === "sm" ? "h-control text-xs" : "h-control-sm text-2xs",
    klass,
  )}
>
  {#if search}
    <Search class="pointer-events-none absolute left-1.5 size-icon-sm text-fg-faint" />
  {/if}
  <!-- `type="text"` for a search: the engine's own search field brings its
       own clear button and its own rounded well, and ignores the theme's. -->
  <input
    {...rest}
    type={search ? "text" : type}
    bind:value
    class={cn(
      "h-full w-full min-w-0 rounded-sm border border-border bg-bg text-fg",
      "placeholder:text-fg-faint hover:border-border-strong focus:border-accent/60 focus:outline-none",
      "disabled:cursor-not-allowed disabled:opacity-40",
      size === "sm" ? "px-2" : "px-1.5",
      search && (size === "sm" ? "pr-6 pl-6" : "pr-5 pl-5.5"),
      // The engine's own spinner is drawn in the platform's colours inside a
      // themed field; the arrow keys and the wheel still step the value.
      type === "number" &&
        "font-mono tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none",
    )}
  />
  {#if search && filled}
    <button
      type="button"
      aria-label={clearLabel ?? $common.clearSearch.value}
      class="absolute right-1 grid size-4 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg"
      onclick={() => (value = "")}
    >
      <X class="size-icon-sm" />
    </button>
  {/if}
</span>

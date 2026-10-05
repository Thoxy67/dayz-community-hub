<script lang="ts">
  import ArrowUp from "~icons/lucide/arrow-up";
  import ArrowDown from "~icons/lucide/arrow-down";
  import ArrowUpDown from "~icons/lucide/arrow-up-down";
  import { cn } from "$lib/cx";

  /** A column heading that sorts by it; the arrow says which way. */
  let {
    label,
    active,
    asc,
    title = "",
    class: klass = "",
    onclick,
  }: {
    label: string;
    active: boolean;
    asc: boolean;
    title?: string;
    class?: string;
    onclick: () => void;
  } = $props();
</script>

<!-- The sort order belongs to the column, not to the button that changes it:
     `aria-sort` is not allowed on a button. -->
<div
  role="columnheader"
  aria-sort={active ? (asc ? "ascending" : "descending") : "none"}
  class={cn("justify-self-start", klass)}
>
  <button
    class={cn("flex items-center gap-1 uppercase hover:text-fg", active && "text-accent")}
    title={title || undefined}
    {onclick}
  >
    {label}
    {#if !active}<ArrowUpDown class="size-3 opacity-40" />{:else if asc}<ArrowUp
        class="size-3"
      />{:else}<ArrowDown class="size-3" />{/if}
  </button>
</div>

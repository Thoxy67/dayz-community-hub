<script lang="ts" generics="T extends string">
  import type { Component } from "svelte";
  import { Toolbar } from "bits-ui";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { cn } from "$lib/cx";

  /**
   * Several things that are each on or off, in one frame: bits-ui's toolbar
   * toggle group. Each says what it is on hover and to a screen reader, and
   * says whether it is on (`aria-pressed`), which a row of plain buttons
   * painted two colours did not.
   */
  type Item = {
    id: T;
    label: string;
    icon: Component<{ class?: string }>;
    kbd?: string;
  };

  let {
    items,
    on,
    onchange,
    class: klass = "",
  }: {
    items: readonly Item[];
    /** The ids that are on. */
    on: readonly T[];
    onchange: (id: T, on: boolean) => void;
    class?: string;
  } = $props();

  // bits-ui hands back the whole new list; what the caller wants to know is
  // which one moved.
  function changed(next: string[]) {
    for (const item of items) {
      const now = next.includes(item.id);
      if (now !== on.includes(item.id)) onchange(item.id, now);
    }
  }
</script>

<Toolbar.Group
  type="multiple"
  bind:value={() => [...on], changed}
  class={cn("flex items-center overflow-hidden rounded-sm border border-border", klass)}
>
  {#each items as item (item.id)}
    <Tooltip
      text={item.label}
      kbd={item.kbd}
      side="bottom"
      class="flex border-r border-border last:border-r-0"
    >
      <Toolbar.GroupItem
        value={item.id}
        aria-label={item.label}
        class="grid size-control place-items-center text-fg-faint hover:bg-raised hover:text-fg
               focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent focus-visible:ring-inset
               data-[state=on]:bg-accent/15 data-[state=on]:text-accent"
      >
        <item.icon class="size-icon" />
      </Toolbar.GroupItem>
    </Tooltip>
  {/each}
</Toolbar.Group>

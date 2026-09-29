<script lang="ts">
  import type { Snippet } from "svelte";
  import { Collapsible } from "bits-ui";
  import ChevronRight from "~icons/lucide/chevron-right";
  import { cn } from "$lib/cx";

  /**
   * A line that opens onto more: the numbers behind a verdict, the long tail
   * of a table. bits-ui's Collapsible, so the trigger says whether it is open
   * and the content is out of the tab order while it is shut.
   *
   * For what is worth having one click away and not worth a panel. The line
   * is quiet on purpose: what it hides is, by definition, not what somebody
   * came to the page for.
   */
  let {
    label,
    open = $bindable(false),
    class: klass = "",
    children,
  }: {
    label: string;
    open?: boolean;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<Collapsible.Root bind:open class={klass}>
  <Collapsible.Trigger
    class={cn(
      "flex h-row w-full items-center gap-1 px-2 text-left text-2xs text-fg-faint select-none",
      "hover:bg-raised/50 hover:text-fg-muted data-[state=open]:text-fg-muted",
    )}
  >
    <ChevronRight
      class="size-icon-sm shrink-0 transition-transform duration-(--duration-fast) {open
        ? 'rotate-90'
        : ''}"
    />
    <span class="min-w-0 flex-1 truncate">{label}</span>
  </Collapsible.Trigger>
  <Collapsible.Content>
    {@render children()}
  </Collapsible.Content>
</Collapsible.Root>

<script lang="ts">
  import { Separator } from "bits-ui";
  import { cn } from "$lib/cx";

  // A hairline between two groups: bits-ui's Separator, which knows whether
  // it means anything (`decorative`) and says so to a screen reader.
  let {
    orientation = "horizontal",
    decorative = true,
    class: klass = "",
  }: {
    orientation?: "horizontal" | "vertical";
    decorative?: boolean;
    class?: string;
  } = $props();
</script>

<Separator.Root
  {orientation}
  {decorative}
  class={cn(
    "shrink-0 bg-border",
    orientation === "horizontal" ? "h-px w-full" : "h-4 w-px self-center",
    klass,
  )}
/>

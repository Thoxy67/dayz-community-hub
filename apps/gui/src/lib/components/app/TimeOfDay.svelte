<script lang="ts">
  import Sun from "~icons/lucide/sun";
  import Moon from "~icons/lucide/moon";
  import Sunrise from "~icons/lucide/sunrise";
  import { cn } from "$lib/cx";

  /** A server's in-game clock, with dawn, day, dusk or night beside it. */
  let { time, class: klass = "" }: { time: string | null | undefined; class?: string } = $props();

  const Phase = $derived.by(() => {
    const h = parseInt(time ?? "", 10);
    if (Number.isNaN(h) || (h >= 5 && h < 7) || (h >= 19 && h < 21)) return Sunrise;
    return h >= 7 && h < 19 ? Sun : Moon;
  });
</script>

<span class={cn("flex items-center gap-1 font-mono text-2xs text-fg-muted", klass)}>
  <Phase class="size-3 shrink-0 text-fg-faint" />{time || "—"}
</span>

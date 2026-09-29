<script lang="ts">
  import Puzzle from "~icons/lucide/puzzle";
  import { cn } from "$lib/cx";

  /**
   * The Mods tab's icon, tinted when something is wrong: red when mods are
   * missing, amber when some are behind. TabStrip draws icons with only a
   * class, so the state is chosen by which wrapper is passed.
   */
  let { class: klass = "", tone }: { class?: string; tone: "warn" | "err" } = $props();
  const look = { warn: ["text-warn", "bg-warn"], err: ["text-err", "bg-err"] } as const;
</script>

<span class={cn("relative inline-grid place-items-center", klass)}>
  <Puzzle class={cn("size-full", look[tone][0])} />
  <span class={cn("absolute -top-0.5 -right-0.5 size-1.5 rounded-full", look[tone][1])}></span>
</span>

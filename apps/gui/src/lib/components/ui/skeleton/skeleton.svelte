<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * The shape of something still on its way: a bar on the raised ground with
   * a faint light crossing it. Size it with classes (`h-2 w-24`). The light is
   * a transform on a child, so a screenful of these costs the compositor only.
   */
  let { class: klass = "" }: { class?: string } = $props();
</script>

<span
  aria-hidden="true"
  class={cn("relative block h-2 overflow-hidden rounded-full bg-raised", klass)}
>
  <span
    class="absolute inset-0 animate-shimmer bg-linear-to-r from-transparent via-fg/6 to-transparent"
  ></span>
</span>

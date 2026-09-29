<script lang="ts">
  import Loader from "~icons/lucide/loader-circle";
  import { cn } from "$lib/cx";

  let { class: klass = "", label = "" }: { class?: string; label?: string } = $props();
</script>

<Loader
  class={cn("size-icon animate-spin text-fg-faint", klass)}
  role={label ? "status" : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}
/>

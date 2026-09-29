<script lang="ts">
  import { dict } from "$lib/i18n";
  import Star from "~icons/lucide/star";
  import { cn } from "$lib/cx";
  import { profile } from "$lib/stores/profile.svelte";

  /** The star: add a server to the favourites, or take it out. */
  let {
    name,
    ip,
    port,
    size = "sm",
  }: { name: string; ip: string; port: number; size?: "sm" | "md" } = $props();
  const c = dict("servers");
  const on = $derived(profile.isFavorite(ip, port));
</script>

<button
  class={cn(
    "grid place-items-center rounded-xs hover:bg-warn/15",
    size === "sm" ? "size-5" : "size-control",
  )}
  title={on ? $c.removeFavorite.value : $c.addFavorite.value}
  aria-label={on ? $c.removeFavorite.value : $c.addFavorite.value}
  aria-pressed={on}
  onclick={(e) => {
    e.stopPropagation();
    void profile.toggleFavorite(name, ip, port);
  }}
>
  <Star
    class={cn(
      size === "sm" ? "size-3.5" : "size-icon",
      on ? "fill-warn text-warn" : "text-fg-faint",
    )}
  />
</button>

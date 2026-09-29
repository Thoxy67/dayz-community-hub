<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Ban from "~icons/lucide/ban";
  import { cn } from "$lib/cx";
  import { profile } from "$lib/stores/profile.svelte";

  /** Hide every server at this IP from the browser, or bring them back. Shows on row hover. */
  let { ip, always = false }: { ip: string; always?: boolean } = $props();
  const c = useIntlayer("servers");
  const on = $derived(profile.excludedIps.has(ip));
</script>

<button
  class={cn(
    "grid size-4 place-items-center rounded-xs hover:bg-err/15",
    on ? "text-err" : "text-fg-faint",
    !on && !always && "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
  )}
  title={on ? $c.ipExcludedClick({ ip }).value : $c.excludeIp({ ip }).value}
  aria-label={on ? $c.ipExcludedClick({ ip }).value : $c.excludeIp({ ip }).value}
  onclick={(e) => {
    e.stopPropagation();
    void (on ? profile.unexcludeIp(ip) : profile.excludeIp(ip));
  }}
>
  <Ban class="size-3" />
</button>

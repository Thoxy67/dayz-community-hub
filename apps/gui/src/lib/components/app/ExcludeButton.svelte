<script lang="ts">
  import { dict } from "$lib/i18n";
  import Ban from "~icons/lucide/ban";
  import { IconButton } from "$lib/components/ui/button";
  import { cn } from "$lib/cx";
  import { profile } from "$lib/stores/profile.svelte";

  /**
   * Hide every server at this IP from the browser, or bring them back: red
   * while hidden. Shows on row hover unless `always`.
   */
  let {
    ip,
    always = false,
    size = "icon-xs",
    variant = "ghost",
  }: {
    ip: string;
    always?: boolean;
    size?: "icon" | "icon-xs";
    variant?: "ghost" | "default";
  } = $props();
  const c = dict("servers");
  const on = $derived(profile.excludedIps.has(ip));
</script>

<IconButton
  icon={Ban}
  {size}
  {variant}
  label={on ? $c.ipExcludedClick({ ip }).value : $c.excludeIp({ ip }).value}
  iconClass={on ? "text-err" : ""}
  hoverTone="danger"
  class={cn(!on && !always && "opacity-0 group-hover:opacity-100 focus-visible:opacity-100")}
  onclick={(e) => {
    e.stopPropagation();
    void (on ? profile.unexcludeIp(ip) : profile.excludeIp(ip));
  }}
/>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import Play from "~icons/lucide/play";
  import { Button } from "$lib/components/ui/button";
  import { connect } from "$lib/stores/connect.svelte";
  import { cn } from "$lib/cx";

  /**
   * Join a server: the listed one when it is in the list (its mods are
   * checked first), else straight to the address.
   *
   * `quiet` is for lists, where every row has one: an outline that fills on
   * the row's hover (a `group`) or when `lit` (the selected row), so a
   * screenful of Join buttons does not outshout the server names.
   */
  let {
    ip,
    port,
    password,
    size = "sm",
    compact = false,
    quiet = false,
    lit = false,
  }: {
    ip: string;
    port: number;
    password?: string | null;
    size?: "sm" | "lg";
    compact?: boolean;
    quiet?: boolean;
    lit?: boolean;
  } = $props();
  const c = dict("servers");
</script>

<Button
  variant="play"
  size={compact ? "icon" : size}
  title={$c.connectTitle.value}
  class={cn(
    quiet &&
      !lit &&
      "border-accent/45 bg-transparent text-accent shadow-none group-hover:border-accent group-hover:bg-accent group-hover:text-accent-fg group-hover:shadow-[var(--glow-accent-soft)]",
  )}
  aria-label={$c.connect.value}
  onclick={(e: MouseEvent) => {
    e.stopPropagation();
    void connect.address(ip, port, password ?? undefined);
  }}
>
  <Play class="size-icon-sm" />{#if !compact}{$c.connect.value}{/if}
</Button>

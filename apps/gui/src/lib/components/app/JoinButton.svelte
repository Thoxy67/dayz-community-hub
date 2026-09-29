<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Play from "~icons/lucide/play";
  import { Button } from "$lib/components/ui/button";
  import { connect } from "$lib/stores/connect.svelte";

  /**
   * Join a server: the listed one when it is in the list (its mods are
   * checked first), else straight to the address.
   */
  let {
    ip,
    port,
    password,
    size = "sm",
    compact = false,
  }: { ip: string; port: number; password?: string | null; size?: "sm" | "lg"; compact?: boolean } = $props();
  const c = useIntlayer("servers");
</script>

<Button
  variant="play"
  size={compact ? "icon" : size}
  title={$c.connectTitle.value}
  aria-label={$c.connect.value}
  onclick={(e: MouseEvent) => {
    e.stopPropagation();
    void (password ? connect.direct({ ip, port, password }) : connect.address(ip, port));
  }}
>
  <Play class="size-icon-sm" />{#if !compact}{$c.connect.value}{/if}
</Button>

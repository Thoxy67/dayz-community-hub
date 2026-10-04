<script lang="ts">
  import { dict } from "$lib/i18n";
  import PlugZap from "~icons/lucide/plug-zap";
  import { IconButton } from "$lib/components/ui/button";
  import CopyIcon from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";
  import { copyText } from "$lib/ipc/native";
  import { ExcludeButton, FavoriteButton, JoinButton } from "$lib/components/app";
  import { connect } from "$lib/stores/connect.svelte";
  import type { DetailModel } from "./model.svelte";

  /** The panel's foot, always in reach whatever tab is open: join, keep, look closer, hide, copy. */
  let { m }: { m: DetailModel } = $props();
  const c = dict("detail");

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function copy() {
    await copyText(m.address);
    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), 1400);
  }
</script>

<footer class="flex shrink-0 items-center gap-1.5 border-t border-border bg-bg/60 px-pad py-2">
  <div class="min-w-0 flex-1 [&_button]:w-full">
    <JoinButton ip={m.ip} port={m.listed ? m.queryPort : m.gamePort} size="lg" />
  </div>
  <span
    class="grid size-control-lg place-items-center rounded-sm border border-border [&>button]:size-full"
  >
    <FavoriteButton name={m.title} ip={m.ip} port={m.queryPort} size="md" />
  </span>
  <IconButton
    icon={PlugZap}
    label={$c.openDirect.value}
    variant="default"
    onclick={() => connect.openInDirect(m.ip, m.gamePort, m.queryPort)}
  />
  <ExcludeButton ip={m.ip} always size="icon" variant="default" />
  <IconButton
    icon={copied ? Check : CopyIcon}
    label={$c.copyIp.value}
    variant="default"
    iconClass={copied ? "text-ok" : ""}
    onclick={copy}
  />
</footer>

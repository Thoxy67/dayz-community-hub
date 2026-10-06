<script lang="ts">
  import type { Snippet } from "svelte";
  import { dict } from "$lib/i18n";
  import Play from "~icons/lucide/play";
  import PanelRight from "~icons/lucide/panel-right";
  import Puzzle from "~icons/lucide/puzzle";
  import Star from "~icons/lucide/star";
  import Activity from "~icons/lucide/activity";
  import Copy from "~icons/lucide/copy";
  import Link from "~icons/lucide/link";
  import PlugZap from "~icons/lucide/plug-zap";
  import Ban from "~icons/lucide/ban";
  import {
    ContextMenuContent,
    ContextMenuItem,
    ContextMenuSeparator,
  } from "$lib/components/ui/context-menu";
  import { copyText } from "$lib/ipc/native";
  import { connect } from "$lib/stores/connect.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { say, errorText } from "$lib/stores/say";

  /**
   * What a right-click on a server offers, in every list: the actions that
   * are otherwise spread over the row's buttons, its keys and the details
   * panel. A list adds its own at the end (`more`).
   */
  let {
    ip,
    queryPort,
    gamePort,
    joinPort,
    name,
    modsCount = 0,
    savedPassword = null,
    ondetails,
    onmods,
    more,
  }: {
    ip: string;
    queryPort: number;
    gamePort: number;
    joinPort: number;
    name: string;
    modsCount?: number;
    savedPassword?: string | null;
    ondetails: () => void;
    onmods?: () => void;
    more?: Snippet;
  } = $props();

  const sv = dict("servers");
  const cm = dict("common");
  const address = $derived(`${ip}:${gamePort}`);
  const favorite = $derived(profile.isFavorite(ip, joinPort));
  const excluded = $derived(profile.excludedIps.has(ip));

  async function copyAddress() {
    try {
      await copyText(address);
      say.ok(`${$cm.copied.value} · ${address}`);
    } catch (e) {
      say.err(errorText(e));
    }
  }
</script>

<ContextMenuContent>
  <ContextMenuItem
    icon={Play}
    kbd="Enter"
    class="font-semibold"
    onselect={() => void connect.address(ip, joinPort, savedPassword ?? undefined)}
  >
    {$sv.connect.value}
  </ContextMenuItem>
  <ContextMenuItem icon={PanelRight} kbd="I" onselect={ondetails}
    >{$sv.details.value}</ContextMenuItem
  >
  {#if modsCount > 0 && onmods}
    <ContextMenuItem icon={Puzzle} onselect={onmods}>{$sv.showMods.value}</ContextMenuItem>
  {/if}
  <ContextMenuSeparator />
  <ContextMenuItem
    icon={Star}
    kbd="F"
    onselect={() => void profile.toggleFavorite(name, ip, joinPort)}
  >
    {favorite ? $sv.removeFavorite.value : $sv.addFavorite.value}
  </ContextMenuItem>
  <ContextMenuItem icon={Activity} kbd="P" onselect={() => void servers.pingOne(ip, queryPort)}>
    {$sv.pingNow.value}
  </ContextMenuItem>
  <ContextMenuSeparator />
  <ContextMenuItem icon={Copy} onselect={copyAddress}>{$sv.copyIpPort.value}</ContextMenuItem>
  <ContextMenuItem
    icon={Link}
    kbd="L"
    onselect={() => void connect.copyLink({ ip, gamePort, queryPort, name })}
  >
    {$sv.copyLinkShort.value}
  </ContextMenuItem>
  <ContextMenuItem
    icon={PlugZap}
    kbd="D"
    onselect={() => connect.openInDirect(ip, gamePort, queryPort, savedPassword ?? undefined)}
  >
    {$sv.directConnect.value}
  </ContextMenuItem>
  <ContextMenuSeparator />
  <ContextMenuItem
    icon={Ban}
    tone={excluded ? "neutral" : "danger"}
    onselect={() => void (excluded ? profile.unexcludeIp(ip) : profile.excludeIp(ip))}
  >
    {excluded ? $sv.includeIp({ ip }).value : $sv.excludeIp({ ip }).value}
  </ContextMenuItem>
  {#if more}
    <ContextMenuSeparator />
    {@render more()}
  {/if}
</ContextMenuContent>

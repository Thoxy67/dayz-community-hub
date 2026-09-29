<script lang="ts">
  import PlugZap from "~icons/lucide/plug-zap";
  import { dict } from "$lib/i18n";
  import { IconButton } from "$lib/components/ui/button";
  import { ExcludeButton, ServerListRow } from "$lib/components/app";
  import type { ServerRow } from "$lib/ipc/servers";
  import { connect } from "$lib/stores/connect.svelte";

  /** One server in the browser: the shared list row, with the browser's own hover actions. */
  let {
    server,
    index,
    selected,
    onselect,
    onjoin,
    onmods,
  }: {
    server: ServerRow;
    index: number;
    selected: boolean;
    onselect: () => void;
    onjoin: () => void;
    onmods: () => void;
  } = $props();

  const sv = dict("servers");
</script>

<ServerListRow
  ip={server.ip}
  queryPort={server.query_port}
  gamePort={server.game_port}
  name={server.name}
  map={server.map}
  time={server.time}
  version={server.version}
  environment={server.environment}
  modsCount={server.mods_count}
  password={server.password}
  firstPerson={server.first_person_only}
  battleye={server.battl_eye}
  excluded={server.excluded}
  {selected}
  rowindex={index + 1}
  {onselect}
  {onjoin}
  {onmods}
>
  {#snippet actions()}
    <IconButton
      icon={PlugZap}
      size="icon-xs"
      label={$sv.directConnect.value}
      onclick={(e) => {
        e.stopPropagation();
        connect.openInDirect(server.ip, server.game_port, server.query_port);
      }}
    />
    <ExcludeButton ip={server.ip} always />
  {/snippet}
</ServerListRow>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import PlugZap from "~icons/lucide/plug-zap";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Search from "~icons/lucide/search";
  import Play from "~icons/lucide/play";
  import Star from "~icons/lucide/star";
  import Eye from "~icons/lucide/eye";
  import EyeOff from "~icons/lucide/eye-off";
  import FileDown from "~icons/lucide/file-down";
  import FileUp from "~icons/lucide/file-up";
  import Link from "~icons/lucide/link";
  import Eraser from "~icons/lucide/eraser";
  import KeyRound from "~icons/lucide/key-round";
  import { PageHeader, Figure, Empty } from "$lib/components/app";
  import { Alert } from "$lib/components/ui/alert";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Panel } from "$lib/components/ui/panel";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Kbd } from "$lib/components/ui/kbd";
  import { connect } from "$lib/stores/connect.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { signalTone, signalLevel } from "$lib/components/ui/signal";
  import { cn } from "$lib/cx";
  import { direct } from "./direct.svelte";
  import ServerDetail from "$features/servers/detail/ServerDetail.svelte";
  import LaunchArgs from "./LaunchArgs.svelte";
  import RecentList from "./RecentList.svelte";

  const c = dict("connect");
  const common = dict("common");

  // Arriving from elsewhere ("open in Direct Connect", a .dzch): fill and ask.
  $effect(() => {
    const p = connect.prefill;
    if (!p) return;
    connect.prefill = null;
    direct.load(p.ip, p.port, p.queryPort, p.password);
  });

  let showPassword = $state(false);
  let touched = $state(false);

  const isFav = $derived(direct.valid && !!direct.favorite);
  const pingKey = $derived(
    direct.queryPortResolved ? `${direct.listed?.ip ?? direct.ip}:${direct.queryPortResolved}` : "",
  );
  const ping = $derived(pingKey ? servers.ping.get(pingKey) : undefined);
  const modsMissing = $derived(
    direct.serverMods.filter((m) => direct.modStatus(m.steam_workshop_id) === "missing").length,
  );

  /** The server to describe: its list entry, or the address the query answered on. */
  const detailTarget = $derived(
    direct.listed
      ? { ip: direct.listed.ip, port: direct.listed.query_port }
      : direct.a2s && direct.resolvedQueryPort
        ? { ip: direct.ip, port: direct.resolvedQueryPort }
        : null,
  );

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      touched = true;
      direct.join();
    }
  }
</script>

<PageHeader title={$c.title.value}>
  {#snippet stats()}
    {#if direct.a2s || direct.listed}
      <Figure
        label={$c.players.value}
        value={`${direct.a2s?.players ?? direct.listed?.players ?? 0}/${direct.a2s?.max_players ?? direct.listed?.max_players ?? 0}`}
        tone="text-ok"
      />
      <Figure
        label={$c.ping.value}
        value={ping == null ? "—" : ping >= 5000 ? "×" : `${ping} ms`}
        tone={signalTone(signalLevel(ping))}
      />
      <Figure
        label={$c.map.value}
        value={direct.a2s?.map || direct.listed?.map || "—"}
        tone="text-map"
      />
      <Figure
        label={$c.modsCount({ count: direct.serverMods.length }).value}
        value={modsMissing
          ? `${modsMissing} ${$c.modMissing.value.toLowerCase()}`
          : String(direct.serverMods.length)}
        tone={modsMissing ? "text-warn" : "text-mods"}
      />
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={() => direct.openFile()} title={$c.openDzchTitle.value}>
      <FileUp class="size-icon-sm" />{$c.openDzch.value}
    </Button>
    <Button
      disabled={!direct.valid}
      onclick={() => direct.copyLink()}
      title={$c.copyUrlTitle.value}
    >
      <Link class="size-icon-sm" />{$c.copyUrl.value}
    </Button>
    <Button
      disabled={!direct.valid}
      onclick={() => direct.exportFile()}
      title={$c.exportTitle.value}
    >
      <FileDown class="size-icon-sm" />{$c.export.value}
    </Button>
  {/snippet}
</PageHeader>

<div class="grid min-h-0 flex-1 grid-cols-[25rem_minmax(0,1fr)] gap-px bg-border">
  <!-- The form, what to add to the launch, and where to start from. -->
  <div class="flex min-h-0 flex-col gap-px overflow-y-auto bg-border">
    <Panel title={$c.connection.value} scroll={false}>
      <form
        class="flex flex-col gap-2.5 p-3"
        onsubmit={(e) => {
          e.preventDefault();
          touched = true;
          void direct.query();
        }}
      >
        <p class="m-0 text-2xs leading-snug text-fg-muted">{$c.intro.value}</p>

        <label class="flex flex-col gap-1">
          <span class="text-2xs font-medium text-fg">{$c.ipHostname.value}</span>
          <Input
            bind:value={direct.address}
            placeholder={$c.ipPlaceholder.value}
            class={cn("font-mono", touched && direct.addressError && "[&_input]:border-err/60")}
            autocomplete="off"
            spellcheck={false}
            onblur={() => direct.splitAddress()}
            {onkeydown}
          />
          <span
            class={cn("text-3xs", touched && direct.addressError ? "text-err" : "text-fg-faint")}
          >
            {touched && direct.addressError ? $c.addressMissing.value : $c.addressHint.value}
          </span>
        </label>

        <div class="grid grid-cols-2 gap-2">
          <label class="flex flex-col gap-1">
            <span class="text-2xs font-medium text-fg"
              >{$c.port.value} <span class="text-fg-faint">{$c.portGame.value}</span></span
            >
            <Input
              bind:value={direct.port}
              type="number"
              min="1"
              max="65535"
              class={cn(direct.portError && "[&_input]:border-err/60")}
              {onkeydown}
            />
          </label>
          <label class="flex flex-col gap-1">
            <span class="text-2xs font-medium text-fg">{$c.queryPortOptional.value}</span>
            <Input
              bind:value={direct.queryPort}
              type="number"
              min="1"
              max="65535"
              placeholder="27016"
              {onkeydown}
            />
          </label>
        </div>
        {#if direct.portError}
          <span class="-mt-1.5 text-3xs text-err">{$c.portInvalid.value}</span>
        {:else}
          <span class="-mt-1.5 text-3xs text-fg-faint">{$c.queryPortHint.value}</span>
        {/if}

        <label class="flex flex-col gap-1">
          <span class="text-2xs font-medium text-fg"
            >{$c.password.value} <span class="text-fg-faint">({$c.optional.value})</span></span
          >
          <span class="flex gap-1">
            <Input
              bind:value={direct.password}
              type={showPassword ? "text" : "password"}
              placeholder={$c.passwordPlaceholder.value}
              autocomplete="off"
              class="min-w-0 flex-1"
              oninput={() => (direct.passwordFromFavorite = false)}
              {onkeydown}
            />
            <IconButton
              icon={showPassword ? EyeOff : Eye}
              label={showPassword ? $c.hidePassword.value : $c.showPassword.value}
              onclick={() => (showPassword = !showPassword)}
            />
          </span>
          {#if direct.passwordFromFavorite && direct.password}
            <span class="flex items-center gap-1 text-3xs text-warn"
              ><KeyRound class="size-3" />{$c.savedPassword.value}</span
            >
          {/if}
        </label>

        <div class="flex gap-1.5">
          <Button type="submit" disabled={direct.querying || !direct.valid} class="flex-1">
            {#if direct.querying}<Spinner class="size-icon-sm" />{$c.querying.value}{:else}<Search
                class="size-icon-sm"
              />{$c.query.value}{/if}
          </Button>
          <IconButton
            icon={Star}
            label={isFav ? $c.alreadyFavorite.value : $c.addFavorite.value}
            active={isFav}
            disabled={!direct.valid}
            onclick={() => direct.favoriteIt()}
          />
          <IconButton
            icon={Eraser}
            label={$common.clear.value}
            disabled={!direct.address}
            onclick={() => direct.clear()}
          />
        </div>
        <Button
          variant="play"
          size="lg"
          disabled={!direct.valid || !profile.data}
          onclick={() => {
            touched = true;
            direct.join();
          }}
        >
          <Play class="size-icon" />{$c.connect.value}
        </Button>
        <span class="flex items-center justify-center gap-1.5 text-3xs text-fg-faint"
          ><Kbd>Enter</Kbd>{$c.enterToConnect.value}</span
        >
      </form>
    </Panel>

    <Panel title={$c.extraMods.value} scroll={false}>
      {#snippet toolbar()}
        <span class="font-mono text-3xs text-fg-faint"
          >{$c.argsCount({ count: direct.launchArgs.length }).value}</span
        >
      {/snippet}
      <LaunchArgs />
    </Panel>

    <Panel title={$c.recent.value} scroll={false} class="flex-1">
      <RecentList />
    </Panel>
  </div>

  <!-- What the server says: the same detail column as the server list. -->
  <div class="flex min-h-0 flex-col bg-panel">
    {#if direct.error}
      <Alert tone="err" icon={TriangleAlert} title={$c.queryFailed.value} banner>
        <span class="font-mono" data-selectable>{direct.error}</span>
      </Alert>
    {/if}
    {#if detailTarget}
      {#key `${detailTarget.ip}:${detailTarget.port}`}
        <div class="min-h-0 flex-1">
          <ServerDetail ip={detailTarget.ip} port={detailTarget.port} name={direct.name} />
        </div>
      {/key}
    {:else if direct.querying}
      <div class="flex flex-1 items-center justify-center gap-2 text-xs text-fg-muted">
        <Spinner class="text-accent" />{$c.queryingServer.value}
      </div>
    {:else if !direct.error}
      <Empty icon={PlugZap} title={$c.noServer.value}>{$c.noServerHint.value}</Empty>
    {/if}
  </div>
</div>

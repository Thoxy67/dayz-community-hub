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
  import { PageHeader, Figure } from "$lib/components/app";
  import { Alert } from "$lib/components/ui/alert";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Panel } from "$lib/components/ui/panel";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Disclosure } from "$lib/components/ui/disclosure";
  import { connect } from "$lib/stores/connect.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { signalTone, signalLevel } from "$lib/components/ui/signal";
  import { cn } from "$lib/cx";
  import { direct } from "./direct.svelte";
  import ServerDetail from "$features/servers/detail/ServerDetail.svelte";
  import LaunchArgs from "./LaunchArgs.svelte";
  import RecentList from "./RecentList.svelte";
  import { padActions } from "$lib/gamepad";

  const c = dict("connect");
  const common = dict("common");
  const p = dict("pad");

  // A controller: X asks the server, Y joins it.
  padActions("connect", {
    primary: {
      label: () => $p.query.value,
      when: () => direct.valid && !direct.querying,
      run: () => void direct.query(),
    },
    secondary: {
      label: () => $p.join.value,
      when: () => direct.valid && !!profile.data,
      run: () => direct.join(),
    },
  });

  // Arriving from elsewhere ("open in Direct Connect", a .dzch): fill and ask.
  $effect(() => {
    const p = connect.prefill;
    if (!p) return;
    connect.prefill = null;
    direct.load(p.ip, p.port, p.queryPort, p.password);
  });

  let showPassword = $state(false);
  let touched = $state(false);
  // The ports rarely need a hand: shut unless one is wrong.
  let portsOpen = $state(false);
  $effect(() => {
    if (direct.portError) portsOpen = true;
  });
  let argsOpen = $state(false);
  const portsLabel = $derived(
    $c.portsSummary({
      ports: direct.queryPort ? `${direct.port} / ${direct.queryPort}` : String(direct.port || "—"),
    }).value,
  );

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
  <!-- The form, then where to start from; what to add to the launch stays folded. -->
  <div class="flex min-h-0 flex-col gap-px bg-border">
    <Panel title={$c.connection.value} scroll={false}>
      {#snippet toolbar()}
        <IconButton
          icon={Star}
          size="icon-xs"
          label={isFav ? $c.alreadyFavorite.value : $c.addFavorite.value}
          active={isFav}
          disabled={!direct.valid}
          onclick={() => direct.favoriteIt()}
        />
        <IconButton
          icon={Eraser}
          size="icon-xs"
          label={$common.clear.value}
          disabled={!direct.address}
          onclick={() => direct.clear()}
        />
      {/snippet}
      <form
        class="flex flex-col gap-2.5 p-3"
        title={$c.intro.value}
        onsubmit={(e) => {
          e.preventDefault();
          touched = true;
          void direct.query();
        }}
      >
        <label class="flex flex-col gap-1">
          <span class="text-2xs font-medium text-fg">{$c.ipHostname.value}</span>
          <Input
            bind:value={direct.address}
            placeholder={$c.ipPlaceholder.value}
            title={$c.addressHint.value}
            class={cn("font-mono", touched && direct.addressError && "[&_input]:border-err/60")}
            autocomplete="off"
            spellcheck={false}
            onblur={() => direct.splitAddress()}
            {onkeydown}
          />
          {#if touched && direct.addressError}
            <span class="text-3xs text-err">{$c.addressMissing.value}</span>
          {/if}
        </label>

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

        <Disclosure label={portsLabel} bind:open={portsOpen} class="-mx-3 -my-1">
          <div class="flex flex-col gap-1 px-3 pt-1 pb-1.5">
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
            <span class={cn("text-3xs", direct.portError ? "text-err" : "text-fg-faint")}>
              {direct.portError ? $c.portInvalid.value : $c.queryPortHint.value}
            </span>
          </div>
        </Disclosure>

        <div class="flex gap-1.5">
          <Button
            type="submit"
            size="lg"
            disabled={direct.querying || !direct.valid}
            class="flex-1"
            title={$c.query.value}
          >
            {#if direct.querying}<Spinner class="size-icon-sm" />{$c.querying.value}{:else}<Search
                class="size-icon-sm"
              />{$c.query.value}{/if}
          </Button>
          <Button
            variant="play"
            size="lg"
            class="flex-[1.4]"
            disabled={!direct.valid || !profile.data}
            title={$c.enterToConnect.value}
            onclick={() => {
              touched = true;
              direct.join();
            }}
          >
            <Play class="size-icon" />{$c.connect.value}
          </Button>
        </div>
      </form>
    </Panel>

    <section class="shrink-0 bg-panel">
      <Disclosure
        label={`${$c.extraMods.value} · ${$c.argsCount({ count: direct.launchArgs.length }).value}`}
        bind:open={argsOpen}
      >
        <div class="max-h-[45vh] overflow-y-auto border-t border-border">
          <LaunchArgs />
        </div>
      </Disclosure>
    </section>

    <!-- Once a server is shown on the right, the recent ones fold into a
         list here; before, they fill the right as cards. -->
    {#if detailTarget || direct.querying || direct.error}
      <Panel title={$c.recent.value} class="flex-1">
        <RecentList />
      </Panel>
    {:else}
      <div class="flex-1 bg-panel"></div>
    {/if}
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
      <div class="min-h-0 flex-1 overflow-y-auto p-pad">
        <div class="mb-3 flex items-start gap-2.5">
          <PlugZap class="mt-0.5 size-icon shrink-0 text-accent" />
          <div class="min-w-0">
            <h2 class="m-0 text-sm font-semibold text-fg">{$c.recent.value}</h2>
            <p class="m-0 mt-0.5 text-2xs text-fg-muted">{$c.startFrom.value}</p>
          </div>
        </div>
        <RecentList wide />
      </div>
    {/if}
  </div>
</div>

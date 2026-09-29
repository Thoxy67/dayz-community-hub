<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { dict } from "$lib/i18n";
  import Minus from "~icons/lucide/minus";
  import Square from "~icons/lucide/square";
  import Copy from "~icons/lucide/copy";
  import X from "~icons/lucide/x";
  import ServerIcon from "~icons/lucide/server";
  import Users from "~icons/lucide/users";
  import { SteamIcon } from "$lib/components/ui/brand";
  import Download from "~icons/lucide/download";
  import PackageUp from "~icons/lucide/package-open";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import UserRound from "~icons/lucide/user-round";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { inTauri } from "$lib/ipc/core";
  import { app } from "$lib/stores/app.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { num } from "$lib/format";
  import { placeOf } from "./nav";
  import LanguageMenu from "./LanguageMenu.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";

  const s = dict("shell");
  const n = dict("nav");
  const win = inTauri ? getCurrentWindow() : null;

  const here = $derived(placeOf(app.view));
  const staleCount = $derived(mods.stale.length);

  /** Drag from anything that is not a control; double-click toggles maximise. */
  function onmousedown(e: MouseEvent) {
    if (e.buttons !== 1 || !win) return;
    if ((e.target as HTMLElement).closest("button, input, a, [data-no-drag]")) return;
    if (e.detail === 2) void win.toggleMaximize();
    else void win.startDragging();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<header
  data-pad-region
  class="relative z-titlebar flex h-titlebar shrink-0 items-stretch border-b border-border bg-bg select-none"
  {onmousedown}
>
  <!-- Identity, then where we are. -->
  <div class="flex min-w-0 items-center gap-2.5 pl-3">
    <img src="/icon.svg" alt="" class="size-5 shrink-0" draggable="false" />
    <span
      class="title-display text-[15px] leading-none tracking-[0.06em] whitespace-nowrap text-fg"
    >
      DayZ <span class="text-accent">Community Hub</span>
    </span>
    <span class="h-3.5 w-px bg-border-strong"></span>
    <span class="label-stencil truncate text-fg-muted">{String($n[here.label])}</span>
  </div>

  <!-- The live figures, centred on the window, not on the space left over. -->
  <div
    class="pointer-events-none absolute inset-y-0 left-1/2 flex -translate-x-1/2 items-center gap-4 max-[1180px]:hidden"
  >
    {#snippet stat(
      Icon: import("svelte").Component<{ class?: string }>,
      value: string,
      label: string,
      tone: string,
    )}
      <Tooltip text={label} side="bottom">
        <span class="pointer-events-auto flex items-center gap-1.5">
          <Icon class="size-icon-sm {tone}" />
          <span class="num font-mono text-2xs font-medium text-fg">{value}</span>
        </span>
      </Tooltip>
    {/snippet}
    {@render stat(
      ServerIcon,
      num(servers.stats?.server_count ?? (servers.total || null)),
      $s.titlebarServers.value,
      "text-info",
    )}
    {@render stat(
      Users,
      num(servers.stats?.total_players),
      $s.titlebarPlayersIngame.value,
      "text-ok",
    )}
    {@render stat(
      SteamIcon,
      num(servers.steamPlayers),
      $s.titlebarPlayersSteam.value,
      "text-fg-muted",
    )}
  </div>

  <div class="ml-auto flex items-stretch">
    <!-- Downloading through the Steam client needs no SteamCMD. -->
    {#if servers.stats && !servers.stats.has_steamcmd && !profile.viaSteam}
      <button
        class="flex items-center gap-1.5 px-2.5 text-2xs font-medium text-warn hover:bg-warn/10"
        onclick={() => app.go("settings", "steam")}
      >
        <TriangleAlert class="size-icon-sm" />
        <span class="max-xl:hidden">{$s.titlebarSteamcmdMissing.value}</span>
      </button>
    {/if}

    {#if updater.state === "available"}
      <Tooltip text={$s.titlebarUpdateAvailableTitle.value} side="bottom">
        <button
          class="flex items-center gap-1.5 px-2.5 text-2xs font-semibold text-ok hover:bg-ok/10"
          onclick={() => app.go("about", "update")}
        >
          <Download class="size-icon-sm" />
          <span class="max-xl:hidden">{$s.titlebarUpdateAvailable.value}</span>
        </button>
      </Tooltip>
    {/if}

    {#if staleCount > 0}
      <Tooltip
        text={staleCount === 1
          ? $s.titlebarUpdateModsTitleOne({ count: staleCount }).value
          : $s.titlebarUpdateModsTitle({ count: staleCount }).value}
        side="bottom"
      >
        <button
          class="flex items-center gap-1.5 px-2.5 text-2xs font-semibold text-warn hover:bg-warn/10"
          onclick={() => {
            app.go("mods");
            mods.updateStale();
          }}
        >
          <PackageUp class="size-icon-sm" />
          <span class="num font-mono">{staleCount}</span>
        </button>
      </Tooltip>
    {/if}

    <span class="my-2 w-px bg-border"></span>

    <!-- The player: avatar, name, Steam login. Opens the account settings. -->
    <Tooltip text={$s.titlebarEditAccount.value} side="bottom">
      <button
        class="flex max-w-56 items-center gap-2 px-2.5 text-xs text-fg-muted hover:bg-raised hover:text-fg"
        onclick={() => app.go("settings", "account")}
      >
        {#if profile.avatarUrl}
          <img
            src={profile.avatarUrl}
            alt=""
            class="size-5 shrink-0 rounded-full ring-1 ring-border-strong"
          />
        {:else}
          <UserRound class="size-icon shrink-0" />
        {/if}
        {#if servers.stats?.player_name}
          <span class="truncate font-medium text-fg">{servers.stats.player_name}</span>
          {#if servers.stats.steam_login}
            <span class="truncate font-mono text-2xs text-fg-faint max-xl:hidden"
              >{servers.stats.steam_login}</span
            >
          {/if}
        {:else}
          <span class="italic">{$s.titlebarSetupAccount.value}</span>
        {/if}
      </button>
    </Tooltip>

    <LanguageMenu />
    <ThemeMenu />

    {#if win}
      <span class="my-2 w-px bg-border"></span>
      <button
        class="grid w-11 place-items-center text-fg-muted hover:bg-raised hover:text-fg"
        aria-label={$s.windowMinimize.value}
        onclick={() => win.minimize()}><Minus class="size-icon-sm" /></button
      >
      <button
        class="grid w-11 place-items-center text-fg-muted hover:bg-raised hover:text-fg"
        aria-label={$s.windowMaximize.value}
        onclick={() => win.toggleMaximize()}
      >
        {#if app.maximized}<Copy class="size-3" />{:else}<Square class="size-3" />{/if}
      </button>
      <button
        class="grid w-11 place-items-center text-fg-muted hover:bg-err hover:text-white"
        aria-label={$s.windowClose.value}
        onclick={() => win.close()}><X class="size-icon-sm" /></button
      >
    {/if}
  </div>
</header>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import Check from "~icons/lucide/check";
  import Settings from "~icons/lucide/settings";
  import { DropdownMenu as Menu } from "bits-ui";
  import {
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
  } from "$lib/components/ui/dropdown-menu";
  import { SteamIcon } from "$lib/components/ui/brand";
  import type { ModDownloaderDto } from "$lib/ipc/types";
  import { steamworksStatus } from "$lib/ipc/mods";
  import { app } from "$lib/stores/app.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import Segment, { WORD } from "./Segment.svelte";

  /**
   * Steam, in one small mark: what downloads the mods (SteamCMD and the
   * account it signed in with, or the Steam client) and whether the Steam
   * client is running, which DayZ needs to start at all. A dot says the
   * latter; amber when downloads go through a Steam that is not there. A
   * click opens the switch between the two, and the Steam settings.
   */
  let { now }: { now: number } = $props();
  const n = dict("nav");
  const s = dict("shell");
  const st = dict("settings");

  const downloaders = $derived<{ value: ModDownloaderDto; label: string; hint: string }[]>([
    { value: "steamcmd", label: "SteamCMD", hint: $n.sbSteamcmdHint.value },
    { value: "steamworks", label: $st.downloaderSteam.value, hint: $n.sbSteamHint.value },
  ]);

  /** Whether the Steam client runs: asked with the bar's clock, while the window has focus. */
  let running = $state<boolean | null>(null);
  $effect(() => {
    void now;
    if (!app.focused) return;
    steamworksStatus()
      .then((r) => (running = r.steam_running))
      .catch(() => (running = null));
  });

  const login = $derived(profile.data?.steamcmd_logged_in ?? null);
  const via = $derived(profile.viaSteam);
  const tip = $derived(
    [
      via
        ? $s.sbSteamClient.value
        : login
          ? $s.sbSteamCmdAs({ login }).value
          : $s.sbSteamCmdNoLogin.value,
      running === true ? $s.sbSteamUp.value : running === false ? $s.sbSteamDown.value : "",
      $s.sbSteamAction.value,
    ]
      .filter(Boolean)
      .join(" "),
  );
</script>

<Menu.Root>
  <Menu.Trigger>
    {#snippet child({ props })}
      <Segment
        icon={SteamIcon}
        tone={running === false ? "warn" : "idle"}
        {tip}
        label={$n.sbDownloader.value}
        trigger={props}
        class="@max-[900px]:hidden"
      >
        <span class={WORD}>{via ? "Steam" : "SteamCMD"}</span>
        <span
          class={cn(
            "size-1.5 rounded-full",
            running === true ? "bg-ok" : running === false ? "bg-warn" : "bg-fg-faint",
          )}
          aria-hidden="true"
        ></span>
      </Segment>
    {/snippet}
  </Menu.Trigger>
  <DropdownMenuContent side="top" align="end" class="min-w-60">
    <DropdownMenuGroup>
      <DropdownMenuLabel>{$n.sbDownloader.value}</DropdownMenuLabel>
      {#each downloaders as o (o.value)}
        {@const on = (profile.data?.mod_downloader ?? "steamcmd") === o.value}
        <DropdownMenuItem
          disabled={mods.busy && !on}
          class="h-auto py-1"
          onselect={() => {
            if (!on) void profile.setModDownloader(o.value);
          }}
        >
          <span class="flex items-center gap-2">
            <span class="grid size-3.5 place-items-center text-accent"
              >{#if on}<Check class="size-3" />{/if}</span
            >
            <span class="flex min-w-0 flex-col">
              <span class={cn(on ? "text-fg" : "text-fg-muted")}>{o.label}</span>
              <span class="truncate text-3xs text-fg-faint"
                >{mods.busy && !on ? $n.sbDownloaderBusy.value : o.hint}</span
              >
            </span>
          </span>
        </DropdownMenuItem>
      {/each}
    </DropdownMenuGroup>
    <DropdownMenuSeparator />
    <DropdownMenuItem icon={Settings} onselect={() => app.go("settings", "steam")}>
      {$n.sbSteamSettings.value}
    </DropdownMenuItem>
  </DropdownMenuContent>
</Menu.Root>

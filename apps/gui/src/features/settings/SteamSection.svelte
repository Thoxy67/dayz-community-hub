<script lang="ts">
  import { dict } from "$lib/i18n";
  import Gamepad from "~icons/lucide/gamepad-2";
  import LogIn from "~icons/lucide/log-in";
  import FolderOpen from "~icons/lucide/folder-open";
  import ShieldCheck from "~icons/lucide/shield-check";
  import CircleCheck from "~icons/lucide/circle-check";
  import CircleX from "~icons/lucide/circle-x";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Download from "~icons/lucide/download";
  import { Field } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Tag } from "$lib/components/ui/tag";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Copy } from "$lib/components/ui/copy";
  import {
    detectSteamcmd,
    downloadSteamcmdWindows,
    openSteamcmdDir,
    steamcmdDirs,
    type SteamcmdStatus,
  } from "$lib/ipc/system";
  import { errorText } from "$lib/ipc/core";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { SettingsSection as Section } from "$lib/components/app";
  import { PathInput } from "$lib/components/app";
  import { form } from "./account-form.svelte";

  const s = dict("settings");
  const a = dict("about");

  let status = $state<SteamcmdStatus | null>(null);
  let detecting = $state(false);
  let installing = $state(false);
  let installError = $state("");

  async function detect() {
    detecting = true;
    try {
      status = await detectSteamcmd();
    } catch {
      status = null;
    } finally {
      detecting = false;
    }
  }
  $effect(() => void detect());

  async function install() {
    installing = true;
    installError = "";
    try {
      await downloadSteamcmdWindows();
      await detect();
      void servers.loadStats();
    } catch (e) {
      installError = errorText(e);
    } finally {
      installing = false;
    }
  }

  // SteamCMD's own folder: shown so the player can see it is not a Steam library.
  let contentDir = $state("");
  $effect(() => {
    steamcmdDirs()
      .then((d) => (contentDir = d.content))
      .catch(() => {});
  });

  const savedLogin = $derived(profile.data?.steam_login ?? "");
  const loggedIn = $derived(!!savedLogin && profile.data?.steamcmd_logged_in === savedLogin);

  const LINUX = [
    { label: () => $a.steamcmdLinuxDebian.value, cmd: "sudo apt install steamcmd" },
    { label: () => $a.steamcmdLinuxArch.value, cmd: "yay -S steamcmd" },
    { label: () => $a.steamcmdLinuxFedora.value, cmd: "sudo dnf install steamcmd" },
  ];
</script>

<Section
  id="steam"
  title={$s.sectionSteam.value}
  description={$s.steamLoginDesc.value}
  icon={Gamepad}
>
  {#snippet aside()}
    {#if detecting}
      <Spinner />
    {:else if status?.found}
      <Tag tone="ok"><CircleCheck class="size-3" />SteamCMD</Tag>
    {:else if status}
      <Tag tone="err"><CircleX class="size-3" />SteamCMD</Tag>
    {/if}
  {/snippet}

  <Field label={$s.username.value} hint={$s.usernameHint.value} for="set-login">
    <Input
      id="set-login"
      bind:value={form.f.steamLogin}
      autocomplete="username"
      spellcheck={false}
      class="flex-1 font-mono"
    />
  </Field>
  <!-- The login: never a password here, SteamCMD asks once and caches it. -->
  <div class="flex flex-col gap-1.5 border-b border-border/60 px-pad py-2.5">
    <div class="flex items-center gap-2 text-xs">
      {#if loggedIn}
        <CircleCheck class="size-icon-sm shrink-0 text-ok" />
        <span class="text-fg">{$s.loggedInAs({ name: savedLogin }).value}</span>
      {:else}
        <ShieldCheck class="size-icon-sm shrink-0 text-fg-faint" />
        <span class="text-fg-muted"
          >{savedLogin ? $s.notLoggedIn.value : $s.loginNeedsUsername.value}</span
        >
      {/if}
      <Button
        variant="ghost"
        size="xs"
        class="ml-auto"
        onclick={() => mods.login()}
        disabled={!savedLogin || !status?.found || mods.busy || form.dirty}
      >
        <LogIn class="size-3" />{$s.loginButton.value}
      </Button>
    </div>
    <p class="m-0 text-2xs text-fg-faint">{$s.loginOnceHint.value}</p>
    {#if profile.data?.has_saved_password}
      <p class="m-0 text-2xs text-warn">{$s.savedPasswordPending.value}</p>
    {/if}
  </div>
  <Field label={$s.steamRoot.value} hint={$s.steamRootHint.value} for="set-root">
    <PathInput
      id="set-root"
      bind:value={form.f.steamRoot}
      title={$s.selectSteamRoot.value}
      directory
      placeholder="~/.steam/steam"
    />
  </Field>
  <Field label={$s.steamcmdPath.value} hint={$s.steamcmdPathHint.value} for="set-steamcmd">
    <PathInput
      id="set-steamcmd"
      bind:value={form.f.steamcmdPath}
      title={$s.selectSteamcmd.value}
      placeholder={status?.path ?? "auto"}
    />
  </Field>

  <Field label={$s.contentDir.value} hint={$s.contentDirHint.value}>
    <div class="flex min-w-0 flex-1 items-center gap-1.5">
      {#if contentDir}<Copy text={contentDir} class="min-w-0 text-fg-muted" />{/if}
      <Button
        variant="ghost"
        size="xs"
        class="ml-auto shrink-0"
        onclick={() => openSteamcmdDir().catch(() => {})}
      >
        <FolderOpen class="size-3" />{$s.openFolder.value}
      </Button>
    </div>
  </Field>

  <!-- What was found, and what to do when nothing was. -->
  <div class="flex flex-col gap-2 px-pad py-2.5">
    <div class="flex items-center gap-2 text-xs">
      {#if status?.found}
        <CircleCheck class="size-icon-sm shrink-0 text-ok" />
        <span class="text-fg">{$s.steamcmdFound.value}</span>
        {#if status.path}<Copy text={status.path} class="min-w-0" />{/if}
      {:else}
        <CircleX class="size-icon-sm shrink-0 text-err" />
        <span class="text-err">{$s.steamcmdMissing.value}</span>
      {/if}
      <Button variant="ghost" size="xs" class="ml-auto" onclick={detect} disabled={detecting}>
        <RefreshCw class="size-3" />{$s.steamcmdDetect.value}
      </Button>
    </div>
    {#if status && !status.found}
      <p class="m-0 text-2xs text-fg-muted">
        {$a.steamcmdDesc({ notFound: $a.steamcmdNotFound.value }).value}
      </p>
      {#if status.platform === "windows"}
        <div class="flex items-center gap-2">
          <Button variant="accent" onclick={install} disabled={installing}>
            {#if installing}<Spinner class="size-3 text-accent-fg" />{$s.steamcmdInstalling
                .value}{:else}<Download class="size-icon-sm" />{$s.steamcmdInstallWin.value}{/if}
          </Button>
          <span class="text-2xs text-fg-faint">{$a.steamcmdWinDesc.value}</span>
        </div>
        {#if installError}<p class="m-0 font-mono text-2xs text-err" data-selectable>
            {installError}
          </p>{/if}
      {:else}
        <p class="m-0 text-2xs text-fg-muted">{$s.steamcmdInstallLinux.value}</p>
        <div class="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-1">
          {#each LINUX as row (row.cmd)}
            <span class="text-2xs text-fg-faint">{row.label()}</span>
            <Copy text={row.cmd} class="justify-self-start text-fg-muted" />
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</Section>

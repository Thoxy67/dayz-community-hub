<script lang="ts">
  import { dict } from "$lib/i18n";
  import CircleCheck from "~icons/lucide/circle-check";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Download from "~icons/lucide/download";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Copy from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";
  import Radar from "~icons/lucide/radar";
  import Terminal from "~icons/lucide/terminal";
  import FolderCog from "~icons/lucide/folder-cog";
  import { Button } from "$lib/components/ui/button";
  import { Field } from "$lib/components/ui/field";
  import { Spinner } from "$lib/components/ui/spinner";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import PathInput from "$lib/components/app/PathInput.svelte";
  import { copyText } from "$lib/ipc/native";
  import { cn } from "$lib/cx";
  import { wizard } from "./wizard.svelte";

  const w = dict("setup");

  const DISTROS = [
    { label: "Arch / Manjaro", cmd: "yay -S steamcmd" },
    { label: "Debian / Ubuntu", cmd: "sudo apt install steamcmd" },
    { label: "Fedora / RHEL", cmd: "sudo dnf install steamcmd" },
  ];

  let copied = $state<string | null>(null);
  async function copy(cmd: string) {
    await copyText(cmd);
    copied = cmd;
    setTimeout(() => copied === cmd && (copied = null), 1400);
  }

  const win = $derived(wizard.platform === "windows");
</script>

<div class="space-y-3">
  <!-- Where things stand. -->
  <div
    class={cn(
      "flex items-center gap-3 rounded-md border px-3 py-2.5",
      wizard.detecting
        ? "border-border bg-raised/40"
        : wizard.found
          ? "border-ok/40 bg-ok/8"
          : "border-warn/40 bg-warn/8",
    )}
  >
    {#if wizard.detecting}
      <Spinner class="size-icon-lg text-accent" />
      <span class="text-xs text-fg-muted">{$w.steamcmdDetecting.value}</span>
    {:else if wizard.found}
      <CircleCheck class="size-icon-lg shrink-0 text-ok" />
      <div class="min-w-0">
        <p class="m-0 text-xs font-semibold text-ok">{$w.steamcmdDetected.value}</p>
        <p class="m-0 truncate font-mono text-2xs text-fg-muted" data-selectable>
          {wizard.status?.path}
        </p>
      </div>
    {:else}
      <TriangleAlert class="size-icon-lg shrink-0 text-warn" />
      <div class="min-w-0 flex-1">
        <p class="m-0 text-xs font-semibold text-warn">{$w.steamcmdNotFound.value}</p>
        <p class="m-0 text-2xs text-fg-faint">
          {$w.platform({ name: win ? "Windows" : "Linux" }).value}
        </p>
      </div>
      <Button size="xs" variant="ghost" onclick={() => wizard.detect()}>
        <RefreshCw class="size-icon-sm" />{$w.rescan.value}
      </Button>
    {/if}
  </div>

  {#if !wizard.found && !wizard.detecting}
    {#if !win}
      <SectionCard title={$w.steamcmdLinuxHint.value} icon={Terminal}>
        <ul class="m-0 list-none divide-y divide-border/50 p-0">
          {#each DISTROS as d (d.cmd)}
            <li class="flex items-center gap-3 px-3 py-1.5">
              <span class="w-28 shrink-0 text-2xs text-fg-muted">{d.label}</span>
              <code
                class="min-w-0 flex-1 truncate rounded-sm bg-bg px-2 py-1 font-mono text-2xs text-fg"
                data-selectable>{d.cmd}</code
              >
              <button
                class={cn(
                  "grid size-control-sm place-items-center rounded-sm hover:bg-raised",
                  copied === d.cmd ? "text-ok" : "text-fg-faint hover:text-fg",
                )}
                aria-label={$w.copyCommand.value}
                title={$w.copyCommand.value}
                onclick={() => copy(d.cmd)}
              >
                {#if copied === d.cmd}<Check class="size-icon-sm" />{:else}<Copy
                    class="size-icon-sm"
                  />{/if}
              </button>
            </li>
          {/each}
        </ul>
        <p
          class="m-0 flex items-center gap-2 border-t border-border/60 px-3 py-2 text-2xs text-fg-muted"
        >
          <Radar class="size-icon-sm animate-pulse text-accent" />{$w.steamcmdLinuxWaiting.value}
        </p>
      </SectionCard>
    {:else}
      <SectionCard title="SteamCMD" icon={Download} padded>
        <p class="m-0 mb-2.5 text-2xs text-fg-muted">{$w.steamcmdWinHint.value}</p>
        <Button
          variant="accent"
          class="w-full"
          disabled={wizard.downloading}
          onclick={() => wizard.download()}
        >
          {#if wizard.downloading}<Spinner class="size-icon-sm text-accent-fg" />{$w
              .steamcmdDownloading.value}{:else}<Download class="size-icon-sm" />{$w
              .steamcmdInstallAuto.value}{/if}
        </Button>
        <p class="m-0 mt-1.5 text-3xs text-fg-faint">
          {$w.steamcmdInstallHint.value} %LOCALAPPDATA%\dayz-community-hub\steamcmd
        </p>
        {#if wizard.downloadError}
          <p class="m-0 mt-2 font-mono text-2xs text-err" data-selectable>{wizard.downloadError}</p>
        {/if}
      </SectionCard>
    {/if}
  {/if}

  <!-- Paths, for anyone whose install is not where it is looked for. -->
  <SectionCard
    title={$w.steamcmdPath.value}
    description={$w.steamcmdOverride.value}
    icon={FolderCog}
  >
    <Field label={$w.steamcmdPath.value} hint={$w.leaveBlank.value} stacked>
      <PathInput
        bind:value={wizard.steamcmdPath}
        placeholder={wizard.status?.path ?? $w.autoDetect.value}
        title={$w.selectSteamcmd.value}
        filters={win ? [{ name: $w.steamcmdFilter.value, extensions: ["exe"] }] : []}
        onpick={(p) => wizard.chose(p)}
      />
    </Field>
  </SectionCard>
</div>

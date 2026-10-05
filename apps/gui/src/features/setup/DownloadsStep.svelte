<script lang="ts">
  import { dict } from "$lib/i18n";
  import Terminal from "~icons/lucide/square-terminal";
  import Gamepad from "~icons/lucide/gamepad-2";
  import CircleCheck from "~icons/lucide/circle-check";
  import CircleX from "~icons/lucide/circle-x";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { cn } from "$lib/cx";
  import type { ModDownloaderDto } from "$lib/ipc/bindings";
  import { wizard } from "./wizard.svelte";
  import SteamcmdStep from "./SteamcmdStep.svelte";

  /**
   * How mods will download: SteamCMD (its own tool, a Steam login once) or
   * the Steam client (nothing to set up, Steam open). What the choice needs
   * follows it on the same page.
   */
  const w = dict("setup");

  const CHOICES = $derived<
    { value: ModDownloaderDto; title: string; desc: string; icon: typeof Terminal }[]
  >([
    { value: "steamcmd", title: "SteamCMD", desc: $w.dlSteamcmdDesc.value, icon: Terminal },
    { value: "steamworks", title: $w.dlSteam.value, desc: $w.dlSteamDesc.value, icon: Gamepad },
  ]);
</script>

<div class="space-y-3">
  <div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label={$w.downloadsTitle.value}>
    {#each CHOICES as c (c.value)}
      {@const on = wizard.downloader === c.value}
      {@const Icon = c.icon}
      <button
        type="button"
        role="radio"
        aria-checked={on}
        class={cn(
          "flex flex-col items-start gap-1.5 rounded-md border px-3 py-3 text-left transition-colors",
          on
            ? "border-accent bg-accent/8"
            : "border-border bg-panel hover:border-border-strong hover:bg-raised/40",
        )}
        onclick={() => wizard.choose(c.value)}
      >
        <span class="flex w-full items-center gap-2">
          <Icon class={cn("size-icon", on ? "text-accent" : "text-fg-faint")} />
          <span class={cn("text-sm font-semibold", on ? "text-fg" : "text-fg-muted")}
            >{c.title}</span
          >
          <span
            class={cn(
              "ml-auto grid size-4 place-items-center rounded-full border",
              on ? "border-accent" : "border-border-strong",
            )}
          >
            {#if on}<span class="size-2 rounded-full bg-accent"></span>{/if}
          </span>
        </span>
        <span class="text-2xs leading-snug text-fg-muted">{c.desc}</span>
      </button>
    {/each}
  </div>

  {#if wizard.viaSteam}
    {@const st = wizard.steam}
    <div
      class={cn(
        "flex items-center gap-3 rounded-md border px-3 py-2.5",
        wizard.checkingSteam || !st
          ? "border-border bg-raised/40"
          : st.library && st.steam_running
            ? "border-ok/40 bg-ok/8"
            : "border-warn/40 bg-warn/8",
      )}
    >
      {#if wizard.checkingSteam || !st}
        <Spinner class="size-icon-lg text-accent" />
      {:else if st.library && st.steam_running}
        <CircleCheck class="size-icon-lg shrink-0 text-ok" />
        <span class="min-w-0 flex-1 text-xs text-ok">{$w.steamOk.value}</span>
      {:else}
        <CircleX class="size-icon-lg shrink-0 text-warn" />
        <span class="min-w-0 flex-1 text-xs text-warn" data-selectable>
          {st.library ? $w.steamClosed.value : $w.steamLibFail({ error: st.error ?? "" }).value}
        </span>
      {/if}
      {#if st && !wizard.checkingSteam}
        <Button size="xs" variant="ghost" onclick={() => wizard.checkSteam()}>
          <RefreshCw class="size-icon-sm" />{$w.checkAgain.value}
        </Button>
      {/if}
    </div>
  {:else}
    <SteamcmdStep />
  {/if}
</div>

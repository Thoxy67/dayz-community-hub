<script lang="ts">
  import { dict } from "$lib/i18n";
  import CircleCheck from "~icons/lucide/circle-check";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Puzzle from "~icons/lucide/puzzle";
  import FolderSearch from "~icons/lucide/folder-search";
  import { Button } from "$lib/components/ui/button";
  import { Field } from "$lib/components/ui/field";
  import { Spinner } from "$lib/components/ui/spinner";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import PathInput from "$lib/components/app/PathInput.svelte";
  import { num } from "$lib/format";
  import { cn } from "$lib/cx";
  import { wizard } from "./wizard.svelte";

  /**
   * Where DayZ is: found on its own in any Steam library on any drive, or in
   * the library the player picks. Not finding it does not block the setup.
   */
  const w = dict("setup");
  const g = $derived(wizard.game);
  const win = $derived(wizard.platform === "windows");

  // A typed path is tried once the typing stops.
  let first = true;
  $effect(() => {
    void wizard.steamRoot;
    if (first) return void (first = false);
    const t = setTimeout(() => void wizard.findGame(), 500);
    return () => clearTimeout(t);
  });
</script>

<div class="space-y-3">
  <div
    class={cn(
      "flex items-center gap-3 rounded-md border px-3 py-2.5",
      wizard.findingGame || !g
        ? "border-border bg-raised/40"
        : g.dayz_dir
          ? "border-ok/40 bg-ok/8"
          : "border-warn/40 bg-warn/8",
    )}
  >
    {#if wizard.findingGame || !g}
      <Spinner class="size-icon-lg text-accent" />
      <span class="text-xs text-fg-muted">{$w.gameFinding.value}</span>
    {:else if g.dayz_dir}
      <CircleCheck class="size-icon-lg shrink-0 text-ok" />
      <div class="min-w-0 flex-1">
        <p class="m-0 text-xs font-semibold text-ok">{$w.gameFound.value}</p>
        <p class="m-0 truncate font-mono text-2xs text-fg-muted" data-selectable>
          {g.dayz_dir}
        </p>
        {#if g.workshop_mods > 0}
          <p class="m-0 mt-0.5 flex items-center gap-1 text-2xs text-mods">
            <Puzzle class="size-3" />{$w.gameMods({ count: num(g.workshop_mods) }).value}
          </p>
        {/if}
      </div>
    {:else}
      <TriangleAlert class="size-icon-lg shrink-0 text-warn" />
      <div class="min-w-0 flex-1">
        <p class="m-0 text-xs font-semibold text-warn">{$w.gameNotFound.value}</p>
        <p class="m-0 text-2xs leading-snug text-fg-muted">{$w.gameNotFoundHint.value}</p>
      </div>
    {/if}
    {#if g && !wizard.findingGame}
      <Button size="xs" variant="ghost" onclick={() => wizard.findGame()}>
        <RefreshCw class="size-icon-sm" />{$w.rescan.value}
      </Button>
    {/if}
  </div>

  <SectionCard title={$w.gameLibrary.value} icon={FolderSearch}>
    <Field
      label={$w.steamRoot.value}
      hint={`${$w.gameLibraryHint.value} ${
        win
          ? $w.windowsPathHint({ path: "D:\\SteamLibrary" }).value
          : $w.linuxPathHint({ path: "/mnt/games/SteamLibrary" }).value
      }`}
      stacked
    >
      <PathInput
        bind:value={wizard.steamRoot}
        placeholder={g?.steamapps ?? $w.autoDetect.value}
        directory
        title={$w.selectSteamRoot.value}
      />
    </Field>
  </SectionCard>
</div>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import ShieldCheck from "~icons/lucide/shield-check";
  import Wrench from "~icons/lucide/wrench";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { SettingsSection as Section } from "$lib/components/app";
  import { gameDataDirs } from "$lib/ipc/launch";
  import { battleye } from "$lib/stores/battleye.svelte";
  import { confirm } from "$lib/stores/dialogs.svelte";
  import { cn } from "$lib/cx";

  /**
   * BattlEye's state, and the thorough repair: the game's data outside its
   * folder deleted, then BattlEye reinstalled. The state's words are the
   * status bar's.
   */
  const s = dict("settings");
  const sh = dict("shell");

  $effect(() => void battleye.refresh());

  const be = $derived(battleye.status?.state ?? "no_game");
  const service = $derived(battleye.status?.service ?? false);
  const said = $derived(
    be === "running"
      ? $sh.sbBeRunning.value
      : be === "disabled"
        ? $sh.sbBeDisabled.value
        : be === "missing"
          ? service
            ? $sh.sbBeMissing.value
            : $sh.sbBeMissingProton.value
          : be === "ready"
            ? service
              ? $sh.sbBeReady.value
              : $sh.sbBeReadyProton.value
            : "",
  );

  async function repair() {
    let dirs: string[] = [];
    try {
      dirs = await gameDataDirs();
    } catch {
      // Asked again by the repair itself.
    }
    const yes = await confirm({
      title: $s.battleyeConfirmTitle.value,
      message: dirs.length
        ? $s.battleyeConfirmMessage({ dirs: dirs.join(", ") }).value
        : $s.battleyeConfirmNoData.value,
      confirmLabel: $s.battleyeConfirmAction.value,
      danger: true,
    });
    if (yes) await battleye.repair(true);
  }
</script>

<Section
  id="battleye"
  title={$s.sectionBattleye.value}
  description={$s.battleyeHint.value}
  icon={ShieldCheck}
>
  {#if said}
    <div class="flex items-center gap-2 border-b border-border/60 px-pad py-2.5 text-xs">
      <span
        class={cn(
          "size-1.5 shrink-0 rounded-full",
          be === "running" ? "bg-ok" : be === "ready" ? "bg-fg-faint" : "bg-warn",
        )}
        aria-hidden="true"
      ></span>
      <span class={be === "missing" || be === "disabled" ? "text-warn" : "text-fg-muted"}
        >{said}</span
      >
    </div>
  {/if}
  <div class="flex items-center gap-3 px-pad py-2.5">
    <div class="min-w-0 flex-1">
      <p class="m-0 text-xs font-medium text-fg">{$s.battleyeRepair.value}</p>
      <p class="m-0 text-2xs text-fg-faint">{$s.battleyeRepairDesc.value}</p>
    </div>
    <Button
      variant="danger"
      disabled={battleye.repairing || be === "no_game"}
      onclick={() => void repair()}
    >
      {#if battleye.repairing}<Spinner class="size-3" />{:else}<Wrench class="size-icon-sm" />{/if}
      {$s.battleyeRepair.value}
    </Button>
  </div>
</Section>

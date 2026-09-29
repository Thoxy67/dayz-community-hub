<script lang="ts">
  import { dict } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import FolderOpen from "~icons/lucide/folder-open";
  import ExternalLink from "~icons/lucide/external-link";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Trash from "~icons/lucide/trash-2";
  import Wrench from "~icons/lucide/wrench";
  import History from "~icons/lucide/history";
  import MapPin from "~icons/lucide/map-pin";
  import Gamepad from "~icons/lucide/gamepad-2";
  import Hash from "~icons/lucide/hash";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Copy } from "$lib/components/ui/copy";
  import { Meter } from "$lib/components/ui/meter";
  import { MiniSwitch } from "$lib/components/ui/switch";
  import { Section, DetailList, DetailRow } from "$lib/components/app";
  import { bytes, date, relative } from "$lib/format";
  import { openUrl } from "$lib/ipc/native";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { review, workshopUrl } from "./review.svelte";
  import SourceTag from "./SourceTag.svelte";
  import ModState from "./ModState.svelte";

  /**
   * One installed mod, top to bottom in the order a player asks about it:
   * is it current (and the button that makes it so), where it is, whether
   * the game loads it, its ids; repairing and deleting last.
   */
  let { mod, onclose }: { mod: InstalledModDto; onclose?: () => void } = $props();
  const m = dict("mods");

  const share = $derived(mods.totalSize > 0 ? mod.size / mods.totalSize : 0);
  const steam = $derived(mod.source === "steam");
  const days = $derived(
    mod.remote_updated && mod.remote_updated > mod.local_updated
      ? Math.floor((mod.remote_updated - mod.local_updated) / 86400)
      : 0,
  );
  const busy = $derived(mods.opState(mod.id) !== null);
  /** "3 days ago" says more than a date for a month; past that it only says "long ago". */
  const recent = (ts: number) => Date.now() / 1000 - ts < 30 * 86400;
</script>

<div class="flex min-h-0 flex-1 flex-col bg-panel">
  <header class="border-b border-border px-pad py-3">
    <div class="flex items-start gap-2">
      <h2
        class="m-0 min-w-0 flex-1 text-base leading-tight font-semibold break-words text-fg"
        data-selectable
      >
        {mod.name}
      </h2>
      {#if onclose}
        <IconButton
          icon={X}
          size="icon-xs"
          label={$m.closeDetail.value}
          kbd="Esc"
          onclick={onclose}
        />
      {/if}
    </div>
    <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
      <ModState {mod} />
      <SourceTag {mod} />
      <span class="font-mono text-2xs text-fg-faint">{mod.size_human}</span>
    </div>
    <div class="mt-2.5 flex flex-wrap gap-1.5">
      <Button
        size="sm"
        variant={mod.update_available ? "accent" : "default"}
        disabled={busy}
        onclick={() => review.updateSelected([mod.id])}
      >
        <RefreshCw class="size-icon-sm" />{mod.update_available
          ? $m.update.value
          : $m.redownload.value}
      </Button>
      <Button size="sm" title={$m.openWorkshop.value} onclick={() => openUrl(workshopUrl(mod.id))}>
        <ExternalLink class="size-icon-sm" />Workshop
      </Button>
      <IconButton
        icon={FolderOpen}
        label={$m.openModFolder.value}
        onclick={() => mods.openModDir(mod.id)}
      />
    </div>
  </header>

  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
    <Section title={$m.sectionVersion.value} icon={History}>
      <DetailList>
        <DetailRow label={$m.yourCopy.value}>
          <span class="font-mono text-fg">{date(mod.local_updated * 1000)}</span>
          {#if recent(mod.local_updated)}<span class="ml-1.5 text-2xs text-fg-faint"
              >{relative(mod.local_updated)}</span
            >{/if}
        </DetailRow>
        <DetailRow label={$m.workshopCopy.value}>
          {#if mod.remote_updated}
            <span class={["font-mono", mod.update_available ? "text-warn" : "text-fg"]}
              >{date(mod.remote_updated * 1000)}</span
            >
            {#if mod.update_available}<span class="ml-1.5 font-mono text-2xs text-warn/80"
                >{days > 0 ? $m.daysBehind({ days }).value : $m.behindToday.value}</span
              >{:else if recent(mod.remote_updated)}<span class="ml-1.5 text-2xs text-fg-faint"
                >{relative(mod.remote_updated)}</span
              >{/if}
          {:else}<span class="text-fg-faint">{$m.detailsUnknown.value}</span>{/if}
        </DetailRow>
      </DetailList>
      {#if steam}
        <p class="m-0 text-2xs leading-snug text-fg-faint">{$m.steamKeepsUpToDate.value}</p>
      {/if}
    </Section>

    <Section title={$m.sectionWhere.value} icon={MapPin}>
      <p class="m-0 text-xs leading-snug text-fg-muted">
        {steam ? $m.sourceSteamHint.value : $m.sourceLauncherHint.value}
      </p>
      <div class="flex items-center gap-1">
        <Copy text={mod.path} class="min-w-0 flex-1" />
        <IconButton
          icon={FolderOpen}
          size="icon-xs"
          label={$m.openModFolder.value}
          onclick={() => mods.openModDir(mod.id)}
        />
      </div>
      {#if mod.other_copy}
        <p class="m-0 text-2xs text-fg-faint">{$m.otherCopy.value}</p>
      {/if}
      <div class="flex items-center gap-2">
        <Meter class="flex-1" value={share} max={1} tone="muted" label={$m.detailsShare.value} />
        <span class="font-mono text-2xs whitespace-nowrap text-fg-muted"
          >{bytes(mod.size)} · {(share * 100).toFixed(1)}%</span
        >
      </div>
    </Section>

    <Section title={$m.colInGame.value} icon={Gamepad}>
      <label class="flex cursor-pointer items-center justify-between gap-2">
        <span class="text-xs font-medium text-fg"
          >{mod.managed ? $m.inGameLinked.value : $m.inGameNotLinked.value}</span
        >
        <MiniSwitch
          bind:checked={() => mod.managed, () => void mods.toggleManaged(mod)}
          aria-label={$m.detailsLink.value}
        />
      </label>
      <p class="m-0 text-2xs leading-snug text-fg-faint">{$m.detailsLinkHint.value}</p>
    </Section>

    <Section title={$m.sectionIds.value} icon={Hash}>
      <DetailList>
        <DetailRow label={$m.colWorkshopId.value}><Copy text={String(mod.id)} /></DetailRow>
        <DetailRow label={$m.detailsLaunchParam.value}><Copy text={`@${mod.id}`} /></DetailRow>
      </DetailList>
    </Section>

    <Section title={$m.sectionCare.value} icon={Wrench}>
      <div class="flex flex-wrap gap-1.5">
        <Button
          size="sm"
          title={$m.repairHint.value}
          disabled={busy}
          onclick={() => mods.repair(mod)}
        >
          <Wrench class="size-icon-sm" />{$m.repair.value}
        </Button>
        <Button size="sm" variant="danger" onclick={() => mods.remove(mod)}>
          <Trash class="size-icon-sm" />{$m.delete.value}
        </Button>
      </div>
      <p class="m-0 text-2xs leading-snug text-fg-faint">{$m.deleteHint.value}</p>
    </Section>
  </div>
</div>

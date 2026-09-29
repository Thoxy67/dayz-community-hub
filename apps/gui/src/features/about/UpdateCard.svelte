<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Download from "~icons/lucide/download";
  import CircleCheck from "~icons/lucide/circle-check";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import PackageCheck from "~icons/lucide/package-check";
  import { Button } from "$lib/components/ui/button";
  import { Meter } from "$lib/components/ui/meter";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Tag } from "$lib/components/ui/tag";
  import { cn } from "$lib/cx";
  import { updater } from "$lib/stores/updater.svelte";
  import { bytes, date } from "$lib/format";

  /** The launcher's own version against the newest release, and installing it. */
  let { version, highlight = false }: { version: string; highlight?: boolean } = $props();
  const a = useIntlayer("about");

  // The backend answers "use your package manager" where it cannot update in place.
  const packaged = $derived(updater.state === "error" && /package|paquet|manager/i.test(updater.error));
</script>

<div
  id="update"
  class={cn(
    "rounded-md border bg-bg/60 transition-shadow",
    highlight ? "border-accent shadow-glow" : updater.state === "available" ? "border-ok/50" : "border-border",
  )}
>
  <div class="flex items-center gap-2 border-b border-border/60 px-pad py-2">
    <span class="label-stencil text-fg-muted">{$a.updates.value}</span>
    <Tag>{$a.version.value} {version || "—"}</Tag>
    <Button
      variant="ghost"
      size="xs"
      class="ml-auto"
      onclick={() => updater.check()}
      disabled={updater.state === "checking" || updater.state === "downloading"}
    >
      <RefreshCw class={cn("size-3", updater.state === "checking" && "animate-spin")} />{$a.checkUpdates.value}
    </Button>
  </div>
  <div class="px-pad py-3">
    {#if updater.state === "checking" || updater.state === "idle"}
      <p class="m-0 flex items-center gap-2 text-xs text-fg-muted"><Spinner />{$a.updatesChecking.value}</p>
    {:else if updater.state === "up_to_date"}
      <p class="m-0 flex items-center gap-2 text-sm text-ok"><CircleCheck class="size-icon" />{$a.updatesUpToDate.value}</p>
      <p class="m-0 mt-0.5 text-2xs text-fg-faint">{$a.updatesLatest({ version }).value}</p>
    {:else if updater.state === "available" && updater.info}
      <div class="flex items-start gap-3">
        <Download class="mt-0.5 size-icon-lg shrink-0 text-ok" />
        <div class="min-w-0 flex-1">
          <p class="m-0 text-sm text-fg">
            {$a.updatesAvailable.value}
            <span class="title-display text-lg text-ok">v{updater.info.version}</span>
          </p>
          <p class="m-0 font-mono text-2xs text-fg-faint">
            {$a.updatesCurrent.value} v{updater.info.currentVersion}
            {#if updater.info.date}· {$a.updatesReleased({ date: date(updater.info.date) }).value}{/if}
          </p>
          {#if updater.info.body}
            <pre
              class="m-0 mt-2 max-h-40 overflow-y-auto rounded-sm border border-border bg-plot p-2 font-mono text-2xs whitespace-pre-wrap text-fg-muted"
              data-selectable>{updater.info.body}</pre>
          {/if}
        </div>
        <Button variant="play" size="lg" onclick={() => updater.install()}>
          <Download class="size-icon-sm" />{$a.updatesInstall.value}
        </Button>
      </div>
    {:else if updater.state === "downloading"}
      <p class="m-0 mb-2 text-xs text-fg">{$a.updatesDownloading({ version: updater.info?.version ?? "" }).value}</p>
      <Meter value={updater.percent} max={100} size="md" label={$a.updatesDownloading({ version: updater.info?.version ?? "" }).value} />
      <p class="m-0 mt-1 flex justify-between font-mono text-2xs text-fg-faint">
        <span>{bytes(updater.received)}{updater.total ? ` / ${bytes(updater.total)}` : ""}</span>
        <span>{updater.percent}%</span>
      </p>
    {:else if updater.state === "done"}
      <p class="m-0 flex items-center gap-2 text-sm text-ok"><PackageCheck class="size-icon" />{$a.updatesDone.value}</p>
      <p class="m-0 mt-0.5 text-2xs text-fg-faint">{$a.updatesDoneHint.value}</p>
    {:else if packaged}
      <p class="m-0 text-xs text-fg-muted">{$a.updatesPackageManager.value}</p>
    {:else}
      <p class="m-0 flex items-start gap-2 text-xs text-err">
        <TriangleAlert class="mt-0.5 size-icon-sm shrink-0" /><span class="font-mono text-2xs" data-selectable>{updater.error}</span>
      </p>
      <Button class="mt-2" onclick={() => updater.check()}><RefreshCw class="size-3" />{$a.updatesRetry.value}</Button>
    {/if}
  </div>
</div>

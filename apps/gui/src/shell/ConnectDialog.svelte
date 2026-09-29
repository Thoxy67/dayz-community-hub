<script lang="ts">
  import { dict } from "$lib/i18n";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import ExternalLink from "~icons/lucide/external-link";
  import Play from "~icons/lucide/play";
  import { Dialog } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import { Tag } from "$lib/components/ui/tag";
  import { connect } from "$lib/stores/connect.svelte";
  import { openUrl } from "$lib/ipc/native";
  import { date } from "$lib/format";

  /**
   * Before joining a modded server: which mods it runs that are missing here,
   * or which are here and may be behind, each with its dates and size, and
   * whether to fetch them first. One dialog, where there used to be two.
   */
  const c = dict("connect");
  const m = dict("mods");

  const req = $derived(connect.request);
  let fetch = $state(false);
  $effect(() => {
    if (req) fetch = req.kind === "missing";
  });

  const workshop = (id: number) => `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`;
  const newerDays = (l?: number, r?: number | null) =>
    l && r && r > l ? Math.floor((r - l) / 86400) : 0;

  function go() {
    const r = req;
    if (!r) return;
    connect.request = null;
    r.go(fetch);
  }

  const count = $derived(req?.mods.length ?? 0);
  const title = $derived(
    !req
      ? ""
      : req.kind === "missing"
        ? (count === 1
            ? $c.connectModalModsNotInstalled({ count })
            : $c.connectModalModsNotInstalledPlural({ count })
          ).value
        : (count === 1
            ? $c.connectModalModsToCheck({ count })
            : $c.connectModalModsToCheckPlural({ count })
          ).value,
  );
</script>

<Dialog
  bind:open={() => req !== null, (v) => !v && (connect.request = null)}
  title={$c.connectModalTitle.value}
  description={req?.serverName ?? ""}
  size="md"
  closeLabel={$c.connectModalCancel.value}
>
  {#if req}
    <div class="mb-2 flex items-center gap-2">
      <Tag tone={req.kind === "missing" ? "warn" : "neutral"}>{title}</Tag>
    </div>
    <ul
      class="m-0 max-h-72 list-none divide-y divide-border/60 overflow-y-auto rounded-sm border border-border bg-bg p-0"
    >
      {#each req.mods as mod (mod.id)}
        {@const days = newerDays(mod.local_updated, mod.remote_updated)}
        <li class="flex items-center gap-2 px-2 py-1.5">
          <div class="min-w-0 flex-1">
            <div class="truncate text-xs font-medium text-fg">{mod.name}</div>
            <div class="flex flex-wrap gap-x-3 font-mono text-3xs text-fg-faint">
              <span>#{mod.id}</span>
              {#if mod.local_updated}<span
                  >{$m.confirmLocalDate.value} {date(mod.local_updated * 1000)}</span
                >{/if}
              {#if mod.remote_updated}<span
                  >{$m.confirmRemoteDate.value} {date(mod.remote_updated * 1000)}</span
                >{/if}
              {#if mod.size_human}<span>{mod.size_human}</span>{/if}
            </div>
          </div>
          {#if days > 0}<Tag tone="warn">{$m.confirmNewer({ days }).value}</Tag>{/if}
          <button
            class="grid size-control-sm shrink-0 place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            title={$m.confirmOpenWorkshop.value}
            aria-label={$m.confirmOpenWorkshop.value}
            onclick={() => openUrl(workshop(mod.id))}><ExternalLink class="size-icon-sm" /></button
          >
        </li>
      {/each}
    </ul>
    <div class="mt-3 rounded-sm border border-border bg-bg">
      <Switch
        bind:checked={fetch}
        label={req.kind === "missing"
          ? $c.connectModalInstallMods.value
          : $c.connectModalUpdateMods.value}
        hint={fetch
          ? $m.confirmWarning.value
          : req.kind === "missing"
            ? $c.connectModalWarning.value
            : ""}
      />
    </div>
    {#if !fetch && req.kind === "missing"}
      <p class="m-0 mt-2 flex items-center gap-1.5 text-2xs text-warn">
        <TriangleAlert class="size-3.5 shrink-0" />{$c.connectModalWarning.value}
      </p>
    {/if}
  {/if}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (connect.request = null)}
      >{$c.connectModalCancel.value}</Button
    >
    <Button variant="play" size="lg" onclick={go}>
      <Play class="size-icon-sm" />
      {fetch
        ? req?.kind === "missing"
          ? $c.connectModalInstallConnect.value
          : $c.connectModalUpdateConnect.value
        : $c.connectModalConnect.value}
    </Button>
  {/snippet}
</Dialog>

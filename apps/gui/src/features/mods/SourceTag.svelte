<script lang="ts">
  import { dict } from "$lib/i18n";
  import { SteamIcon } from "$lib/components/ui/brand";
  import HardDrive from "~icons/lucide/hard-drive-download";
  import { Tag } from "$lib/components/ui/tag";
  import { cn } from "$lib/cx";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { whereOf } from "./where";

  /**
   * Whose copy a mod is: the launcher's folder, a Steam subscription, or a
   * Steam library copy nobody is subscribed to. As a tag (the details'
   * header) or as the quiet line under a name in the list (`line`).
   */
  let {
    mod,
    line = false,
  }: { mod: Pick<InstalledModDto, "id" | "source" | "other_copy">; line?: boolean } = $props();
  const m = dict("mods");

  const item = $derived(mods.steamById.get(mod.id));
  const where = $derived(whereOf(mod, item, !!mods.steam?.available));
  /** The mods that require this one. */
  const parents = $derived(mods.requiredBy.get(mod.id) ?? []);
  const label = $derived(
    {
      launcher: line ? $m.whereLauncher.value : $m.sourceLauncher.value,
      subscribed: line ? $m.whereSubscribed.value : $m.sourceSubscribed.value,
      unsubscribed: line ? $m.whereNotSubscribed.value : $m.sourceSteam.value,
      steam: line ? $m.whereSteam.value : $m.sourceSteam.value,
    }[where] +
      (line && where === "launcher" && item?.subscribed ? ` · ${$m.alsoSubscribed.value}` : "") +
      (line && parents.length > 0
        ? ` · ${$m.requiredByLine({ name: mods.nameOf(parents[0]!) }).value}` +
          (parents.length > 1 ? ` +${parents.length - 1}` : "")
        : ""),
  );
  const title = $derived(
    [
      {
        launcher: $m.sourceLauncherHint.value,
        subscribed: $m.subscribedHint.value,
        unsubscribed: $m.notSubscribedHint.value,
        steam: $m.sourceSteamHint.value,
      }[where],
      mod.other_copy ? $m.otherCopy.value : "",
    ]
      .filter(Boolean)
      .join("\n"),
  );
</script>

{#snippet icon()}
  {#if mod.source === "steam"}<SteamIcon class="size-2.5 shrink-0" />{:else}<HardDrive
      class="size-2.5 shrink-0"
    />{/if}
{/snippet}

{#if line}
  <span
    class={cn(
      "flex min-w-0 items-center gap-1 text-2xs",
      where === "subscribed" ? "text-fg-muted" : "text-fg-faint",
    )}
    {title}
  >
    {@render icon()}<span class="truncate">{label}</span>
  </span>
{:else}
  <Tag tone={where === "subscribed" ? "accent" : "neutral"} {title} class="shrink-0">
    {@render icon()}{label}
  </Tag>
{/if}

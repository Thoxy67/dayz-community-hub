<script lang="ts">
  import { dict } from "$lib/i18n";
  import { SteamIcon } from "$lib/components/ui/brand";
  import HardDrive from "~icons/lucide/hard-drive-download";
  import { Tag } from "$lib/components/ui/tag";
  import type { InstalledModDto } from "$lib/ipc/types";

  /**
   * Whose folder a mod is in: the launcher's (it updates and deletes it) or a
   * Steam library's (read only).
   */
  let { mod }: { mod: Pick<InstalledModDto, "source" | "other_copy"> } = $props();
  const m = dict("mods");
  const steam = $derived(mod.source === "steam");
  const title = $derived(
    `${steam ? $m.sourceSteamHint.value : $m.sourceLauncherHint.value}${mod.other_copy ? `\n${$m.otherCopy.value}` : ""}`,
  );
</script>

<Tag tone={steam ? "accent" : "neutral"} {title} class="shrink-0">
  {#if steam}<SteamIcon class="size-2.5" />{:else}<HardDrive class="size-2.5" />{/if}
  {steam ? $m.sourceSteam.value : $m.sourceLauncher.value}
</Tag>

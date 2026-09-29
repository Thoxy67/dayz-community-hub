<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Ellipsis from "~icons/lucide/ellipsis";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Wrench from "~icons/lucide/wrench";
  import FolderOpen from "~icons/lucide/folder-open";
  import ExternalLink from "~icons/lucide/external-link";
  import Hash from "~icons/lucide/hash";
  import Link from "~icons/lucide/link";
  import Unlink from "~icons/lucide/unlink";
  import Trash from "~icons/lucide/trash-2";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import {
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
  } from "$lib/components/ui/dropdown-menu";
  import { copyText, openUrl } from "$lib/ipc/native";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { review, workshopUrl } from "./review.svelte";

  /**
   * Everything that can be done to one mod, behind a "⋯": the list keeps
   * only the update button on each row, the rest waits here.
   */
  let { mod }: { mod: InstalledModDto } = $props();
  const m = dict("mods");
</script>

<Menu.Root>
  <Tooltip text={$m.moreActions.value} side="left">
    <Menu.Trigger
      class="grid size-control-sm place-items-center rounded-md text-fg-muted hover:bg-raised hover:text-fg data-[state=open]:bg-raised data-[state=open]:text-fg"
      aria-label={$m.moreActions.value}
      onclick={(e: MouseEvent) => e.stopPropagation()}
    >
      <Ellipsis class="size-icon-sm" />
    </Menu.Trigger>
  </Tooltip>
  <DropdownMenuContent>
    <DropdownMenuItem icon={RefreshCw} kbd="U" onselect={() => review.updateSelected([mod.id])}>
      {mod.update_available ? $m.update.value : $m.redownload.value}
    </DropdownMenuItem>
    <DropdownMenuItem icon={Wrench} onselect={() => mods.repair(mod)}>
      {$m.repair.value}
    </DropdownMenuItem>
    <DropdownMenuSeparator />
    <DropdownMenuItem
      icon={mod.managed ? Unlink : Link}
      kbd="M"
      onselect={() => mods.toggleManaged(mod)}
    >
      {mod.managed ? $m.unlinkAction.value : $m.linkAction.value}
    </DropdownMenuItem>
    <DropdownMenuItem icon={FolderOpen} onselect={() => mods.openModDir(mod.id)}>
      {$m.openModFolder.value}
    </DropdownMenuItem>
    <DropdownMenuItem icon={ExternalLink} onselect={() => openUrl(workshopUrl(mod.id))}>
      {$m.openWorkshop.value}
    </DropdownMenuItem>
    <DropdownMenuItem icon={Hash} onselect={() => copyText(String(mod.id))}>
      {$m.copyId.value}
    </DropdownMenuItem>
    <DropdownMenuSeparator />
    <DropdownMenuItem icon={Trash} tone="danger" kbd="Del" onselect={() => mods.remove(mod)}>
      {$m.delete.value}
    </DropdownMenuItem>
  </DropdownMenuContent>
</Menu.Root>

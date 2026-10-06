<script lang="ts">
  import { dict } from "$lib/i18n";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Wrench from "~icons/lucide/wrench";
  import FolderOpen from "~icons/lucide/folder-open";
  import ExternalLink from "~icons/lucide/external-link";
  import Hash from "~icons/lucide/hash";
  import Link from "~icons/lucide/link";
  import Unlink from "~icons/lucide/unlink";
  import Trash from "~icons/lucide/trash-2";
  import { DropdownMenuItem, DropdownMenuSeparator } from "$lib/components/ui/dropdown-menu";
  import { ContextMenuItem, ContextMenuSeparator } from "$lib/components/ui/context-menu";
  import { copyText, openUrl } from "$lib/ipc/native";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { review, workshopUrl } from "./review.svelte";

  /**
   * Everything that can be done to one mod: the items of its "⋯" menu and
   * of the menu a right-click on its row opens (`context`), one list.
   */
  let { mod, context = false }: { mod: InstalledModDto; context?: boolean } = $props();
  const m = dict("mods");
  const Item = $derived(context ? ContextMenuItem : DropdownMenuItem);
  const Separator = $derived(context ? ContextMenuSeparator : DropdownMenuSeparator);
</script>

<Item icon={RefreshCw} kbd="U" onselect={() => review.updateSelected([mod.id])}>
  {mod.update_available ? $m.update.value : $m.redownload.value}
</Item>
<Item icon={Wrench} onselect={() => mods.repair(mod)}>
  {$m.repair.value}
</Item>
<Separator />
<Item icon={mod.managed ? Unlink : Link} kbd="M" onselect={() => mods.toggleManaged(mod)}>
  {mod.managed ? $m.unlinkAction.value : $m.linkAction.value}
</Item>
<Item icon={FolderOpen} onselect={() => mods.openModDir(mod.id)}>
  {$m.openModFolder.value}
</Item>
<Item icon={ExternalLink} onselect={() => openUrl(workshopUrl(mod.id))}>
  {$m.openWorkshop.value}
</Item>
<Item icon={Hash} onselect={() => copyText(String(mod.id))}>
  {$m.copyId.value}
</Item>
<Separator />
<Item icon={Trash} tone="danger" kbd="Del" onselect={() => mods.remove(mod)}>
  {$m.delete.value}
</Item>

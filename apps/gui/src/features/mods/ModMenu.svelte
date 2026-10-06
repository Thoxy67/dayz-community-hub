<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Ellipsis from "~icons/lucide/ellipsis";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { DropdownMenuContent } from "$lib/components/ui/dropdown-menu";
  import type { InstalledModDto } from "$lib/ipc/types";
  import ModMenuItems from "./ModMenuItems.svelte";

  /**
   * Everything that can be done to one mod, behind a "⋯": the list keeps
   * only the update button on each row, the rest waits here (the same items
   * a right-click on the row opens: ModMenuItems).
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
    <ModMenuItems {mod} />
  </DropdownMenuContent>
</Menu.Root>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Dialog } from "$lib/components/ui/dialog";
  import { ShortcutList } from "$lib/components/app";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /** `?`: every keyboard shortcut, in two columns, over whatever is on screen. */
  const a = dict("about");
  const cm = dict("common");
</script>

<Dialog
  bind:open={dialogs.shortcuts}
  title={$a.shortcuts.value}
  size="lg"
  closeLabel={$cm.close.value}
>
  <ShortcutList class="grid grid-cols-2 gap-x-6 max-[640px]:grid-cols-1" />
</Dialog>

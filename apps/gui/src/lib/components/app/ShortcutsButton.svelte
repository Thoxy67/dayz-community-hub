<script lang="ts">
  import { dict } from "$lib/i18n";
  import Keyboard from "~icons/lucide/keyboard";
  import { IconButton } from "$lib/components/ui/button";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /** Opens the sheet of keyboard shortcuts, where a view used to spell them out in a line. */
  const s = dict("shell");
</script>

<IconButton
  icon={Keyboard}
  label={$s.paletteShortcuts.value}
  kbd="?"
  onclick={() => (dialogs.shortcuts = true)}
/>

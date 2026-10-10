<script lang="ts">
  import { dict } from "$lib/i18n";
  import Keyboard from "~icons/lucide/keyboard";
  import { IconButton } from "$lib/components/ui/button";
  import { keyText } from "$lib/shortcuts/registry";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /** Opens the sheet of shortcuts, where a view used to spell them out in a line. */
  const k = dict("keys");
</script>

<IconButton
  icon={Keyboard}
  label={$k.sheet.value}
  kbd={keyText("sheet")}
  onclick={() => (dialogs.shortcuts = true)}
/>

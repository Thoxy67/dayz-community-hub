<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Dialog } from "$lib/components/ui/dialog";
  import { ShortcutList } from "$lib/components/app";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /**
   * `?` (or View on a controller): every shortcut, keyboard or controller,
   * over whatever is on screen. The same list as the About page.
   */
  const k = dict("keys");

  // Screenshot aid, in mock builds only: `?shortcuts=1` (`=pad`: its controller tab) opens it.
  let mode = $state<"keys" | "pad" | undefined>(undefined);
  if (import.meta.env.VITE_MOCK === "1") {
    const want = new URLSearchParams(location.search).get("shortcuts");
    if (want) {
      if (want === "pad") mode = "pad";
      void (async () => {
        const { app } = await import("$lib/stores/app.svelte");
        while (!app.initialized) await new Promise((r) => setTimeout(r, 100));
        await new Promise((r) => setTimeout(r, 800));
        dialogs.shortcuts = true;
      })();
    }
  }
  const cm = dict("common");
</script>

<Dialog bind:open={dialogs.shortcuts} title={$k.title.value} size="lg" closeLabel={$cm.close.value}>
  {#key mode}<ShortcutList
      mode={mode ?? undefined}
      class="block columns-2 gap-x-6 space-y-3 max-[640px]:columns-1"
    />{/key}
</Dialog>

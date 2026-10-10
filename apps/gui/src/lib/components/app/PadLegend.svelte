<script lang="ts">
  import ShortcutList from "./ShortcutList.svelte";

  /**
   * What a controller's buttons do, in the glyphs of the one plugged in:
   * the registry's controller shortcuts, without the keyboard tab.
   */
  let { class: klass = "" }: { class?: string } = $props();
</script>

<ShortcutList mode="pad" tabs={false} class={klass} />

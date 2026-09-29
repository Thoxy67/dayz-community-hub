<script lang="ts">
  import TabIcon from "./TabIcon.svelte";
  let { class: klass = "" }: { class?: string } = $props();
</script>

<TabIcon class={klass} tone="warn" />

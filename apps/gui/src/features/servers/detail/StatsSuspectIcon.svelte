<script lang="ts">
  import TriangleAlert from "~icons/lucide/triangle-alert";
  // The Stats tab's mark when DayZ Metrics finds the player count suspicious.
  let { class: klass = "" }: { class?: string } = $props();
</script>

<TriangleAlert class="{klass} text-warn" />

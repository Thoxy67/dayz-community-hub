<script lang="ts">
  import ShieldAlert from "~icons/lucide/shield-alert";
  // The Stats tab's mark when DayZ Metrics judges the player count fake.
  let { class: klass = "" }: { class?: string } = $props();
</script>

<ShieldAlert class="{klass} text-err" />

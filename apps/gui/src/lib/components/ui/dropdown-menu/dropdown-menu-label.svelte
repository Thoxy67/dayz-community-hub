<script lang="ts">
  import type { Snippet } from "svelte";
  import { DropdownMenu } from "bits-ui";

  let { children }: { children: Snippet } = $props();
</script>

<DropdownMenu.GroupHeading
  class="px-1.5 pt-1.5 pb-0.5 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase"
>
  {@render children()}
</DropdownMenu.GroupHeading>

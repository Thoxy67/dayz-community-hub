<script lang="ts">
  import type { Snippet } from "svelte";
  import { Tooltip } from "bits-ui";

  // Once, at the root of each window. Every tooltip under it shares the
  // delays, which is what lets sweeping along a toolbar read every hint
  // without waiting for each: the first waits, and for a moment after one has
  // been up the next comes at once.
  //
  // `disableHoverableContent`: a hint is not something to move the pointer
  // onto, and keeping it up while the pointer crosses it covers the very
  // controls it was describing.
  let { children }: { children: Snippet } = $props();
</script>

<Tooltip.Provider
  delayDuration={350}
  skipDelayDuration={400}
  disableHoverableContent
  ignoreNonKeyboardFocus
>
  {@render children()}
</Tooltip.Provider>

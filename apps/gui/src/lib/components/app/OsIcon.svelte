<script lang="ts">
  import { dict } from "$lib/i18n";
  import Windows from "~icons/boxicons/windows";
  import { LinuxIcon } from "$lib/components/ui/brand";

  /** The server's operating system: "w" is Windows, anything else Linux. */
  let { environment, class: klass = "size-3.5" }: { environment: string; class?: string } = $props();
  const c = dict("servers");
</script>

<span class="grid place-items-center text-fg-faint" title={environment === "w" ? $c.osWindows.value : $c.osLinux.value}>
  {#if environment === "w"}<Windows class={klass} />{:else}<LinuxIcon class={klass} />{/if}
</span>

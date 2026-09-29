<script lang="ts">
  import { dict } from "$lib/i18n";
  import Ban from "~icons/lucide/ban";
  import X from "~icons/lucide/x";
  import { Tag } from "$lib/components/ui/tag";
  import { profile } from "$lib/stores/profile.svelte";
  import { SettingsSection as Section } from "$lib/components/app";

  const s = dict("settings");
  const ips = $derived(profile.data?.excluded_ips ?? []);

</script>

<Section id="excluded" title={$s.sectionExcluded.value} description={$s.excludedHint.value} icon={Ban}>
  {#snippet aside()}
    <Tag tone={ips.length ? "warn" : "neutral"}>
      {ips.length === 1 ? $s.excludedIpsCountOne({ count: 1 }).value : $s.excludedIpsCountOther({ count: ips.length }).value}
    </Tag>
  {/snippet}
  {#if ips.length === 0}
    <p class="m-0 px-pad py-4 text-center text-xs text-fg-faint">{$s.excludedIpsNone.value}</p>
  {:else}
    <ul class="m-0 list-none divide-y divide-border/60 p-0">
      {#each ips as ip (ip)}
        <li class="flex items-center gap-2 bg-bg px-pad py-1.5">
          <Ban class="size-3.5 shrink-0 text-err" />
          <span class="font-mono text-xs text-fg" data-selectable>{ip}</span>
          <button
            type="button"
            class="ml-auto grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            aria-label={$s.excludedIpsRemove({ ip }).value}
            title={$s.excludedIpsRemove({ ip }).value}
            onclick={() => profile.unexcludeIp(ip)}><X class="size-icon-sm" /></button
          >
        </li>
      {/each}
    </ul>
  {/if}
</Section>

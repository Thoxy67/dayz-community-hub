<script lang="ts">
  import type { TabId } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Tab {
    id: TabId;
    label: string;
    count?: number;
    icon?: string;   // iconify icon id — replaces label when set
    pushRight?: boolean; // push this tab (and all following) to the right
  }

  interface Props {
    activeTab: TabId;
    tabs: Tab[];
    onSelect: (id: TabId) => void;
  }

  let { activeTab, tabs, onSelect }: Props = $props();
</script>

<div class="tabs tabs-border bg-base-200 border-b border-base-300 flex-shrink-0 px-2 pt-1">
  {#each tabs as tab}
    <button
      class="tab tab-sm gap-1 {tab.pushRight ? 'ml-auto' : ''} {activeTab === tab.id ? 'tab-active font-semibold' : 'text-base-content/60 hover:text-base-content'}"
      title={tab.icon ? tab.label : undefined}
      onclick={() => onSelect(tab.id)}
    >
      {#if tab.icon}
        <Icon icon={tab.icon} class="size-4" />
      {:else}
        {tab.label}
        {#if tab.count !== undefined && tab.count > 0}
          <span class="badge badge-sm {activeTab === tab.id ? 'badge-primary' : 'badge-ghost'}">
            {tab.count}
          </span>
        {/if}
      {/if}
    </button>
  {/each}
</div>

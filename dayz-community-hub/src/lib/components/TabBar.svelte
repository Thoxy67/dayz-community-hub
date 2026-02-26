<script lang="ts">
  import type { TabId } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Tab {
    id: TabId;
    label: string;
    count?: number;
    icon?: string;
    pushRight?: boolean;
  }

  interface Props {
    activeTab: TabId;
    tabs: Tab[];
    onSelect: (id: TabId) => void;
  }

  let { activeTab, tabs, onSelect }: Props = $props();
</script>

<div class="tabs-wrap">
  {#each tabs as tab}
    <button
      class="tab-btn {tab.pushRight ? 'ml-auto' : ''} {activeTab === tab.id ? 'tab-btn--active' : ''}"
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

      <!-- Per-tab underline that grows in when active -->
      <span class="tab-line"></span>
    </button>
  {/each}
</div>

<style>
  .tabs-wrap {
    display: flex;
    align-items: flex-end;
    background: var(--color-base-200);
    border-bottom: 1px solid var(--color-base-300);
    flex-shrink: 0;
    padding: 0 0.5rem;
  }

  .tab-btn {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.875rem;
    color: oklch(from var(--color-base-content) l c h / 0.6);
    transition: color 150ms, transform 150ms;
    cursor: pointer;
    background: none;
    border: none;
    outline: none;
  }

  .tab-btn:hover {
    color: var(--color-base-content);
    transform: scale(1.05);
  }

  .tab-btn--active {
    color: var(--color-base-content);
    font-weight: 600;
    transform: scale(1.08);
  }

  /* The sliding underline lives inside each button */
  .tab-line {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--color-primary);
    border-radius: 2px 2px 0 0;
    /* Start scaled to 0, transition to full width */
    transform: scaleX(0);
    transform-origin: center;
    transition: transform 200ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .tab-btn--active .tab-line {
    transform: scaleX(1);
  }
</style>

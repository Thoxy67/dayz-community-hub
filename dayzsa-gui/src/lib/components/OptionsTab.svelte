<script lang="ts">
  import type { LaunchOptionDto } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Props {
    options: LaunchOptionDto[];
    onToggle: (key: string) => void;
    onSetValue: (key: string, value: string | null) => void;
  }

  let { options, onToggle, onSetValue }: Props = $props();

  let editingKey = $state<string | null>(null);
  let editValue = $state('');

  function startEdit(opt: LaunchOptionDto) {
    editingKey = opt.key;
    editValue = opt.value ?? '';
  }

  function applyEdit() {
    if (editingKey === null) return;
    onSetValue(editingKey, editValue.trim() || null);
    editingKey = null;
    editValue = '';
  }

  function cancelEdit() {
    editingKey = null;
    editValue = '';
  }

  function handleEditKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') applyEdit();
    if (e.key === 'Escape') cancelEdit();
  }
</script>

<div class="flex flex-col h-full overflow-hidden">
  <div class="px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
    <p class="text-xs text-base-content/50">
      Toggle options or set values for flags passed to DayZ at launch.
    </p>
  </div>

  <div class="overflow-y-auto flex-1">
    <table class="table table-sm table-pin-rows w-full">
      <thead>
        <tr class="bg-base-200 text-base-content/60 text-xs">
          <th class="w-12">On</th>
          <th class="w-40">Option</th>
          <th>Description</th>
          <th class="w-32">Value</th>
          <th class="w-16"></th>
        </tr>
      </thead>
      <tbody>
        {#each options as opt}
          <tr class="hover:bg-base-200 {opt.enabled ? '' : 'opacity-60'}">
            <td class="text-center">
              <input
                type="checkbox"
                class="toggle toggle-sm toggle-primary"
                checked={opt.enabled}
                onchange={() => onToggle(opt.key)}
              />
            </td>
            <td class="font-mono text-xs font-medium">{opt.key}</td>
            <td class="text-xs text-base-content/70">{opt.description}</td>
            <td>
              {#if editingKey === opt.key}
                <input
                  type="text"
                  class="input input-xs input-bordered w-full font-mono"
                  bind:value={editValue}
                  onkeydown={handleEditKeydown}
                />
              {:else}
                <span class="text-xs font-mono {opt.value ? 'text-accent' : 'text-base-content/30'}">
                  {opt.value ?? '—'}
                </span>
              {/if}
            </td>
            <td>
              {#if editingKey === opt.key}
                <div class="flex gap-1">
                  <button class="btn btn-success btn-xs" onclick={applyEdit}>
                    <Icon icon="ph:check" class="size-3.5" />
                  </button>
                  <button class="btn btn-ghost btn-xs" onclick={cancelEdit}>
                    <Icon icon="ph:x" class="size-3.5" />
                  </button>
                </div>
              {:else}
                <button
                  class="btn btn-ghost btn-xs"
                  onclick={() => startEdit(opt)}
                  title="Edit value"
                >
                  Edit
                </button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

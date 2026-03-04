<script lang="ts">
  import type { LaunchOptionDto } from '$lib/types';
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    options: LaunchOptionDto[];
    search: string;
    onToggle: (key: string) => void;
    onSetValue: (key: string, value: string | null) => void;
  }

  let { options, search = $bindable(''), onToggle, onSetValue }: Props = $props();
  let editingKey = $state<string | null>(null);
  let editValue = $state('');

  // ── Group definitions ──────────────────────────────────────────────────────
  type GroupDef = {
    labelKey: string;
    icon: string;
    color: string;
    keys: string[];
  };

  const groupDefs: GroupDef[] = [
    {
      labelKey: 'window',
      icon: 'ph:monitor',
      color: 'text-opt-display',
      keys: ['window', 'noborder'],
    },
    {
      labelKey: 'startup',
      icon: 'ph:rocket-launch',
      color: 'text-opt-network',
      keys: ['nosplash', 'skipintro', 'nolauncher'],
    },
    {
      labelKey: 'performance',
      icon: 'ph:gauge',
      color: 'text-opt-launch',
      keys: ['high', 'max_mem', 'max_vram', 'cpu_count', 'ex_threads', 'no_benchmark'],
    },
    {
      labelKey: 'world',
      icon: 'ph:globe-hemisphere-west',
      color: 'text-opt-input',
      keys: ['world', 'no_pause'],
    },
    {
      labelKey: 'developer',
      icon: 'ph:code',
      color: 'text-opt-misc',
      keys: ['file_patching', 'do_logs', 'script_debug', 'buldozer', 'winxp', 'profiles'],
    },
  ];

  const groupLabels: Record<string, () => string> = {
    window: () => m.options_group_window(),
    startup: () => m.options_group_startup(),
    performance: () => m.options_group_performance(),
    world: () => m.options_group_world(),
    developer: () => m.options_group_developer(),
  };

  function getGroupLabel(key: string): string {
    return groupLabels[key]?.() ?? key;
  }

  // Per-option metadata: icon + label function
  type MetaDef = { icon: string; labelFn: () => string };
  const metaDefs: Record<string, MetaDef> = {
    window: { icon: 'ph:frame-corners', labelFn: () => m.options_label_windowed() },
    noborder: { icon: 'ph:browsers', labelFn: () => m.options_label_borderless() },
    nosplash: { icon: 'ph:image-broken', labelFn: () => m.options_label_nosplash() },
    skipintro: { icon: 'ph:skip-forward-circle', labelFn: () => m.options_label_skipintro() },
    nolauncher: { icon: 'ph:rocket', labelFn: () => m.options_label_nolauncher() },
    high: { icon: 'ph:arrow-fat-up', labelFn: () => m.options_label_high() },
    max_mem: { icon: 'ph:memory', labelFn: () => m.options_label_max_mem() },
    max_vram: { icon: 'ph:graphics-card', labelFn: () => m.options_label_max_vram() },
    cpu_count: { icon: 'ph:cpu', labelFn: () => m.options_label_cpu_count() },
    ex_threads: { icon: 'ph:threads-logo', labelFn: () => m.options_label_ex_threads() },
    no_benchmark: { icon: 'ph:chart-bar', labelFn: () => m.options_label_no_benchmark() },
    world: { icon: 'ph:map-trifold', labelFn: () => m.options_label_world() },
    no_pause: { icon: 'ph:pause-circle', labelFn: () => m.options_label_no_pause() },
    file_patching: { icon: 'ph:file-dashed', labelFn: () => m.options_label_file_patching() },
    do_logs: { icon: 'ph:scroll', labelFn: () => m.options_label_do_logs() },
    script_debug: { icon: 'ph:bug', labelFn: () => m.options_label_script_debug() },
    buldozer: { icon: 'ph:bulldozer', labelFn: () => m.options_label_buldozer() },
    winxp: { icon: 'ph:windows-logo', labelFn: () => m.options_label_winxp() },
    profiles: { icon: 'ph:folder-open', labelFn: () => m.options_label_profiles() },
  };

  function getMeta(key: string): { icon: string; label: string } {
    const def = metaDefs[key];
    return def ? { icon: def.icon, label: def.labelFn() } : { icon: 'ph:sliders', label: key };
  }

  // Translated descriptions for options
  const descFns: Record<string, () => string> = {
    window: () => m.options_desc_window(),
    noborder: () => m.options_desc_noborder(),
    nosplash: () => m.options_desc_nosplash(),
    skipintro: () => m.options_desc_skipintro(),
    nolauncher: () => m.options_desc_nolauncher(),
    high: () => m.options_desc_high(),
    max_mem: () => m.options_desc_max_mem(),
    max_vram: () => m.options_desc_max_vram(),
    cpu_count: () => m.options_desc_cpu_count(),
    ex_threads: () => m.options_desc_ex_threads(),
    no_benchmark: () => m.options_desc_no_benchmark(),
    world: () => m.options_desc_world(),
    no_pause: () => m.options_desc_no_pause(),
    file_patching: () => m.options_desc_file_patching(),
    do_logs: () => m.options_desc_do_logs(),
    script_debug: () => m.options_desc_script_debug(),
    buldozer: () => m.options_desc_buldozer(),
    winxp: () => m.options_desc_winxp(),
    profiles: () => m.options_desc_profiles(),
  };

  function getDescription(key: string, fallback: string): string {
    return descFns[key]?.() ?? fallback;
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  // Lookup map rebuilt only when options changes — O(n) once instead of per render.
  let optMap = $derived(new Map(options.map((o) => [o.key, o])));

  // These are $derived so they only recompute when options or search changes,
  // not on every render tick (previously plain functions called in {#each}).
  let filteredGroups = $derived(
    (() => {
      const q = search.trim().toLowerCase();
      return groupDefs
        .map((g) => ({
          ...g,
          label: getGroupLabel(g.labelKey),
          opts: g.keys
            .map((k) => optMap.get(k))
            .filter((o): o is LaunchOptionDto => {
              if (!o) return false;
              if (!q) return true;
              return (
                o.key.toLowerCase().includes(q) ||
                o.description.toLowerCase().includes(q) ||
                getMeta(o.key).label.toLowerCase().includes(q)
              );
            }),
        }))
        .filter((g) => g.opts.length > 0);
    })(),
  );

  // Ungrouped options (not listed in any group)
  let ungroupedOpts = $derived(
    (() => {
      const listed = new Set(groupDefs.flatMap((g) => g.keys));
      const q = search.trim().toLowerCase();
      return options.filter((o) => {
        if (listed.has(o.key)) return false;
        if (!q) return true;
        return o.key.toLowerCase().includes(q) || o.description.toLowerCase().includes(q);
      });
    })(),
  );

  // ── Edit helpers ──────────────────────────────────────────────────────────
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

  // ── Flag badge text ────────────────────────────────────────────────────────
  function flagText(opt: LaunchOptionDto): string {
    const flagMap: Record<string, string> = {
      window: '-window',
      noborder: '-noborder',
      nosplash: '-nosplash',
      skipintro: '-skipIntro',
      nolauncher: '-nolauncher',
      file_patching: '-filePatching',
      do_logs: '-doLogs',
      buldozer: '-buldozer',
      winxp: '-winxp',
      high: '-high',
      world: '-world',
      no_pause: '-noPause',
      max_mem: '-maxMem',
      max_vram: '-maxVRAM',
      cpu_count: '-cpuCount',
      ex_threads: '-exThreads',
      no_benchmark: '-noBenchmark',
      script_debug: '-scriptDebug',
      profiles: '-profiles',
    };
    const flag = flagMap[opt.key] ?? `-${opt.key}`;
    return opt.value ? `${flag}=${opt.value}` : flag;
  }

  // Count of enabled options
  let enabledCount = $derived(options.filter((o) => o.enabled).length);
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Header bar -->
  <div class="flex-shrink-0 flex items-center gap-3 px-4 py-2.5 bg-base-200 border-b border-base-300">
    <!-- Search -->
    <label class="input input-sm input-bordered flex items-center gap-2 flex-1 max-w-xs">
      <Icon icon="ph:magnifying-glass" class="size-3.5 text-base-content/40 flex-shrink-0" />
      <input type="text" placeholder={m.options_search_placeholder()} class="grow text-xs" bind:value={search} />
      {#if search}
        <button class="text-base-content/40 hover:text-base-content" onclick={() => (search = '')}>
          <Icon icon="ph:x" class="size-3" />
        </button>
      {/if}
    </label>

    <div class="flex-1"></div>

    <!-- Active count pill -->
    <span class="badge badge-primary badge-sm gap-1 font-medium">
      <Icon icon="ph:check-circle" class="size-3" />
      {enabledCount === 1 ? m.options_active_one({ count: enabledCount }) : m.options_active({ count: enabledCount })}
    </span>

    <span class="text-xs text-base-content/40">{m.options_flags_hint()}</span>
  </div>

  <!-- Scrollable body -->
  <div class="overflow-y-auto flex-1 p-4 space-y-5">
    {#each filteredGroups as group}
      <!-- Group card -->
      <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
        <!-- Group header -->
        <div class="flex items-center gap-2 px-4 py-2 bg-base-200 border-b border-base-300">
          <Icon icon={group.icon} class="size-4 {group.color}" />
          <span class="text-xs font-semibold text-base-content/80 uppercase tracking-wide">
            {group.label}
          </span>
          <span class="ml-auto text-xs text-base-content/40">
            {m.options_enabled_count({ enabled: group.opts.filter((o) => o.enabled).length, total: group.opts.length })}
          </span>
        </div>

        <!-- Option rows -->
        <div class="divide-y divide-base-200">
          {#each group.opts as opt}
            <div
              class="flex items-center gap-3 px-4 py-2.5 transition-colors"
              class:opacity-50={!opt.enabled}
              class:hover:bg-base-200={editingKey !== opt.key}
            >
              <!-- Icon -->
              <span class="flex-shrink-0 {group.color} opacity-80">
                <Icon icon={getMeta(opt.key).icon} class="size-4" />
              </span>

              <!-- Label + description -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="text-sm font-medium leading-tight">
                    {getMeta(opt.key).label}
                  </span>
                  {#if opt.enabled}
                    <span class="font-mono text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded leading-none">
                      {flagText(opt)}
                    </span>
                  {/if}
                </div>
                <p class="text-xs text-base-content/50 mt-0.5 leading-tight">
                  {getDescription(opt.key, opt.description)}
                </p>
              </div>

              <!-- Value edit area -->
              {#if opt.value !== null || editingKey === opt.key}
                <div class="flex-shrink-0 flex items-center gap-1">
                  {#if editingKey === opt.key}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="text"
                      class="input input-xs input-bordered w-28 font-mono text-xs"
                      bind:value={editValue}
                      onkeydown={handleEditKeydown}
                      autofocus
                    />
                    <button class="btn btn-success btn-xs btn-square" onclick={applyEdit} title={m.options_apply()}>
                      <Icon icon="ph:check" class="size-3.5" />
                    </button>
                    <button class="btn btn-ghost btn-xs btn-square" onclick={cancelEdit} title={m.options_cancel()}>
                      <Icon icon="ph:x" class="size-3.5" />
                    </button>
                  {:else}
                    <span class="font-mono text-xs text-accent bg-accent/10 px-2 py-0.5 rounded">
                      {opt.value}
                    </span>
                    <button
                      class="btn btn-ghost btn-xs btn-square"
                      onclick={() => startEdit(opt)}
                      title={m.options_edit_value()}
                    >
                      <Icon icon="ph:pencil-simple" class="size-3.5" />
                    </button>
                  {/if}
                </div>
              {:else if opt.enabled}
                <!-- No value set, but option supports one — show edit hint for value-capable options -->
                {#if ['max_mem', 'max_vram', 'cpu_count', 'ex_threads', 'world', 'profiles', 'script_debug'].includes(opt.key)}
                  <button
                    class="btn btn-ghost btn-xs text-base-content/30 hover:text-base-content/70"
                    onclick={() => startEdit(opt)}
                    title={m.options_set_value()}
                  >
                    <Icon icon="ph:pencil-simple" class="size-3.5" />
                    <span class="text-xs">{m.options_set_value()}</span>
                  </button>
                {/if}
              {/if}

              <!-- Toggle -->
              <input
                type="checkbox"
                class="toggle toggle-sm toggle-primary flex-shrink-0"
                checked={opt.enabled}
                onchange={() => onToggle(opt.key)}
              />
            </div>
          {/each}
        </div>
      </div>
    {/each}

    <!-- Ungrouped fallback -->
    {#if ungroupedOpts.length > 0}
      <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
        <div class="flex items-center gap-2 px-4 py-2 bg-base-200 border-b border-base-300">
          <Icon icon="ph:sliders" class="size-4 text-base-content/50" />
          <span class="text-xs font-semibold text-base-content/80 uppercase tracking-wide"
            >{m.options_group_other()}</span
          >
        </div>
        <div class="divide-y divide-base-200">
          {#each ungroupedOpts as opt}
            <div
              class="flex items-center gap-3 px-4 py-2.5 hover:bg-base-200 transition-colors"
              class:opacity-50={!opt.enabled}
            >
              <Icon icon="ph:sliders" class="size-4 text-base-content/40 flex-shrink-0" />
              <div class="flex-1 min-w-0">
                <span class="text-sm font-medium font-mono">{opt.key}</span>
                <p class="text-xs text-base-content/50 mt-0.5">{getDescription(opt.key, opt.description)}</p>
              </div>
              {#if editingKey === opt.key}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  type="text"
                  class="input input-xs input-bordered w-28 font-mono text-xs"
                  bind:value={editValue}
                  onkeydown={handleEditKeydown}
                  autofocus
                />
                <button class="btn btn-success btn-xs btn-square" onclick={applyEdit}>
                  <Icon icon="ph:check" class="size-3.5" />
                </button>
                <button class="btn btn-ghost btn-xs btn-square" onclick={cancelEdit}>
                  <Icon icon="ph:x" class="size-3.5" />
                </button>
              {:else if opt.value}
                <span class="font-mono text-xs text-accent bg-accent/10 px-2 py-0.5 rounded">{opt.value}</span>
                <button class="btn btn-ghost btn-xs btn-square" onclick={() => startEdit(opt)} title={m.options_edit()}>
                  <Icon icon="ph:pencil-simple" class="size-3.5" />
                </button>
              {/if}
              <input
                type="checkbox"
                class="toggle toggle-sm toggle-primary flex-shrink-0"
                checked={opt.enabled}
                onchange={() => onToggle(opt.key)}
              />
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if filteredGroups.length === 0 && ungroupedOpts.length === 0}
      <div class="flex flex-col items-center justify-center py-16 text-base-content/30 gap-2">
        <Icon icon="ph:magnifying-glass" class="size-8" />
        <span class="text-sm">{m.options_no_match({ search })}</span>
      </div>
    {/if}
  </div>
</div>

<script lang="ts">
  import type { LaunchOptionDto } from '$lib/types';
  import Icon from '@iconify/svelte';

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
    label: string;
    icon: string;
    color: string;
    keys: string[];
  };

  const groups: GroupDef[] = [
    {
      label: 'Window',
      icon: 'ph:monitor',
      color: 'text-blue-400',
      keys: ['window', 'noborder'],
    },
    {
      label: 'Startup',
      icon: 'ph:rocket-launch',
      color: 'text-green-400',
      keys: ['nosplash', 'skipintro', 'nolauncher'],
    },
    {
      label: 'Performance',
      icon: 'ph:gauge',
      color: 'text-orange-400',
      keys: ['high', 'max_mem', 'max_vram', 'cpu_count', 'ex_threads', 'no_benchmark'],
    },
    {
      label: 'World',
      icon: 'ph:globe-hemisphere-west',
      color: 'text-teal-400',
      keys: ['world', 'no_pause'],
    },
    {
      label: 'Developer',
      icon: 'ph:code',
      color: 'text-purple-400',
      keys: ['file_patching', 'do_logs', 'script_debug', 'buldozer', 'winxp', 'profiles'],
    },
  ];

  // Per-option metadata: icon + short label
  type Meta = { icon: string; label: string };
  const meta: Record<string, Meta> = {
    window: { icon: 'ph:frame-corners', label: 'Windowed' },
    noborder: { icon: 'ph:browsers', label: 'Borderless' },
    nosplash: { icon: 'ph:image-broken', label: 'No Splash' },
    skipintro: { icon: 'ph:skip-forward-circle', label: 'Skip Intro' },
    nolauncher: { icon: 'ph:rocket', label: 'No Launcher' },
    high: { icon: 'ph:arrow-fat-up', label: 'High Priority' },
    max_mem: { icon: 'ph:memory', label: 'Max RAM' },
    max_vram: { icon: 'ph:graphics-card', label: 'Max VRAM' },
    cpu_count: { icon: 'ph:cpu', label: 'CPU Cores' },
    ex_threads: { icon: 'ph:threads-logo', label: 'Threads' },
    no_benchmark: { icon: 'ph:chart-bar', label: 'No Benchmark' },
    world: { icon: 'ph:map-trifold', label: 'World' },
    no_pause: { icon: 'ph:pause-circle', label: 'No Pause' },
    file_patching: { icon: 'ph:file-dashed', label: 'File Patching' },
    do_logs: { icon: 'ph:scroll', label: 'Logging' },
    script_debug: { icon: 'ph:bug', label: 'Script Debug' },
    buldozer: { icon: 'ph:bulldozer', label: 'Buldozer' },
    winxp: { icon: 'ph:windows-logo', label: 'DirectX 9' },
    profiles: { icon: 'ph:folder-open', label: 'Profiles Dir' },
  };

  function getMeta(key: string): Meta {
    return meta[key] ?? { icon: 'ph:sliders', label: key };
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  // Lookup map rebuilt only when options changes — O(n) once instead of per render.
  let optMap = $derived(new Map(options.map((o) => [o.key, o])));

  // These are $derived so they only recompute when options or search changes,
  // not on every render tick (previously plain functions called in {#each}).
  let filteredGroups = $derived(
    (() => {
      const q = search.trim().toLowerCase();
      return groups
        .map((g) => ({
          ...g,
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
      const listed = new Set(groups.flatMap((g) => g.keys));
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
      <input type="text" placeholder="Search options…" class="grow text-xs" bind:value={search} />
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
      {enabledCount} active
    </span>

    <span class="text-xs text-base-content/40">Flags passed to DayZ at launch</span>
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
            {group.opts.filter((o) => o.enabled).length}/{group.opts.length} enabled
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
                <p class="text-xs text-base-content/50 mt-0.5 leading-tight">{opt.description}</p>
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
                    <button class="btn btn-success btn-xs btn-square" onclick={applyEdit} title="Apply">
                      <Icon icon="ph:check" class="size-3.5" />
                    </button>
                    <button class="btn btn-ghost btn-xs btn-square" onclick={cancelEdit} title="Cancel">
                      <Icon icon="ph:x" class="size-3.5" />
                    </button>
                  {:else}
                    <span class="font-mono text-xs text-accent bg-accent/10 px-2 py-0.5 rounded">
                      {opt.value}
                    </span>
                    <button class="btn btn-ghost btn-xs btn-square" onclick={() => startEdit(opt)} title="Edit value">
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
                    title="Set value"
                  >
                    <Icon icon="ph:pencil-simple" class="size-3.5" />
                    <span class="text-xs">set value</span>
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
          <span class="text-xs font-semibold text-base-content/80 uppercase tracking-wide">Other</span>
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
                <p class="text-xs text-base-content/50 mt-0.5">{opt.description}</p>
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
                <button class="btn btn-ghost btn-xs btn-square" onclick={() => startEdit(opt)} title="Edit">
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
        <span class="text-sm">No options match "{search}"</span>
      </div>
    {/if}
  </div>
</div>

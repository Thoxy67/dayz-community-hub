<script lang="ts">
  import Icon from '@iconify/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';

  interface Props {
    onExport: () => void;
    onImport: () => void;
    onReset: () => void;
  }

  let { onExport, onImport, onReset }: Props = $props();
</script>

<div class="h-full overflow-y-auto">
  <div class="max-w-2xl mx-auto px-6 py-8 space-y-8">

    <!-- Header -->
    <div class="flex items-center gap-4">
      <img src="/icon.svg" alt="DayZ Community Hub" class="w-12 h-12" />
      <div>
        <h1 class="text-xl font-bold text-base-content tracking-tight">DayZ Community Hub</h1>
        <p class="text-sm text-base-content/50 mt-0.5">A server browser and mod manager for DayZ Standalone</p>
      </div>
    </div>

    <!-- ── Quick Start ──────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:rocket-launch" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">Quick Start</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">
        <div class="flex items-start gap-3 px-4 py-3">
          <span class="size-5 rounded-full bg-primary/15 text-primary text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">1</span>
          <div>
            <p class="text-sm font-medium text-base-content">Open account settings</p>
            <p class="text-xs text-base-content/50 mt-0.5">Click your name (or "Set up account") in the top-left of the title bar.</p>
          </div>
        </div>
        <div class="flex items-start gap-3 px-4 py-3">
          <span class="size-5 rounded-full bg-primary/15 text-primary text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">2</span>
          <div>
            <p class="text-sm font-medium text-base-content">Set your in-game name</p>
            <p class="text-xs text-base-content/50 mt-0.5">This is the player name used when launching DayZ.</p>
          </div>
        </div>
        <div class="flex items-start gap-3 px-4 py-3">
          <span class="size-5 rounded-full bg-primary/15 text-primary text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">3</span>
          <div>
            <p class="text-sm font-medium text-base-content">Browse servers and connect</p>
            <p class="text-xs text-base-content/50 mt-0.5">Use the <span class="font-semibold text-base-content/70">Servers</span> tab to find a server, double-click it (or click Connect) to launch DayZ.</p>
          </div>
        </div>
      </div>
    </section>

    <!-- ── Tabs overview ───────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:tabs" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">Tabs</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">
        {#each [
          { icon: 'mdi:server-network', label: 'Servers', desc: 'Browse the live public server list. Use the search bar and flag filters (1P, password, BE, mods, map) to narrow results. Double-click a row to connect.' },
          { icon: 'ph:star', label: 'Favorites', desc: 'Servers you have starred. Quickly re-join without searching.' },
          { icon: 'ph:clock-clockwise', label: 'History', desc: 'The last servers you connected to, with timestamps.' },
          { icon: 'mdi:puzzle', label: 'Mods', desc: 'All DayZ Workshop mods installed in your workshop directory. Supports checking for updates (requires Steam API key), downloading, and managing mods via SteamCMD.' },
          { icon: 'ph:newspaper', label: 'News', desc: 'Latest DayZ news articles fetched from the official site.' },
          { icon: 'ph:plugs-connected', label: 'Connect', desc: 'Connect directly to a server by IP and port without browsing the list.' },
          { icon: 'ph:sliders', label: 'Options', desc: 'Toggle DayZ launch options (e.g. -noPause, -filePatching). Changes apply on the next connection.' },
          { icon: 'ph:mountains', label: 'Offline', desc: 'Manage DayZ offline/LAN missions for solo play without a server.' },
        ] as tab}
          <div class="flex items-start gap-3 px-4 py-3">
            <Icon icon={tab.icon} class="size-4 text-base-content/40 shrink-0 mt-0.5" />
            <div>
              <p class="text-sm font-medium text-base-content">{tab.label}</p>
              <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">{tab.desc}</p>
            </div>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── Account Settings ─────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:user-gear" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">Account Settings</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">

        <!-- Identity -->
        <div class="px-4 py-3">
          <p class="text-sm font-medium text-base-content flex items-center gap-2">
            <Icon icon="ph:game-controller" class="size-3.5 text-base-content/40" />
            Identity — In-game name
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            Your DayZ character name. Passed to the game as <span class="font-mono bg-base-300/60 px-1 rounded">-name=</span>. Optional but recommended.
          </p>
        </div>

        <!-- Steam Login -->
        <div class="px-4 py-3">
          <p class="text-sm font-medium text-base-content flex items-center gap-2">
            <Icon icon="mdi:steam" class="size-3.5 text-base-content/40" />
            Steam Login — for SteamCMD
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            Your Steam username and password, used only by <span class="font-mono bg-base-300/60 px-1 rounded">SteamCMD</span> to download and update mods.
            Leave blank to use the <span class="font-mono bg-base-300/60 px-1 rounded">anonymous</span> login (works for free mods).
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            <span class="font-semibold text-warning/80">Steam root</span> is the path to your Steam installation folder.
            Linux default: <span class="font-mono bg-base-300/60 px-1 rounded">~/.steam/steam</span> —
            Windows default: <span class="font-mono bg-base-300/60 px-1 rounded">C:\Program Files (x86)\Steam</span>.
            Required so the app can find your Workshop directory.
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            <span class="font-semibold text-base-content/60">SteamCMD path</span> — leave blank to let the app auto-detect.
            Set this if SteamCMD is installed in a non-standard location.
            Linux: <span class="font-mono bg-base-300/60 px-1 rounded">steamcmd</span> or
            <span class="font-mono bg-base-300/60 px-1 rounded">/usr/bin/steamcmd</span> —
            Windows: <span class="font-mono bg-base-300/60 px-1 rounded">C:\SteamCMD\steamcmd.exe</span>.
          </p>
          <div class="flex items-center gap-1.5 mt-2 text-xs text-warning/70">
            <Icon icon="ph:warning" class="size-3 shrink-0" />
            <span>Password is stored in plaintext in <span class="font-mono">profile.json</span>. Leave blank and rely on cached SteamCMD credentials when possible.</span>
          </div>
        </div>

        <!-- Steam API -->
        <div class="px-4 py-3">
          <p class="text-sm font-medium text-base-content flex items-center gap-2">
            <Icon icon="ph:identification-card" class="size-3.5 text-base-content/40" />
            Steam API Key &amp; Steam ID — optional
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            Used for two features:
          </p>
          <ul class="mt-1.5 space-y-1 text-xs text-base-content/50">
            <li class="flex items-start gap-2">
              <Icon icon="ph:user-circle" class="size-3.5 text-base-content/30 shrink-0 mt-0.5" />
              <span><span class="font-semibold text-base-content/60">Steam avatar</span> — your profile picture shown in the title bar. Requires both API key and your 64-bit Steam ID.</span>
            </li>
            <li class="flex items-start gap-2">
              <Icon icon="mdi:puzzle-outline" class="size-3.5 text-base-content/30 shrink-0 mt-0.5" />
              <span><span class="font-semibold text-base-content/60">Mod update checks</span> — the Mods tab "Check updates" button queries the Steam Workshop API for newer mod versions. The API key raises rate limits; without it the check still works but at lower limits.</span>
            </li>
          </ul>
          <p class="text-xs text-base-content/40 mt-2 leading-relaxed">
            Get a free API key at <button class="font-mono bg-base-300/60 hover:bg-base-300 px-1 rounded text-primary hover:underline transition-colors" onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}>steamcommunity.com/dev/apikey</button>.
            Your Steam ID (64-bit) is visible at <button class="font-mono bg-base-300/60 hover:bg-base-300 px-1 rounded text-primary hover:underline transition-colors" onclick={() => openUrl('https://steamdb.info/calculator/')}>steamdb.info/calculator</button>.
          </p>
        </div>

      </div>
    </section>

    <!-- ── SteamCMD ─────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:terminal-window" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">SteamCMD &amp; Mods</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">
        <div class="px-4 py-3">
          <p class="text-sm font-medium text-base-content">What is SteamCMD?</p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            SteamCMD is a command-line tool from Valve used to download Steam Workshop content (mods) without the full Steam client.
            The app uses it to install and update your DayZ mods in the background.
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            If the title bar shows <span class="text-warning/80 font-semibold">SteamCMD not found</span>, download it from
            <span class="font-mono bg-base-300/60 px-1 rounded">developer.valvesoftware.com/wiki/SteamCMD</span> and either
            place it on your PATH or set the path manually in account settings.
          </p>
          <p class="text-xs text-base-content/50 mt-1 leading-relaxed">
            <span class="font-semibold text-base-content/60">Linux:</span> install via package manager
            (<span class="font-mono bg-base-300/60 px-1 rounded">apt install steamcmd</span>) or place the binary in
            <span class="font-mono bg-base-300/60 px-1 rounded">/usr/bin/steamcmd</span>.
            <span class="font-semibold text-base-content/60">Windows:</span> extract to
            <span class="font-mono bg-base-300/60 px-1 rounded">C:\SteamCMD\</span> and add to PATH,
            or set the path explicitly in account settings.
          </p>
        </div>
        <div class="px-4 py-3">
          <p class="text-sm font-medium text-base-content">Mod workflow</p>
          <ol class="mt-1.5 space-y-1 text-xs text-base-content/50 list-none">
            <li class="flex items-start gap-2">
              <span class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">1</span>
              <span>Connect to a server — the app will offer to install any missing mods automatically.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">2</span>
              <span>Go to the <span class="font-semibold text-base-content/60">Mods</span> tab and click <span class="font-semibold text-base-content/60">Check updates</span> to see which mods are outdated.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">3</span>
              <span>Click <span class="font-semibold text-base-content/60">Update N</span> to update all stale mods in one batch, or update them individually.</span>
            </li>
          </ol>
        </div>
      </div>
    </section>

    <!-- ── Tips ────────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:lightbulb" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">Tips</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">
        {#each [
          { icon: 'ph:arrows-down-up', tip: 'Click any column header in the server list to sort by that column. Click again to reverse.' },
          { icon: 'ph:keyboard', tip: 'Use arrow keys to navigate the server list, Enter to connect to the selected server.' },
          { icon: 'ph:copy', tip: 'Click an IP address in the server list to copy it to clipboard.' },
          { icon: 'ph:star', tip: 'Star servers from the footer bar or the Info panel to keep them in Favorites.' },
          { icon: 'ph:info', tip: 'Click Info in the footer to query live A2S data — player list, live ping, and mod list.' },
          { icon: 'ph:sun', tip: 'Toggle between light and dark theme with the sun/moon button in the title bar.' },
        ] as item}
          <div class="flex items-start gap-3 px-4 py-3">
            <Icon icon={item.icon} class="size-3.5 text-base-content/30 shrink-0 mt-0.5" />
            <p class="text-xs text-base-content/55 leading-relaxed">{item.tip}</p>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── Profile Management ───────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:archive" class="size-4 text-primary shrink-0" />
        <h2 class="text-sm font-semibold text-base-content uppercase tracking-wider">Profile Management</h2>
      </div>
      <div class="bg-base-200/60 rounded-xl border border-base-300/60 divide-y divide-base-300/50">

        <!-- Export -->
        <div class="flex items-center gap-4 px-4 py-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-base-content flex items-center gap-2">
              <Icon icon="ph:export" class="size-3.5 text-base-content/40" />
              Export
            </p>
            <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">
              Save all settings, favorites, history, launch options and mod list to a
              <span class="font-mono bg-base-300/60 px-1 rounded">.dchub</span> file.
              The file is zstd-compressed and can be imported on any machine.
            </p>
          </div>
          <button class="btn btn-sm btn-primary shrink-0 gap-1.5" onclick={onExport}>
            <Icon icon="ph:export" class="size-3.5" />
            Export
          </button>
        </div>

        <!-- Import -->
        <div class="flex items-center gap-4 px-4 py-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-base-content flex items-center gap-2">
              <Icon icon="ph:import" class="size-3.5 text-base-content/40" />
              Import
            </p>
            <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">
              Restore a previously exported
              <span class="font-mono bg-base-300/60 px-1 rounded">.dchub</span> file.
              This will overwrite your current profile and mod list.
            </p>
          </div>
          <button class="btn btn-sm btn-ghost shrink-0 gap-1.5" onclick={onImport}>
            <Icon icon="ph:import" class="size-3.5" />
            Import
          </button>
        </div>

        <!-- Reset -->
        <div class="flex items-center gap-4 px-4 py-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-base-content flex items-center gap-2">
              <Icon icon="ph:arrow-counter-clockwise" class="size-3.5 text-error/60" />
              <span class="text-error/80">Reset to defaults</span>
            </p>
            <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">
              Wipe your profile back to factory defaults — clears all settings, favorites,
              history and launch options. Installed mods on disk are not affected.
            </p>
          </div>
          <button class="btn btn-sm btn-error btn-outline shrink-0 gap-1.5" onclick={onReset}>
            <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />
            Reset
          </button>
        </div>

      </div>
    </section>

    <!-- Footer -->
    <div class="text-center text-xs text-base-content/25 pb-2">
      DayZ Community Hub — built with Tauri + Svelte
    </div>

  </div>
</div>

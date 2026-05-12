<script lang="ts">
  // Theme editor modal — extracted from TitleBar.svelte so the ~520 LOC of
  // markup, color-group definitions, ThemeColorPicker references and the
  // lazy CssEditor loader all live in their own chunk.  Loaded on demand
  // via dynamic import when the user opens the editor.

  import Icon from '@iconify/svelte';
  import { app } from '$lib/state.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import ThemeColorPicker from './ThemeColorPicker.svelte';

  // CssEditor stays lazy inside this lazy modal — saves the editor's
  // (chunky) highlighter until the user clicks the Code tab.
  const lazyCssEditor = () => import('./CssEditor.svelte');

  interface Props {
    customCss: string;
    originalCssOnOpen: string;
    platform: string;
    onClose: () => void;
    onImportTheme: () => void;
    onExportTheme: () => void;
  }

  let {
    customCss = $bindable(),
    originalCssOnOpen,
    platform,
    onClose,
    onImportTheme,
    onExportTheme,
  }: Props = $props();

  // Tab state lives only inside the modal — no need to lift it into the parent.
  let themeEditorTab = $state<'colors' | 'code'>('colors');

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  // ── Color group definitions ───────────────────────────────────────────
  const baseColors = [
    { key: '--color-base-100', label: () => m.theme_color_base_100() },
    { key: '--color-base-200', label: () => m.theme_color_base_200() },
    { key: '--color-base-300', label: () => m.theme_color_base_300() },
    { key: '--color-base-content', label: () => m.theme_color_base_content() },
  ];
  const brandColors = [
    { key: '--color-primary', label: () => m.theme_color_primary() },
    { key: '--color-secondary', label: () => m.theme_color_secondary() },
    { key: '--color-accent', label: () => m.theme_color_accent() },
  ];
  const statusColors = [
    { key: '--color-success', label: () => m.theme_color_success() },
    { key: '--color-warning', label: () => m.theme_color_warning() },
    { key: '--color-error', label: () => m.theme_color_error() },
    { key: '--color-info', label: () => m.theme_color_info() },
  ];
  const terminalColors = [
    { key: '--color-terminal-bg', label: () => m.theme_color_terminal_bg() },
    { key: '--color-terminal-border', label: () => m.theme_color_terminal_border() },
    { key: '--color-log-error', label: () => m.theme_color_log_error() },
    { key: '--color-log-warning', label: () => m.theme_color_log_warning() },
    { key: '--color-log-success', label: () => m.theme_color_log_success() },
    { key: '--color-log-info', label: () => m.theme_color_log_info() },
    { key: '--color-log-default', label: () => m.theme_color_log_default() },
  ];
  const syntaxColors = [
    { key: '--color-syntax-comment', label: () => m.theme_color_syntax_comment() },
    { key: '--color-syntax-selector', label: () => m.theme_color_syntax_selector() },
    { key: '--color-syntax-property', label: () => m.theme_color_syntax_property() },
    { key: '--color-syntax-variable', label: () => m.theme_color_syntax_variable() },
    { key: '--color-syntax-string', label: () => m.theme_color_syntax_string() },
    { key: '--color-syntax-number', label: () => m.theme_color_syntax_number() },
    { key: '--color-syntax-function', label: () => m.theme_color_syntax_function() },
  ];
  const neutralColors = [
    { key: '--color-neutral', label: () => m.theme_color_neutral() },
    { key: '--color-neutral-content', label: () => m.theme_color_neutral_content() },
  ];
  const contentColors = [
    { key: '--color-primary-content', label: () => m.theme_color_primary_content() },
    { key: '--color-secondary-content', label: () => m.theme_color_secondary_content() },
    { key: '--color-accent-content', label: () => m.theme_color_accent_content() },
    { key: '--color-info-content', label: () => m.theme_color_info_content() },
    { key: '--color-success-content', label: () => m.theme_color_success_content() },
    { key: '--color-warning-content', label: () => m.theme_color_warning_content() },
    { key: '--color-error-content', label: () => m.theme_color_error_content() },
  ];
  const appAccentColors = [
    { key: '--color-accent-map', label: () => m.theme_color_accent_map() },
    { key: '--color-accent-mods', label: () => m.theme_color_accent_mods() },
    { key: '--color-accent-stat-server', label: () => m.theme_color_accent_stat_server() },
    { key: '--color-accent-stat-players', label: () => m.theme_color_accent_stat_players() },
    { key: '--color-accent-stat-steam', label: () => m.theme_color_accent_stat_steam() },
    { key: '--color-accent-update', label: () => m.theme_color_accent_update() },
    { key: '--color-accent-stale', label: () => m.theme_color_accent_stale() },
    { key: '--color-accent-highlight', label: () => m.theme_color_accent_highlight() },
  ];
  const featureColors = [
    { key: '--color-feat-browser', label: () => m.theme_color_feat_browser() },
    { key: '--color-feat-mods', label: () => m.theme_color_feat_mods() },
    { key: '--color-feat-stats', label: () => m.theme_color_feat_stats() },
    { key: '--color-feat-launch', label: () => m.theme_color_feat_launch() },
  ];
  const badgeColors = [
    { key: '--color-badge-rank', label: () => m.theme_color_badge_rank() },
    { key: '--color-badge-status', label: () => m.theme_color_badge_status() },
    { key: '--color-badge-country', label: () => m.theme_color_badge_country() },
    { key: '--color-badge-players', label: () => m.theme_color_badge_players() },
    { key: '--color-badge-firstperson', label: () => m.theme_color_badge_firstperson() },
    { key: '--color-badge-official', label: () => m.theme_color_badge_official() },
    { key: '--color-badge-custom', label: () => m.theme_color_badge_custom() },
  ];
  const uiElementColors = [
    { key: '--color-port-query', label: () => m.theme_color_port_query() },
    { key: '--color-port-game', label: () => m.theme_color_port_game() },
    { key: '--color-link', label: () => m.theme_color_link() },
    { key: '--color-score', label: () => m.theme_color_score() },
    { key: '--color-best', label: () => m.theme_color_best() },
  ];
  const optionGroupColors = [
    { key: '--color-opt-display', label: () => m.theme_color_opt_display() },
    { key: '--color-opt-network', label: () => m.theme_color_opt_network() },
    { key: '--color-opt-launch', label: () => m.theme_color_opt_launch() },
    { key: '--color-opt-input', label: () => m.theme_color_opt_input() },
    { key: '--color-opt-misc', label: () => m.theme_color_opt_misc() },
  ];
  const techBrandColors = [
    { key: '--color-tech-tauri', label: () => m.theme_color_tech_tauri() },
    { key: '--color-tech-svelte', label: () => m.theme_color_tech_svelte() },
    { key: '--color-tech-rust', label: () => m.theme_color_tech_rust() },
    { key: '--color-tech-daisyui', label: () => m.theme_color_tech_daisyui() },
    { key: '--color-tech-tailwind', label: () => m.theme_color_tech_tailwind() },
  ];
  const windowColors = [{ key: '--color-btn-close', label: () => m.theme_color_btn_close() }];
  const borderRadiusVars = [
    { key: '--radius-btn', label: () => m.theme_radius_btn() },
    { key: '--radius-box', label: () => m.theme_radius_box() },
    { key: '--radius-badge', label: () => m.theme_radius_badge() },
  ];

  // ── CSS variable accessors ─────────────────────────────────────────────
  function getColorFromCss(css: string, varName: string): string {
    const regex = new RegExp(`${varName.replace('--', '\\-\\-')}:\\s*([^;]+);`);
    const match = css.match(regex);
    return match ? match[1].trim() : 'oklch(50% 0.1 0)';
  }

  function updateColorInCss(css: string, varName: string, newValue: string): string {
    const regex = new RegExp(`(${varName.replace('--', '\\-\\-')}:\\s*)[^;]+(;)`);
    if (regex.test(css)) {
      return css.replace(regex, `$1${newValue}$2`);
    }
    return css;
  }
</script>

<div
  class="fixed inset-0 modal-backdrop-window z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  style="top: 36px;"
  role="presentation"
  onclick={onClose}
>
  <div
    class="bg-base-100 rounded-xl shadow-2xl w-full max-w-3xl mx-4 flex flex-col overflow-hidden max-h-[85vh]"
    role="dialog"
    aria-modal="true"
    aria-label={m.theme_editor_title()}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={handleKeydown}
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-4 bg-base-200 border-b border-base-300 flex-shrink-0">
      <div class="flex items-center gap-3">
        <Icon icon="ph:palette" class="size-5 text-primary" />
        <div>
          <h2 class="text-sm font-semibold text-base-content">{m.theme_editor_title()}</h2>
          <p class="text-xs text-base-content/50">{m.theme_editor_desc()}</p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button
          class="size-7 rounded flex items-center justify-center text-base-content/40 hover:bg-base-300 hover:text-base-content transition-colors"
          onclick={onClose}
          title={m.window_close()}
        >
          <Icon icon="ph:x" class="size-4" />
        </button>
      </div>
    </div>

    <!-- Window Settings (Linux only - Windows has its own window frame) -->
    {#if platform === 'linux'}
      <div class="px-5 py-3 bg-base-200/30 border-b border-base-300 flex-shrink-0 space-y-2">
        <!-- Row 1: Checkboxes and selects -->
        <div class="flex items-center gap-6">
          <div class="flex items-center gap-2 text-xs text-base-content/60">
            <Icon icon="ph:app-window" class="size-3.5" />
            <span class="font-medium uppercase tracking-wider">{m.theme_window_section()}</span>
          </div>
          <!-- Rounded Corners -->
          <div class="flex items-center gap-3">
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                class="checkbox checkbox-xs checkbox-primary"
                checked={app.windowRadius !== '0'}
                onchange={(e) => {
                  app.windowRadius = (e.target as HTMLInputElement).checked ? '0.75rem' : '0';
                  app.saveWindowSettings();
                }}
              />
              <span class="text-xs text-base-content/70">{m.theme_window_rounded()}</span>
            </label>
            {#if app.windowRadius !== '0'}
              <select
                class="select select-xs select-bordered h-6 min-h-0"
                value={app.windowRadius}
                onchange={(e) => {
                  app.windowRadius = (e.target as HTMLSelectElement).value;
                  app.saveWindowSettings();
                }}
              >
                <option value="0.375rem">S</option>
                <option value="0.5rem">M</option>
                <option value="0.75rem">L</option>
                <option value="1rem">XL</option>
              </select>
            {/if}
          </div>
          <!-- Focus Border -->
          <div class="flex items-center gap-3">
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                class="checkbox checkbox-xs checkbox-primary"
                checked={app.windowBorderSize !== '0'}
                onchange={(e) => {
                  app.windowBorderSize = (e.target as HTMLInputElement).checked ? '1px' : '0';
                  app.saveWindowSettings();
                }}
              />
              <span class="text-xs text-base-content/70">{m.theme_window_border()}</span>
            </label>
            {#if app.windowBorderSize !== '0'}
              <select
                class="select select-xs select-bordered h-6 min-h-0"
                value={app.windowBorderSize}
                onchange={(e) => {
                  app.windowBorderSize = (e.target as HTMLSelectElement).value;
                  app.saveWindowSettings();
                }}
              >
                <option value="1px">1px</option>
                <option value="2px">2px</option>
                <option value="3px">3px</option>
              </select>
            {/if}
          </div>
        </div>
        <!-- Row 2: Border colors (only when border enabled) -->
        {#if app.windowBorderSize !== '0'}
          <div class="flex items-center gap-4 pl-[88px]">
            <ThemeColorPicker
              label={m.theme_window_border_focus()}
              value={app.windowBorderFocus}
              onChange={(v) => {
                app.windowBorderFocus = v;
                app.saveWindowSettings();
              }}
            />
            <ThemeColorPicker
              label={m.theme_window_border_blur()}
              value={app.windowBorderBlur}
              onChange={(v) => {
                app.windowBorderBlur = v;
                app.saveWindowSettings();
              }}
            />
          </div>
        {/if}
      </div>
    {/if}

    <!-- Tabs -->
    <div class="flex border-b border-base-300 bg-base-200/50 px-4">
      <button
        class="px-4 py-2.5 text-xs font-medium transition-colors relative {themeEditorTab === 'colors'
          ? 'text-primary'
          : 'text-base-content/60 hover:text-base-content'}"
        onclick={() => (themeEditorTab = 'colors')}
      >
        <span class="flex items-center gap-1.5">
          <Icon icon="ph:paint-bucket" class="size-3.5" />
          {m.theme_tab_colors()}
        </span>
        {#if themeEditorTab === 'colors'}
          <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary rounded-t"></div>
        {/if}
      </button>
      <button
        class="px-4 py-2.5 text-xs font-medium transition-colors relative {themeEditorTab === 'code'
          ? 'text-primary'
          : 'text-base-content/60 hover:text-base-content'}"
        onclick={() => (themeEditorTab = 'code')}
      >
        <span class="flex items-center gap-1.5">
          <Icon icon="ph:code" class="size-3.5" />
          {m.theme_tab_code()}
        </span>
        {#if themeEditorTab === 'code'}
          <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary rounded-t"></div>
        {/if}
      </button>
    </div>

    <!-- Tab Content -->
    <div class="flex-1 min-h-0 overflow-auto">
      {#if themeEditorTab === 'colors'}
        <!-- Colors Tab - Visual Editor -->
        <div class="p-5 space-y-5">
          <!-- Base Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:squares-four" class="size-3.5" />
              {m.theme_color_base()}
            </h3>
            <div class="space-y-1">
              {#each baseColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Brand Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:paint-brush" class="size-3.5" />
              {m.theme_color_primary()}, {m.theme_color_secondary()}, {m.theme_color_accent()}
            </h3>
            <div class="space-y-1">
              {#each brandColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Status Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:flag" class="size-3.5" />
              {m.theme_color_status()}
            </h3>
            <div class="space-y-1">
              {#each statusColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Terminal Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:terminal" class="size-3.5" />
              {m.theme_color_terminal()}
            </h3>
            <div class="space-y-1">
              {#each terminalColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Syntax Highlighting -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:code" class="size-3.5" />
              {m.theme_color_syntax()}
            </h3>
            <div class="space-y-1">
              {#each syntaxColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Neutral Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:circle-half" class="size-3.5" />
              {m.theme_color_neutral_group()}
            </h3>
            <div class="space-y-1">
              {#each neutralColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Content Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:text-aa" class="size-3.5" />
              {m.theme_color_content_group()}
            </h3>
            <div class="space-y-1">
              {#each contentColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- App Accent Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:sparkle" class="size-3.5" />
              {m.theme_color_app_accent_group()}
            </h3>
            <div class="space-y-1">
              {#each appAccentColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Feature Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:star" class="size-3.5" />
              {m.theme_color_feature_group()}
            </h3>
            <div class="space-y-1">
              {#each featureColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Badge Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:tag" class="size-3.5" />
              {m.theme_color_badge_group()}
            </h3>
            <div class="space-y-1">
              {#each badgeColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- UI Element Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:squares-four" class="size-3.5" />
              {m.theme_color_ui_element_group()}
            </h3>
            <div class="space-y-1">
              {#each uiElementColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Option Group Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:sliders" class="size-3.5" />
              {m.theme_color_option_group()}
            </h3>
            <div class="space-y-1">
              {#each optionGroupColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Tech Brand Colors -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:cpu" class="size-3.5" />
              {m.theme_color_tech_brand_group()}
            </h3>
            <div class="space-y-1">
              {#each techBrandColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Window Controls -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:app-window" class="size-3.5" />
              {m.theme_color_window_group()}
            </h3>
            <div class="space-y-1">
              {#each windowColors as def}
                <ThemeColorPicker
                  label={def.label()}
                  value={getColorFromCss(customCss, def.key)}
                  onChange={(v) => (customCss = updateColorInCss(customCss, def.key, v))}
                />
              {/each}
            </div>
          </div>

          <!-- Border Radius -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:corners" class="size-3.5" />
              {m.theme_radius_group()}
            </h3>
            <div class="space-y-1">
              {#each borderRadiusVars as def}
                <label class="flex flex-col gap-1.5">
                  <span class="text-xs text-base-content/60">{def.label()}</span>
                  <input
                    type="text"
                    class="input input-sm input-bordered w-full font-mono text-xs"
                    value={getColorFromCss(customCss, def.key)}
                    onchange={(e) =>
                      (customCss = updateColorInCss(customCss, def.key, (e.target as HTMLInputElement).value))}
                  />
                </label>
              {/each}
            </div>
          </div>

          <!-- Logo Settings -->
          <div>
            <h3
              class="text-xs font-semibold text-base-content/70 uppercase tracking-wider mb-3 flex items-center gap-2"
            >
              <Icon icon="ph:image" class="size-3.5" />
              {m.theme_logo_section()}
            </h3>
            <div class="flex items-center gap-3">
              <label class="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm checkbox-primary"
                  checked={getColorFromCss(customCss, '--logo-invert') === '1'}
                  onchange={(e) => {
                    const val = (e.target as HTMLInputElement).checked ? '1' : '0';
                    customCss = updateColorInCss(customCss, '--logo-invert', val);
                  }}
                />
                <span class="text-xs text-base-content/70">{m.theme_logo_invert()}</span>
              </label>
            </div>
          </div>
        </div>
      {:else if themeEditorTab === 'code'}
        <!-- Code Tab - CSS Editor (lazy-loaded) -->
        <div class="p-4 h-[400px]">
          {#await lazyCssEditor() then mod}
            {@const CssEditor = mod.default}
            <CssEditor value={customCss} onInput={(v) => (customCss = v)} placeholder={m.theme_css_placeholder()} />
          {/await}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
      <div class="flex items-center gap-2">
        <button
          class="btn btn-ghost btn-sm text-base-content/60 gap-1.5"
          onclick={() => {
            customCss = originalCssOnOpen;
          }}
          title={m.theme_reset()}
        >
          <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />
          {m.theme_reset()}
        </button>
        <button class="btn btn-ghost btn-sm text-base-content/60 gap-1.5" onclick={onImportTheme}>
          <Icon icon="ph:upload-simple" class="size-3.5" />
          {m.theme_import()}
        </button>
        <button class="btn btn-ghost btn-sm text-base-content/60 gap-1.5" onclick={onExportTheme}>
          <Icon icon="ph:download-simple" class="size-3.5" />
          {m.theme_export()}
        </button>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-xs text-base-content/40 italic">{m.theme_autosaved()}</span>
        <button class="btn btn-primary btn-sm gap-1.5" onclick={onClose}>
          <Icon icon="ph:check" class="size-3.5" />
          {m.theme_done()}
        </button>
      </div>
    </div>
  </div>
</div>

<script lang="ts">
  import type { AppStatsDto, ProfileDto } from '$lib/types';
  import { app, type ThemeName, THEMES } from '$lib/state.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, ask } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onMount } from 'svelte';
  import Icon from '@iconify/svelte';
  import GlitchText from '$lib/components/GlitchText.svelte';
  // Theme editor modal — pulls in ThemeColorPicker (×15+) and the chunky
  // CssEditor (further lazy-loaded inside it). Only loaded when the user
  // opens the editor, which keeps TitleBar's initial chunk slim.
  const lazyThemeEditorModal = () => import('$lib/components/ThemeEditorModal.svelte');
  import LanguageSelector from '$lib/components/LanguageSelector.svelte';
  import { themePresets, loadThemeCss, type ThemePresetId } from '$lib/constants/theme-presets';
  import { parseOklch, formatOklch } from '$lib/utils/oklch';
  import * as m from '$lib/paraglide/messages.js';

  // Platform detection for Linux-only window settings
  let platform = $state<string>('');
  onMount(async () => {
    try {
      const status = await invoke<{ platform: string }>('detect_steamcmd');
      platform = status.platform;
    } catch {
      /* ignore */
    }
  });

  type UpdateState = 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

  interface Props {
    stats: AppStatsDto | null;
    avatarUrl: string | null;
    steamPlayers: number | null;
    theme: ThemeName;
    profile: ProfileDto | null;
    staleModCount?: number;
    updateState?: UpdateState;
    /** Increment to imperatively trigger the title glitch animation */
    glitchTick?: number;
    onSetTheme: (theme: ThemeName) => void;
    onUpdateMods?: () => void;
    onGoToUpdate?: () => void;
    onSaveSettings: (
      player: string | null,
      steamLogin: string | null,
      steamPassword: string | null,
      steamRoot: string | null,
      steamcmdEnabled: boolean,
      steamcmdPath: string | null,
      steamApiKey: string | null,
      steamId: string | null,
      battlemetricsApiKey: string | null,
      userLocation: [number, number] | null,
    ) => void;
    onUnexcludeIp: (ip: string) => void;
    onOpenExcludedIps: () => void;
  }

  let {
    stats,
    avatarUrl,
    steamPlayers,
    theme,
    profile,
    staleModCount = 0,
    updateState = 'idle',
    glitchTick = 0,
    onSetTheme,
    onSaveSettings,
    onUnexcludeIp,
    onOpenExcludedIps,
    onUpdateMods,
    onGoToUpdate,
  }: Props = $props();

  // ── Theme dropdown state ─────────────────────────────────────────────────
  let themeDropdownOpen = $state(false);
  let customThemeModalOpen = $state(false);
  let customCss = $state('');
  let originalCssOnOpen = $state('');
  // themeEditorTab moved into ThemeEditorModal (only relevant while open).

  // Default custom theme CSS template
  const customCssTemplate = `/* Custom Theme */
[data-theme="custom"] {
  /* ── Base surfaces ──────────────────────────────────────────────────────── */
  --color-base-100: oklch(15% 0.01 0);
  --color-base-200: oklch(20% 0.01 0);
  --color-base-300: oklch(28% 0.015 0);
  --color-base-content: oklch(90% 0.01 0);

  /* ── Brand colors ───────────────────────────────────────────────────────── */
  --color-primary: oklch(65% 0.20 255);
  --color-primary-content: oklch(10% 0.01 255);
  --color-secondary: oklch(60% 0.16 290);
  --color-secondary-content: oklch(95% 0.01 290);
  --color-accent: oklch(65% 0.16 180);
  --color-accent-content: oklch(10% 0.01 180);
  --color-neutral: oklch(20% 0.01 0);
  --color-neutral-content: oklch(90% 0.01 0);

  /* ── Status colors ──────────────────────────────────────────────────────── */
  --color-info: oklch(60% 0.16 230);
  --color-info-content: oklch(10% 0.01 230);
  --color-success: oklch(60% 0.18 145);
  --color-success-content: oklch(10% 0.01 145);
  --color-warning: oklch(70% 0.18 75);
  --color-warning-content: oklch(10% 0.02 75);
  --color-error: oklch(60% 0.22 25);
  --color-error-content: oklch(98% 0.01 25);

  /* ── App accent colors ──────────────────────────────────────────────────── */
  --color-accent-map: oklch(70% 0.18 75);
  --color-accent-mods: oklch(60% 0.16 290);
  --color-accent-stat-server: oklch(60% 0.22 25);
  --color-accent-stat-players: oklch(60% 0.18 145);
  --color-accent-stat-steam: oklch(60% 0.16 230);
  --color-accent-update: oklch(60% 0.18 145);
  --color-accent-stale: oklch(70% 0.18 75);
  --color-accent-highlight: oklch(65% 0.20 255);

  /* ── Terminal / Log colors ──────────────────────────────────────────────── */
  --color-terminal-bg: oklch(12% 0.01 240);
  --color-terminal-border: oklch(25% 0.01 240);
  --color-log-error: oklch(70% 0.20 25);
  --color-log-warning: oklch(80% 0.16 85);
  --color-log-success: oklch(72% 0.18 160);
  --color-log-info: oklch(75% 0.14 220);
  --color-log-default: oklch(65% 0.02 250);

  /* ── Syntax highlighting ────────────────────────────────────────────────── */
  --color-syntax-comment: oklch(55% 0.02 120);
  --color-syntax-selector: oklch(70% 0.18 200);
  --color-syntax-property: oklch(70% 0.16 280);
  --color-syntax-variable: oklch(75% 0.18 180);
  --color-syntax-string: oklch(70% 0.16 80);
  --color-syntax-number: oklch(70% 0.18 140);
  --color-syntax-function: oklch(70% 0.16 320);

  /* ── Feature colors ─────────────────────────────────────────────────────── */
  --color-feat-browser: oklch(60% 0.16 230);
  --color-feat-mods: oklch(60% 0.16 290);
  --color-feat-stats: oklch(60% 0.18 145);
  --color-feat-launch: oklch(70% 0.18 75);

  /* ── Badge colors ───────────────────────────────────────────────────────── */
  --color-badge-rank: oklch(70% 0.18 75);
  --color-badge-status: oklch(60% 0.22 25);
  --color-badge-country: oklch(60% 0.16 230);
  --color-badge-players: oklch(60% 0.16 290);
  --color-badge-firstperson: oklch(60% 0.16 290);
  --color-badge-official: oklch(60% 0.18 145);
  --color-badge-custom: oklch(65% 0.02 250);

  /* ── UI element colors ──────────────────────────────────────────────────── */
  --color-port-query: oklch(60% 0.16 230);
  --color-port-game: oklch(70% 0.18 75);
  --color-link: oklch(60% 0.16 230);
  --color-score: oklch(60% 0.18 145);
  --color-best: oklch(70% 0.18 75);

  /* ── Option group colors ────────────────────────────────────────────────── */
  --color-opt-display: oklch(60% 0.16 230);
  --color-opt-network: oklch(60% 0.18 145);
  --color-opt-launch: oklch(70% 0.18 75);
  --color-opt-input: oklch(65% 0.02 250);
  --color-opt-misc: oklch(60% 0.16 290);

  /* ── Tech brand colors ──────────────────────────────────────────────────── */
  --color-tech-tauri: oklch(60% 0.16 230);
  --color-tech-svelte: oklch(65% 0.22 30);
  --color-tech-rust: oklch(60% 0.16 50);
  --color-tech-daisyui: oklch(60% 0.16 290);
  --color-tech-tailwind: oklch(60% 0.16 200);

  /* ── Window control colors ──────────────────────────────────────────────── */
  --color-btn-close: oklch(60% 0.22 25);

  /* ── Border radius ──────────────────────────────────────────────────────── */
  --radius-btn: 0.375rem;
  --radius-box: 0.5rem;
  --radius-badge: 1rem;

  /* ── Logo ─────────────────────────────────────────────────────────────────── */
  --logo-invert: 0;
}`;

  // Default values from app.css - used as fallback when computed styles are empty
  const defaultCssValues: Record<string, string> = {
    // Base surfaces (from DaisyUI dark theme)
    '--color-base-100': 'oklch(25.33% 0.016 252.42)',
    '--color-base-200': 'oklch(23.26% 0.014 253.1)',
    '--color-base-300': 'oklch(21.15% 0.012 254.09)',
    '--color-base-content': 'oklch(97.807% 0.029 256.847)',
    // Brand colors (from DaisyUI dark theme)
    '--color-primary': 'oklch(58.12% 0.213 263.83)',
    '--color-primary-content': 'oklch(96.27% 0.014 264.53)',
    '--color-secondary': 'oklch(64.22% 0.2108 256.02)',
    '--color-secondary-content': 'oklch(94.31% 0.019 257.67)',
    '--color-accent': 'oklch(76.15% 0.176 70.08)',
    '--color-accent-content': 'oklch(15.07% 0.039 70.98)',
    '--color-neutral': 'oklch(27% 0.02 255)',
    '--color-neutral-content': 'oklch(98% 0.005 255)',
    // Status colors (from DaisyUI dark theme)
    '--color-info': 'oklch(70.12% 0.138 214.09)',
    '--color-info-content': 'oklch(15.07% 0.039 215.09)',
    '--color-success': 'oklch(76.81% 0.188 138.32)',
    '--color-success-content': 'oklch(15.07% 0.039 139.32)',
    '--color-warning': 'oklch(82.56% 0.178 79.93)',
    '--color-warning-content': 'oklch(15.07% 0.039 80.93)',
    '--color-error': 'oklch(62.8% 0.257 29.23)',
    '--color-error-content': 'oklch(15.07% 0.039 30.23)',
    // App accent colors (from app.css [data-theme])
    '--color-accent-map': 'oklch(75% 0.15 75)',
    '--color-accent-mods': 'oklch(75% 0.18 290)',
    '--color-accent-stat-server': 'oklch(70% 0.20 25)',
    '--color-accent-stat-players': 'oklch(75% 0.18 145)',
    '--color-accent-stat-steam': 'oklch(75% 0.16 230)',
    '--color-accent-update': 'oklch(75% 0.18 160)',
    '--color-accent-stale': 'oklch(80% 0.16 85)',
    '--color-accent-highlight': 'oklch(75% 0.16 230)',
    // Terminal / Log colors (from app.css [data-theme])
    '--color-terminal-bg': 'oklch(12% 0.01 240)',
    '--color-terminal-border': 'oklch(25% 0.01 240)',
    '--color-log-error': 'oklch(70% 0.20 25)',
    '--color-log-warning': 'oklch(80% 0.16 85)',
    '--color-log-success': 'oklch(72% 0.18 160)',
    '--color-log-info': 'oklch(75% 0.14 220)',
    '--color-log-default': 'oklch(65% 0.02 250)',
    // Syntax highlighting (from app.css [data-theme])
    '--color-syntax-comment': 'oklch(55% 0.02 120)',
    '--color-syntax-selector': 'oklch(70% 0.18 200)',
    '--color-syntax-property': 'oklch(70% 0.16 280)',
    '--color-syntax-variable': 'oklch(75% 0.18 180)',
    '--color-syntax-string': 'oklch(70% 0.16 80)',
    '--color-syntax-number': 'oklch(70% 0.18 140)',
    '--color-syntax-function': 'oklch(70% 0.16 320)',
    // Feature colors (from app.css [data-theme])
    '--color-feat-browser': 'oklch(75% 0.16 220)',
    '--color-feat-mods': 'oklch(75% 0.20 310)',
    '--color-feat-stats': 'oklch(75% 0.18 160)',
    '--color-feat-launch': 'oklch(75% 0.18 55)',
    // Badge colors (from app.css [data-theme])
    '--color-badge-rank': 'oklch(80% 0.16 85)',
    '--color-badge-status': 'oklch(70% 0.18 15)',
    '--color-badge-country': 'oklch(75% 0.16 220)',
    '--color-badge-players': 'oklch(72% 0.18 290)',
    '--color-badge-firstperson': 'oklch(75% 0.20 310)',
    '--color-badge-official': 'oklch(72% 0.18 160)',
    '--color-badge-custom': 'oklch(70% 0.14 180)',
    // UI element colors (from app.css [data-theme])
    '--color-port-query': 'oklch(75% 0.16 220)',
    '--color-port-game': 'oklch(80% 0.16 85)',
    '--color-link': 'oklch(70% 0.16 260)',
    '--color-score': 'oklch(72% 0.18 160)',
    '--color-best': 'oklch(80% 0.16 85)',
    // Option group colors (from app.css [data-theme])
    '--color-opt-display': 'oklch(70% 0.16 240)',
    '--color-opt-network': 'oklch(72% 0.18 160)',
    '--color-opt-launch': 'oklch(75% 0.18 55)',
    '--color-opt-input': 'oklch(70% 0.14 180)',
    '--color-opt-misc': 'oklch(70% 0.16 290)',
    // Tech brand colors (from app.css [data-theme])
    '--color-tech-tauri': 'oklch(75% 0.16 220)',
    '--color-tech-svelte': 'oklch(68% 0.20 30)',
    '--color-tech-rust': 'oklch(60% 0.16 50)',
    '--color-tech-daisyui': 'oklch(70% 0.18 290)',
    '--color-tech-tailwind': 'oklch(75% 0.16 200)',
    // Window control colors (from app.css [data-theme])
    '--color-btn-close': 'oklch(60% 0.25 25)',
    // Border radius
    '--radius-btn': '0.375rem',
    '--radius-box': '0.5rem',
    '--radius-badge': '1rem',
    // Logo inversion
    '--logo-invert': '0',
  };

  // Extract current CSS variables from computed styles and generate CSS
  function extractCurrentThemeCss(): string {
    // Read from the element with data-theme attribute (not document.documentElement)
    // to get the explicit theme colors, not the system default
    const themedEl = document.querySelector('[data-theme]') || document.documentElement;
    const style = getComputedStyle(themedEl);

    let css = `/* Custom Theme */
[data-theme="custom"] {
  /* ── Base surfaces ──────────────────────────────────────────────────────── */\n`;

    const sections: Record<string, string[]> = {
      'Base surfaces': ['--color-base-100', '--color-base-200', '--color-base-300', '--color-base-content'],
      'Brand colors': [
        '--color-primary',
        '--color-primary-content',
        '--color-secondary',
        '--color-secondary-content',
        '--color-accent',
        '--color-accent-content',
        '--color-neutral',
        '--color-neutral-content',
      ],
      'Status colors': [
        '--color-info',
        '--color-info-content',
        '--color-success',
        '--color-success-content',
        '--color-warning',
        '--color-warning-content',
        '--color-error',
        '--color-error-content',
      ],
      'App accent colors': [
        '--color-accent-map',
        '--color-accent-mods',
        '--color-accent-stat-server',
        '--color-accent-stat-players',
        '--color-accent-stat-steam',
        '--color-accent-update',
        '--color-accent-stale',
        '--color-accent-highlight',
      ],
      'Terminal / Log colors': [
        '--color-terminal-bg',
        '--color-terminal-border',
        '--color-log-error',
        '--color-log-warning',
        '--color-log-success',
        '--color-log-info',
        '--color-log-default',
      ],
      'Syntax highlighting': [
        '--color-syntax-comment',
        '--color-syntax-selector',
        '--color-syntax-property',
        '--color-syntax-variable',
        '--color-syntax-string',
        '--color-syntax-number',
        '--color-syntax-function',
      ],
      'Feature colors': ['--color-feat-browser', '--color-feat-mods', '--color-feat-stats', '--color-feat-launch'],
      'Badge colors': [
        '--color-badge-rank',
        '--color-badge-status',
        '--color-badge-country',
        '--color-badge-players',
        '--color-badge-firstperson',
        '--color-badge-official',
        '--color-badge-custom',
      ],
      'UI element colors': ['--color-port-query', '--color-port-game', '--color-link', '--color-score', '--color-best'],
      'Option group colors': [
        '--color-opt-display',
        '--color-opt-network',
        '--color-opt-launch',
        '--color-opt-input',
        '--color-opt-misc',
      ],
      'Tech brand colors': [
        '--color-tech-tauri',
        '--color-tech-svelte',
        '--color-tech-rust',
        '--color-tech-daisyui',
        '--color-tech-tailwind',
      ],
      'Window control colors': ['--color-btn-close'],
      'Border radius': ['--radius-btn', '--radius-box', '--radius-badge'],
      Logo: ['--logo-invert'],
    };

    let isFirst = true;
    for (const [sectionName, vars] of Object.entries(sections)) {
      if (!isFirst) {
        css += `\n  /* ── ${sectionName} ${'─'.repeat(Math.max(0, 66 - sectionName.length))} */\n`;
      }
      isFirst = false;
      for (const varName of vars) {
        // Read from computed styles, fallback to default values from app.css
        let value = style.getPropertyValue(varName).trim();
        if (!value) {
          value = defaultCssValues[varName] || '';
        }
        if (value) {
          css += `  ${varName}: ${value};\n`;
        }
      }
    }

    css += `}`;
    return css;
  }

  async function openCustomThemeModal() {
    // 'dark' and 'light' are built-in themes without custom CSS - extract current colors
    if (activePresetId === 'dark' || activePresetId === 'light' || !activePresetId) {
      // Extract the actual colors from the current theme
      customCss = extractCurrentThemeCss();
    } else {
      // For other presets, reload their CSS to ensure the editor shows the correct colors
      const css = await loadThemeCss(activePresetId as ThemePresetId);
      if (css) {
        customCss = css;
        localStorage.setItem('custom-theme-css', css);
      }
    }
    // Save original CSS for reset button
    originalCssOnOpen = customCss;
    // Always switch to custom theme when opening editor so user sees changes live
    onSetTheme('custom' as ThemeName);
    customThemeModalOpen = true;
    themeDropdownOpen = false;
  }

  // Get all presets organized by category
  const darkPresets = themePresets.filter((p) =>
    [
      'dark',
      'github_dark',
      'neutral',
      'midnight',
      'forest',
      'blood_moon',
      'military',
      'oled',
      'sepia',
      'nord',
      'dracula',
      'catppuccin',
      'tokyonight',
      'kanagawa',
      'rosepine',
      'gruvbox_dark',
    ].includes(p.id),
  );
  const lightPresets = themePresets.filter((p) =>
    [
      'github_light',
      'light',
      'ocean',
      'sand',
      'rose',
      'mint',
      'lavender',
      'catppuccin_latte',
      'tokyonight_light',
      'rosepine_dawn',
      'gruvbox_light',
    ].includes(p.id),
  );
  const mixedPresets = themePresets.filter((p) => ['twilight', 'mocha'].includes(p.id));

  function closeCustomThemeModal() {
    customThemeModalOpen = false;
    // themeEditorTab now lives inside ThemeEditorModal, which is unmounted
    // when customThemeModalOpen flips to false (resetting tab on next open).
  }

  // Color definitions for the visual editor
  // Color group definitions, getColorFromCss / updateColorInCss helpers all
  // moved into ThemeEditorModal — they're only used inside the editor.

  // Track which preset is currently active (for highlighting in dropdown)
  let activePresetId = $state<string | null>(localStorage.getItem('active-preset-id'));

  // Track system theme for "Default" logic - use prefers-color-scheme which reflects
  // the actual OS preference, not the current window theme (which changes when we apply themes)
  let systemTheme = $state<'dark' | 'light'>(
    typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark',
  );

  // Check if current selection matches system default (Default and system theme are linked)
  const isDefaultSelected = $derived(
    !activePresetId ||
      (systemTheme === 'dark' && activePresetId === 'dark') ||
      (systemTheme === 'light' && activePresetId === 'light'),
  );

  // Check if a preset should show as selected
  // Dark/Light are also selected when they match the system theme and Default is active
  function isPresetSelected(presetId: string): boolean {
    // For dark preset: selected if explicitly chosen OR system is dark with default
    if (presetId === 'dark') {
      return activePresetId === 'dark' || (systemTheme === 'dark' && !activePresetId);
    }
    // For light preset: selected if explicitly chosen OR system is light with default
    if (presetId === 'light') {
      return activePresetId === 'light' || (systemTheme === 'light' && !activePresetId);
    }
    // For other presets: just check if active
    return activePresetId === presetId;
  }

  // Auto-apply theme based on system preference on first run
  let hasInitialized = false;
  $effect(() => {
    if (hasInitialized) return;
    hasInitialized = true;

    // If this is the first run (no saved theme/preset), detect system preference using Tauri
    if (app.isFirstRun()) {
      getCurrentWindow()
        .theme()
        .then(async (systemTheme) => {
          if (systemTheme === 'light') {
            // Apply GitHub Light as default for light mode
            const css = await loadThemeCss('github_light');
            if (css) {
              customCss = css;
              localStorage.setItem('custom-theme-css', css);
              localStorage.setItem('active-preset-id', 'github_light');
              activePresetId = 'github_light';
              applyCustomTheme(css);
              onSetTheme('custom' as ThemeName);
            }
          }
          // For dark mode (default), we don't need to do anything - it's already the default
        });
    }
  });

  // Reset to the built-in default theme (no custom CSS)
  function resetToDefault() {
    // Clear custom theme CSS
    customCss = '';
    localStorage.removeItem('custom-theme-css');
    localStorage.removeItem('active-preset-id');
    activePresetId = null;
    // Remove custom theme style element
    const styleEl = document.getElementById('custom-theme-style');
    if (styleEl) {
      styleEl.remove();
    }
    // Switch to built-in theme matching system preference
    onSetTheme(systemTheme as ThemeName);
    themeDropdownOpen = false;
  }

  // Apply a preset (load CSS and switch to custom theme, or use built-in for dark/light)
  async function applyPreset(presetId: string) {
    // For 'dark' and 'light', use the built-in DaisyUI themes instead of custom CSS
    if (presetId === 'dark' || presetId === 'light') {
      localStorage.removeItem('custom-theme-css');
      localStorage.setItem('active-preset-id', presetId);
      activePresetId = presetId;
      // Remove custom theme style element
      const styleEl = document.getElementById('custom-theme-style');
      if (styleEl) {
        styleEl.remove();
      }
      // Switch to built-in theme
      onSetTheme(presetId as ThemeName);
      themeDropdownOpen = false;
      // If modal is open, extract current theme colors after theme applies
      if (customThemeModalOpen) {
        setTimeout(() => {
          customCss = extractCurrentThemeCss();
          applyCustomTheme(customCss);
          onSetTheme('custom' as ThemeName);
        }, 0);
      } else {
        customCss = '';
      }
      return;
    }

    // For other presets, load their CSS
    const css = await loadThemeCss(presetId as ThemePresetId);
    if (css) {
      customCss = css;
      localStorage.setItem('custom-theme-css', css);
      localStorage.setItem('active-preset-id', presetId);
      activePresetId = presetId;
      applyCustomTheme(css);
      onSetTheme('custom' as ThemeName);
      themeDropdownOpen = false;
    }
  }

  // Preset icons mapping
  const presetIcons: Record<string, string> = {
    // Dark
    dark: 'ph:moon',
    github_dark: 'mdi:github',
    neutral: 'ph:circle-dashed',
    midnight: 'ph:moon-stars',
    forest: 'ph:tree',
    blood_moon: 'ph:drop',
    military: 'ph:shield-chevron',
    oled: 'ph:circle-half',
    sepia: 'ph:film-strip',
    nord: 'ph:snowflake',
    dracula: 'ph:ghost',
    catppuccin: 'ph:cat',
    tokyonight: 'ph:city',
    kanagawa: 'game-icons:big-wave',
    rosepine: 'ph:flower-lotus',
    gruvbox_dark: 'ph:tree-evergreen',
    // Light
    github_light: 'mdi:github',
    light: 'ph:sun',
    ocean: 'ph:waves',
    sand: 'ph:sun-horizon',
    rose: 'ph:flower-lotus',
    mint: 'ph:leaf',
    lavender: 'ph:butterfly',
    catppuccin_latte: 'ph:cat',
    tokyonight_light: 'ph:city',
    rosepine_dawn: 'ph:sun-horizon',
    gruvbox_light: 'ph:tree-evergreen',
    // Mixed
    twilight: 'ph:cloud-sun',
    mocha: 'ph:coffee',
  };

  // Get preset label from messages
  function getPresetLabel(presetId: string): string {
    switch (presetId) {
      // Dark
      case 'dark':
        return m.theme_preset_dark();
      case 'github_dark':
        return m.theme_preset_github_dark();
      case 'neutral':
        return m.theme_preset_neutral();
      case 'midnight':
        return m.theme_preset_midnight();
      case 'forest':
        return m.theme_preset_forest();
      case 'blood_moon':
        return m.theme_preset_blood_moon();
      case 'military':
        return m.theme_preset_military();
      case 'oled':
        return m.theme_preset_oled();
      case 'sepia':
        return m.theme_preset_sepia();
      case 'nord':
        return m.theme_preset_nord();
      case 'dracula':
        return m.theme_preset_dracula();
      case 'catppuccin':
        return m.theme_preset_catppuccin();
      case 'tokyonight':
        return m.theme_preset_tokyonight();
      case 'kanagawa':
        return m.theme_preset_kanagawa();
      case 'rosepine':
        return m.theme_preset_rosepine();
      case 'gruvbox_dark':
        return m.theme_preset_gruvbox_dark();
      // Light
      case 'github_light':
        return m.theme_preset_github_light();
      case 'light':
        return m.theme_preset_light();
      case 'ocean':
        return m.theme_preset_ocean();
      case 'sand':
        return m.theme_preset_sand();
      case 'rose':
        return m.theme_preset_rose();
      case 'mint':
        return m.theme_preset_mint();
      case 'lavender':
        return m.theme_preset_lavender();
      case 'catppuccin_latte':
        return m.theme_preset_catppuccin_latte();
      case 'tokyonight_light':
        return m.theme_preset_tokyonight_light();
      case 'rosepine_dawn':
        return m.theme_preset_rosepine_dawn();
      case 'gruvbox_light':
        return m.theme_preset_gruvbox_light();
      // Mixed
      case 'twilight':
        return m.theme_preset_twilight();
      case 'mocha':
        return m.theme_preset_mocha();
      default:
        return presetId;
    }
  }

  // Load a preset into the editor (without switching theme)
  async function loadPresetIntoEditor(presetId: string) {
    const css = await loadThemeCss(presetId as ThemePresetId);
    if (css) {
      customCss = css;
    }
  }

  // Export theme as JSON file
  async function exportTheme() {
    const data = {
      version: 1,
      name: 'Custom Theme',
      css: customCss,
    };
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'dayz-hub-theme.json';
    a.click();
    URL.revokeObjectURL(url);
  }

  // Import theme from JSON file
  async function importTheme() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const data = JSON.parse(text);
        if (data.css && typeof data.css === 'string') {
          customCss = data.css;
        }
      } catch {
        // Invalid file
      }
    };
    input.click();
  }

  function applyCustomTheme(css: string) {
    let styleEl = document.getElementById('custom-theme-style');
    if (!styleEl) {
      styleEl = document.createElement('style');
      styleEl.id = 'custom-theme-style';
      document.head.appendChild(styleEl);
    }
    styleEl.textContent = css;
  }

  // Load custom theme CSS on mount (only if theme is 'custom')
  $effect(() => {
    if (theme === 'custom') {
      const saved = localStorage.getItem('custom-theme-css');
      if (saved) {
        customCss = saved;
      }
    } else {
      // Clear activePresetId when theme is not custom, EXCEPT for built-in 'dark'/'light' presets
      if (activePresetId && activePresetId !== 'dark' && activePresetId !== 'light') {
        activePresetId = null;
        localStorage.removeItem('active-preset-id');
      }
    }
  });

  // Apply custom CSS only when theme is 'custom', remove it otherwise
  $effect(() => {
    if (theme === 'custom') {
      applyCustomTheme(customCss);
      // Save to localStorage on every change
      if (customCss) {
        localStorage.setItem('custom-theme-css', customCss);
      }
    } else {
      // Remove custom theme styles when not using custom theme
      const styleEl = document.getElementById('custom-theme-style');
      if (styleEl) {
        styleEl.remove();
      }
    }
  });

  function handleThemeKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      themeDropdownOpen = false;
    }
  }

  // Modal keydown handler now lives in ThemeEditorModal.

  $effect(() => {
    if (themeDropdownOpen) {
      const handleClickOutside = (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        if (!target.closest('.theme-dropdown')) {
          themeDropdownOpen = false;
        }
      };
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  });

  const currentTheme = $derived(THEMES.find((t) => t.id === theme) ?? THEMES[0]);
  const currentIcon = $derived(activePresetId ? (presetIcons[activePresetId] ?? 'ph:palette') : currentTheme.icon);

  // ── Window controls ────────────────────────────────────────────────────────
  const win = getCurrentWindow();
  const minimize = () => win.minimize();
  const toggleMaximize = () => win.toggleMaximize();
  const close = () => win.close();

  function onTitlebarMousedown(e: MouseEvent) {
    // Only drag on primary button, and not when clicking interactive children
    if (e.buttons !== 1) return;
    const target = e.target as HTMLElement;
    if (target.closest('button, input, a, [data-no-drag]')) return;
    if (e.detail === 2) {
      toggleMaximize();
    } else {
      win.startDragging();
    }
  }

  // ── Account modal state ────────────────────────────────────────────────────
  let logoHovered = $state(false);

  let modalOpen = $state(false);
  let playerName = $state('');
  let steamLogin = $state('');
  let steamPassword = $state('');
  let steamRoot = $state('');
  let steamApiKey = $state('');
  let steamId = $state('');
  let steamcmdPath = $state('');
  let battlemetricsApiKey = $state('');
  let userLocation = $state<[number, number] | null>(null);
  let detectingLocation = $state(false);
  let showPassword = $state(false);
  let showApiKey = $state(false);
  let showBmKey = $state(false);

  function openModal() {
    playerName = profile?.player ?? '';
    steamLogin = profile?.steam_login ?? '';
    steamPassword = profile?.steam_password ?? '';
    steamRoot = profile?.steam_root ?? '';
    steamcmdPath = profile?.steamcmd_path ?? '';
    steamApiKey = profile?.steam_api_key ?? '';
    steamId = profile?.steam_id ?? '';
    battlemetricsApiKey = profile?.battlemetrics_api_key ?? '';
    userLocation = profile?.user_location ?? null;
    manualLat = profile?.user_location ? profile.user_location[1].toFixed(4) : '';
    manualLon = profile?.user_location ? profile.user_location[0].toFixed(4) : '';
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    showPassword = false;
    showApiKey = false;
    showBmKey = false;
    modalOpen = true;
  }

  function closeModal() {
    modalOpen = false;
  }

  function handleOk() {
    onSaveSettings(
      playerName.trim() || null,
      steamLogin.trim() || null,
      steamPassword || null,
      steamRoot.trim() || null,
      profile?.steamcmd_enabled ?? true,
      steamcmdPath.trim() || null,
      steamApiKey.trim() || null,
      steamId.trim() || null,
      battlemetricsApiKey.trim() || null,
      userLocation,
    );
    closeModal();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleOk();
    if (e.key === 'Escape') closeModal();
  }

  async function browseSteamRoot() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: m.settings_select_steam_root(),
    });
    if (selected) steamRoot = selected as string;
  }

  async function clearSteamPassword() {
    const yes = await ask(
      'Remove the Steam password from profile.json? SteamCMD will fall back to cached credentials.',
      {
        title: 'Clear Steam password',
        kind: 'warning',
        okLabel: 'Remove',
        cancelLabel: 'Cancel',
      },
    );
    if (!yes) return;
    steamPassword = '';
    onSaveSettings(
      playerName.trim() || null,
      steamLogin.trim() || null,
      null,
      steamRoot.trim() || null,
      profile?.steamcmd_enabled ?? true,
      steamcmdPath.trim() || null,
      steamApiKey.trim() || null,
      steamId.trim() || null,
      battlemetricsApiKey.trim() || null,
      userLocation,
    );
  }

  let manualLat = $state('');
  let manualLon = $state('');
  let locationError = $state('');
  let detectedCity = $state('');
  let detectedCountry = $state('');

  async function detectLocationByIp() {
    detectingLocation = true;
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    try {
      const res = await fetch('http://ip-api.com/json/?fields=status,message,lat,lon,city,country,countryCode');
      const data = await res.json();
      if (data.status === 'success') {
        userLocation = [data.lon, data.lat];
        manualLat = data.lat.toFixed(4);
        manualLon = data.lon.toFixed(4);
        detectedCity = data.city || '';
        detectedCountry = data.country || '';
      } else {
        locationError = data.message || 'IP geolocation failed';
      }
    } catch (e) {
      locationError = 'Network error';
      console.error('IP geolocation failed:', e);
    } finally {
      detectingLocation = false;
    }
  }

  function applyManualLocation() {
    const lat = parseFloat(manualLat);
    const lon = parseFloat(manualLon);
    if (isNaN(lat) || isNaN(lon) || lat < -90 || lat > 90 || lon < -180 || lon > 180) {
      locationError = 'Invalid coordinates';
      return;
    }
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    userLocation = [lon, lat];
  }

  function clearLocation() {
    userLocation = null;
    manualLat = '';
    manualLon = '';
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
  }

  function fmt(n: number | null | undefined): string {
    return n == null ? '—' : n.toLocaleString();
  }
</script>

<!-- ── Title bar ──────────────────────────────────────────────────────────── -->
<!-- Sits above modals (z-[1001] beats DaisyUI modal z-[1000]) so window
     controls and drag-to-move remain usable while any modal is open. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="h-9 flex items-center bg-base-200 border-b border-base-300 flex-shrink-0 select-none relative z-[1001]"
  onmousedown={onTitlebarMousedown}
  onmouseenter={() => (logoHovered = true)}
  onmouseleave={() => (logoHovered = false)}
>
  <!-- Left: identity — fixed width so glitch chars never shift adjacent elements -->
  <div
    class="flex items-center gap-2 px-4 pr-4 border-r border-base-300 shrink-0 overflow-hidden titlebar-identity"
    role="presentation"
  >
    <img src="/icon.svg" class="w-5 h-5 titlebar-logo" class:titlebar-logo--hovered={logoHovered} alt="icon" />
    <GlitchText
      text="DayZ Community Hub"
      class="text-sm font-semibold text-base-content tracking-tight font-mono whitespace-nowrap"
      externalTrigger={glitchTick}
    />
  </div>

  <!-- Center: live stats — absolutely centred so neither side affects its position -->
  <div
    class="absolute left-1/2 -translate-x-1/2 flex items-center gap-5 px-4 text-xs text-base-content/60 pointer-events-none"
  >
    <span class="flex items-center gap-1.5 pointer-events-auto" title={m.titlebar_servers()}>
      <Icon icon="mdi:server-network" class="size-3.5 text-accent-stat-server" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.server_count)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title={m.titlebar_players_ingame()}>
      <Icon icon="mdi:controller" class="size-3.5 text-accent-stat-players" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.total_players)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title={m.titlebar_players_steam()}>
      <Icon icon="mdi:steam" class="size-3.5 text-accent-stat-steam" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(steamPlayers)}</span>
    </span>
  </div>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Right: user + theme + window controls -->
  <div class="flex items-center gap-1 text-xs">
    {#if stats && !stats.has_steamcmd}
      <button
        class="flex items-center gap-1 text-warning mr-2 hover:text-warning/80 transition-colors"
        title={m.titlebar_steamcmd_missing()}
        onclick={openModal}
      >
        <Icon icon="ph:warning" class="size-3.5" />
        <span>{m.titlebar_steamcmd_missing()}</span>
      </button>
    {/if}

    <!-- Launcher update badge — only shown when an update is available -->
    {#if updateState === 'available'}
      <button
        class="flex items-center justify-center px-1.5 py-1 rounded text-accent-update hover:opacity-80 hover:bg-base-300 transition-colors border-r border-base-300 mr-1"
        onclick={onGoToUpdate}
        title={m.titlebar_update_available_title()}
        data-no-drag
      >
        <Icon icon="line-md:downloading-loop" class="size-4" />
      </button>
    {/if}

    <!-- Mod update badge — only shown when stale mods exist -->
    {#if staleModCount > 0}
      <button
        class="flex items-center justify-center gap-1 px-1.5 py-1 rounded text-accent-stale hover:opacity-80 hover:bg-base-300 transition-colors border-r border-base-300 mr-1"
        onclick={onUpdateMods}
        title={staleModCount === 1
          ? m.titlebar_update_mods_title_one({ count: staleModCount })
          : m.titlebar_update_mods_title({ count: staleModCount })}
        data-no-drag
      >
        <Icon icon="line-md:download-outline-loop" class="size-4" />
        <span class="text-xs font-bold tabular-nums">{staleModCount}</span>
      </button>
    {/if}

    <!-- User chip -->
    <button
      class="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-base-300 text-base-content/70 hover:text-primary transition-colors border-r border-base-300 mr-1"
      onclick={openModal}
      title={m.titlebar_edit_account()}
    >
      {#if avatarUrl}
        <img
          src={avatarUrl}
          alt="Steam avatar"
          class="size-5 rounded-full object-cover ring-1 ring-base-300 flex-shrink-0"
        />
      {:else}
        <Icon icon="ph:user-circle" class="size-3.5 text-base-content/40 flex-shrink-0" />
      {/if}
      {#if stats?.player_name}
        <span class="font-medium">{stats.player_name}</span>
        {#if stats.steam_login}
          <span class="text-base-content/40">({stats.steam_login})</span>
        {/if}
      {:else}
        <span class="italic text-base-content/40">{m.titlebar_setup_account()}</span>
      {/if}
      <Icon icon="ph:pencil-simple" class="size-3 text-base-content/30" />
    </button>

    <!-- Language selector -->
    <LanguageSelector />

    <!-- Theme selector -->
    <div class="relative theme-dropdown" onkeydown={handleThemeKeydown}>
      <button
        class="inline-flex items-center justify-center gap-1.5 h-9 px-2 text-base-content/50 hover:bg-base-300 hover:text-base-content transition-colors"
        onclick={(e) => {
          e.stopPropagation();
          themeDropdownOpen = !themeDropdownOpen;
        }}
        title={m.theme_change()}
      >
        <Icon icon={currentIcon} class="size-4" />
        <Icon icon="ph:caret-down" class="size-3 opacity-50" />
      </button>

      {#if themeDropdownOpen}
        <div
          class="absolute right-0 top-full mt-1 w-44 bg-base-200 border border-base-300 rounded-lg shadow-xl z-50 py-1 overflow-hidden max-h-[420px] overflow-y-auto"
        >
          <!-- Reset to Default option -->
          <button
            class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {isDefaultSelected
              ? 'text-primary bg-base-300/50'
              : 'text-base-content/70'}"
            onclick={resetToDefault}
          >
            <Icon icon="ph:arrow-counter-clockwise" class="size-4 shrink-0" />
            <span class="flex-1">{m.theme_default()}</span>
            {#if isDefaultSelected}
              <Icon icon="ph:check" class="size-4 text-primary" />
            {/if}
          </button>

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Dark themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">
            {m.theme_dark()}
          </div>
          {#each darkPresets as preset}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {isPresetSelected(
                preset.id,
              )
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => applyPreset(preset.id)}
            >
              <Icon icon={presetIcons[preset.id] ?? 'ph:palette'} class="size-4 shrink-0" />
              <span class="flex-1">{getPresetLabel(preset.id)}</span>
              {#if isPresetSelected(preset.id)}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Light themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">
            {m.theme_light()}
          </div>
          {#each lightPresets as preset}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {isPresetSelected(
                preset.id,
              )
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => applyPreset(preset.id)}
            >
              <Icon icon={presetIcons[preset.id] ?? 'ph:palette'} class="size-4 shrink-0" />
              <span class="flex-1">{getPresetLabel(preset.id)}</span>
              {#if isPresetSelected(preset.id)}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Mixed themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">
            {m.theme_mixed()}
          </div>
          {#each mixedPresets as preset}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {isPresetSelected(
                preset.id,
              )
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => applyPreset(preset.id)}
            >
              <Icon icon={presetIcons[preset.id] ?? 'ph:palette'} class="size-4 shrink-0" />
              <span class="flex-1">{getPresetLabel(preset.id)}</span>
              {#if isPresetSelected(preset.id)}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Edit custom theme button -->
          <button
            class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {theme ===
            'custom'
              ? 'text-primary bg-base-300/50'
              : 'text-base-content/70'}"
            onclick={openCustomThemeModal}
          >
            <Icon icon="ph:pencil-simple" class="size-4 shrink-0" />
            <span class="flex-1">{m.theme_edit_custom()}</span>
          </button>
        </div>
      {/if}
    </div>

    <!-- Window controls -->
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-base-300 hover:text-base-content transition-colors"
      onclick={minimize}
      title={m.window_minimize()}
    >
      <Icon icon="mdi:minus" class="size-3.5" />
    </button>
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-base-300 hover:text-base-content transition-colors"
      onclick={toggleMaximize}
      title={m.window_maximize()}
    >
      <Icon icon="mdi:checkbox-blank-outline" class="size-3" />
    </button>
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-btn-close hover:text-white transition-colors"
      onclick={close}
      title={m.window_close()}
    >
      <Icon icon="mdi:close" class="size-3.5" />
    </button>
  </div>
</div>

<!-- ── Account settings modal ─────────────────────────────────────────────── -->
{#if modalOpen}
  <div
    class="fixed inset-0 modal-backdrop-window z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
    onclick={closeModal}
  >
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-md mx-4 flex flex-col overflow-hidden max-h-[90vh]"
      role="dialog"
      aria-modal="true"
      aria-label={m.settings_account()}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <!-- ── Header: identity preview ──────────────────────────────────────── -->
      <div class="flex items-center gap-3 px-5 py-4 bg-base-200 border-b border-base-300 flex-shrink-0">
        <!-- Avatar preview -->
        <div
          class="size-10 rounded-full bg-base-300 border border-base-300 overflow-hidden flex items-center justify-center flex-shrink-0"
        >
          {#if avatarUrl}
            <img src={avatarUrl} alt="Steam avatar" class="w-full h-full object-cover" />
          {:else}
            <Icon icon="ph:user" class="size-5 text-base-content/30" />
          {/if}
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-base-content leading-tight truncate">
            {playerName || m.settings_unnamed_player()}
          </p>
          <p class="text-xs text-base-content/50 truncate">
            {steamLogin ? m.settings_steam_linked({ login: steamLogin }) : m.settings_no_steam_linked()}
          </p>
        </div>
        <button
          class="size-7 rounded flex items-center justify-center text-base-content/40 hover:bg-base-300 hover:text-base-content transition-colors flex-shrink-0"
          onclick={closeModal}
          aria-label={m.settings_close()}
          title={m.settings_close()}
        >
          <Icon icon="ph:x" class="size-3.5" />
        </button>
      </div>

      <!-- ── Scrollable body ───────────────────────────────────────────────── -->
      <div class="flex-1 overflow-y-auto p-5 space-y-5">
        <!-- ── Section: Identity ──────────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:game-controller" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider"
              >{m.settings_identity()}</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- In-game name -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-player"
                >{m.settings_ingame_name()}</label
              >
              <input
                id="field-player"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder={m.settings_ingame_name_placeholder()}
                autocomplete="nickname"
                bind:value={playerName}
              />
            </div>
          </div>
        </div>

        <!-- ── Section: Steam Login ───────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="mdi:steam" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider"
              >{m.settings_steam_login()}</span
            >
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >{m.settings_steam_login_desc()}</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- Username -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-login">{m.settings_username()}</label
              >
              <input
                id="field-login"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder={m.titlebar_anonymous()}
                autocomplete="username"
                bind:value={steamLogin}
              />
            </div>
            <!-- Password -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-password"
                >{m.settings_password()}</label
              >
              <div class="flex-1 flex items-center gap-1.5">
                {#if showPassword}
                  <input
                    id="field-password"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder={m.settings_password_placeholder()}
                    autocomplete="current-password"
                    bind:value={steamPassword}
                  />
                {:else}
                  <input
                    id="field-password"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder={m.settings_password_placeholder()}
                    autocomplete="current-password"
                    bind:value={steamPassword}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showPassword = !showPassword)}
                  title={showPassword ? m.settings_hide() : m.settings_show()}
                >
                  <Icon icon={showPassword ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
                {#if steamPassword}
                  <button
                    type="button"
                    class="text-base-content/30 hover:text-error transition-colors shrink-0"
                    onclick={clearSteamPassword}
                    title={m.settings_clear_password()}
                  >
                    <Icon icon="ph:x-circle" class="size-3.5" />
                  </button>
                {/if}
              </div>
            </div>
            <!-- Steam root -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-root"
                >{m.settings_steam_root()}</label
              >
              <input
                id="field-root"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                placeholder="~/.steam/steam"
                bind:value={steamRoot}
              />
              <button
                class="text-base-content/35 hover:text-primary transition-colors shrink-0"
                onclick={browseSteamRoot}
                title={m.settings_browse()}
              >
                <Icon icon="ph:folder-open" class="size-3.5" />
              </button>
            </div>
            <!-- SteamCMD path -->
            <div class="flex items-center gap-3 px-3 py-2.5">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-steamcmd"
                >{m.settings_steamcmd_path()}</label
              >
              <input
                id="field-steamcmd"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                placeholder="auto-detect"
                bind:value={steamcmdPath}
              />
              <button
                class="text-base-content/35 hover:text-primary transition-colors shrink-0"
                onclick={async () => {
                  const selected = await openDialog({ multiple: false, title: m.settings_select_steamcmd() });
                  if (selected) steamcmdPath = selected as string;
                }}
                title={m.settings_browse()}
              >
                <Icon icon="ph:folder-open" class="size-3.5" />
              </button>
            </div>
          </div>
          <!-- Plaintext warning -->
          {#if steamPassword}
            <div class="flex items-start gap-2 mt-2 px-3 py-2 rounded-lg bg-warning/10 border border-warning/30">
              <Icon icon="ph:warning" class="size-3.5 text-warning shrink-0 mt-0.5" />
              <p class="text-xs text-warning leading-snug">
                {m.settings_password_warning()}
              </p>
            </div>
          {/if}
        </div>

        <!-- ── Section: Steam API ─────────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:identification-card" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider"
              >{m.settings_steam_api()}</span
            >
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >{m.settings_steam_api_desc()}</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- API key -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-apikey">{m.settings_api_key()}</label
              >
              <div class="flex-1 flex items-center gap-1.5">
                {#if showApiKey}
                  <input
                    id="field-apikey"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="steamcommunity.com/dev/apikey"
                    autocomplete="off"
                    bind:value={steamApiKey}
                  />
                {:else}
                  <input
                    id="field-apikey"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="steamcommunity.com/dev/apikey"
                    autocomplete="off"
                    bind:value={steamApiKey}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showApiKey = !showApiKey)}
                  title={showApiKey ? m.settings_hide() : m.settings_show()}
                >
                  <Icon icon={showApiKey ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
              </div>
            </div>
            <!-- Steam ID -->
            <div class="flex items-center gap-3 px-3 py-2.5">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-steamid"
                >{m.settings_steam_id()}</label
              >
              <input
                id="field-steamid"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder="76561198000000000"
                bind:value={steamId}
              />
            </div>
          </div>
        </div>

        <!-- ── Section: BattleMetrics ─────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:chart-line-up" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider"
              >{m.settings_battlemetrics()}</span
            >
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >{m.settings_battlemetrics_desc()}</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- API token row -->
            <div class="flex items-center gap-3 px-3 py-2.5">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-bmkey"
                >{m.settings_api_token()}</label
              >
              <div class="flex-1 flex items-center gap-1.5">
                {#if showBmKey}
                  <input
                    id="field-bmkey"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="battlemetrics.com/developers"
                    autocomplete="off"
                    bind:value={battlemetricsApiKey}
                  />
                {:else}
                  <input
                    id="field-bmkey"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="battlemetrics.com/developers"
                    autocomplete="off"
                    bind:value={battlemetricsApiKey}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showBmKey = !showBmKey)}
                  title={showBmKey ? m.settings_hide() : m.settings_show()}
                >
                  <Icon icon={showBmKey ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
              </div>
            </div>
          </div>
          <p class="text-xs text-base-content/35 mt-1.5 px-1">
            {m.settings_get_token_at()}
            <button
              type="button"
              class="text-primary hover:underline"
              onclick={() => {
                openUrl('https://www.battlemetrics.com/developers');
              }}>battlemetrics.com/developers</button
            >
          </p>
        </div>

        <!-- ── Section: Location ──────────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:map-pin" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider"
              >{m.settings_your_location()}</span
            >
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >{m.settings_location_desc()}</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <div class="px-3 py-3 space-y-2.5">

              {#if userLocation}
                <!-- Location set: show nice card -->
                <div class="flex items-center gap-3 px-3 py-2.5 rounded-lg bg-success/5 border border-success/20">
                  <div class="size-8 rounded-full bg-success/10 flex items-center justify-center shrink-0">
                    <Icon icon="ph:map-pin-fill" class="size-4 text-success" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-base-content truncate">
                      {#if detectedCity || detectedCountry}
                        {detectedCity}{detectedCity && detectedCountry ? ', ' : ''}{detectedCountry}
                      {:else}
                        {m.settings_location_set()}
                      {/if}
                    </p>
                    <p class="text-xs text-base-content/40 font-mono">
                      {userLocation[1].toFixed(4)}, {userLocation[0].toFixed(4)}
                    </p>
                  </div>
                  <div class="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs btn-square text-base-content/40 hover:text-primary"
                      onclick={() => openUrl(`https://www.google.com/maps?q=${userLocation![1]},${userLocation![0]}`)}
                      title={m.settings_open_maps()}
                    >
                      <Icon icon="ph:map-trifold" class="size-4" />
                    </button>
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs btn-square text-base-content/40 hover:text-error"
                      onclick={clearLocation}
                      title={m.settings_clear_location()}
                    >
                      <Icon icon="ph:trash" class="size-4" />
                    </button>
                  </div>
                </div>
              {:else}
                <!-- No location: show detection options -->
                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    class="btn btn-sm btn-primary gap-1.5 flex-1"
                    onclick={detectLocationByIp}
                    disabled={detectingLocation}
                  >
                    {#if detectingLocation}
                      <span class="loading loading-spinner loading-xs"></span>
                      {m.settings_detecting()}
                    {:else}
                      <Icon icon="ph:crosshair" class="size-4" />
                      {m.settings_autodetect_ip()}
                    {/if}
                  </button>
                </div>
              {/if}

              <!-- Manual input (always visible, collapsed style) -->
              <div class="flex items-center gap-2 pt-1">
                <span class="text-xs text-base-content/35">{m.settings_manual()}</span>
                <input
                  type="text"
                  class="w-20 px-2 py-1 rounded bg-base-300/40 text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none border border-transparent focus:border-primary/50"
                  placeholder={m.settings_lat()}
                  aria-label={m.settings_lat()}
                  bind:value={manualLat}
                />
                <input
                  type="text"
                  class="w-20 px-2 py-1 rounded bg-base-300/40 text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none border border-transparent focus:border-primary/50"
                  placeholder={m.settings_lon()}
                  aria-label={m.settings_lon()}
                  bind:value={manualLon}
                />
                <button
                  type="button"
                  class="btn btn-ghost btn-xs gap-1"
                  onclick={applyManualLocation}
                  disabled={!manualLat || !manualLon}
                >
                  <Icon icon="ph:check" class="size-3.5" />
                </button>
              </div>

              {#if locationError}
                <div class="flex items-center gap-1.5 px-2 py-1.5 rounded bg-error/10 text-error text-xs">
                  <Icon icon="ph:warning-circle" class="size-3.5 shrink-0" />
                  {locationError}
                </div>
              {/if}
            </div>
          </div>
          <p class="text-xs text-base-content/35 mt-1.5 px-1">{m.settings_location_help()}</p>
        </div>
      </div>

      <!-- ── Footer ─────────────────────────────────────────────────────────── -->
      <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
        <button class="btn btn-ghost btn-sm text-base-content/60" onclick={closeModal}>{m.settings_cancel()}</button>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-ghost btn-sm gap-1.5 text-base-content/50"
            onclick={() => {
              onOpenExcludedIps();
            }}
            title={m.settings_manage_excluded()}
          >
            <Icon icon="ph:prohibit" class="size-3.5" />
            {m.settings_excluded_ips()}
            {#if (profile?.excluded_ips?.length ?? 0) > 0}
              <span class="badge badge-xs badge-error/70 text-error font-mono">{profile!.excluded_ips!.length}</span>
            {/if}
          </button>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={handleOk}>
            <Icon icon="ph:check" class="size-3.5" />
            {m.settings_save()}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- ── Custom theme modal (lazy-loaded — see ThemeEditorModal.svelte) ─────── -->
{#if customThemeModalOpen}
  {#await lazyThemeEditorModal() then mod}
    {@const ThemeEditorModal = mod.default}
    <ThemeEditorModal
      bind:customCss
      {originalCssOnOpen}
      {platform}
      onClose={closeCustomThemeModal}
      onImportTheme={importTheme}
      onExportTheme={exportTheme}
    />
  {/await}
{/if}

<style>
  .titlebar-logo {
    transition: transform 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .titlebar-logo--hovered {
    transform: rotate(15deg) scale(1.1);
  }
</style>

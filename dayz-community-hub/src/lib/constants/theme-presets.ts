/**
 * Theme preset definitions with lazy-loaded CSS
 * CSS files are loaded on-demand to optimize initial bundle size
 */

// All theme IDs
export const themeIds = [
  // Dark themes
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
  // Light themes
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
  // Mixed themes
  'twilight',
  'mocha',
] as const;

export type ThemePresetId = (typeof themeIds)[number];

// Lazy-load theme CSS files using Vite's glob import
const themeModules = import.meta.glob<string>('./themes/*.css', {
  query: '?raw',
  import: 'default',
});

// Cache for loaded themes
const themeCache = new Map<string, string>();

/**
 * Load a theme's CSS by its ID
 * Returns cached version if already loaded
 */
export async function loadThemeCss(id: ThemePresetId): Promise<string> {
  // Check cache first
  const cached = themeCache.get(id);
  if (cached) return cached;

  // Load the CSS file
  const path = `./themes/${id}.css`;
  const loader = themeModules[path];

  if (!loader) {
    console.warn(`Theme "${id}" not found`);
    return '';
  }

  const css = await loader();
  themeCache.set(id, css);
  return css;
}

/**
 * Preload multiple themes (useful for commonly used themes)
 */
export async function preloadThemes(ids: ThemePresetId[]): Promise<void> {
  await Promise.all(ids.map(id => loadThemeCss(id)));
}

// For backwards compatibility - returns theme objects with just IDs
// CSS is loaded separately via loadThemeCss()
export const themePresets = themeIds.map(id => ({ id }));

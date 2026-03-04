<script lang="ts">
  import { parseOklch, formatOklch, oklchToHex, hexToOklch } from '$lib/utils/oklch';

  interface Props {
    label: string;
    value: string; // OKLCH string like "oklch(65% 0.20 255)"
    onChange: (value: string) => void;
  }

  let { label, value, onChange }: Props = $props();

  // Convert OKLCH to hex for the color input
  const hexValue = $derived(() => {
    const parsed = parseOklch(value);
    if (!parsed) return '#808080';
    return oklchToHex(parsed);
  });

  // Local state for the hex input field
  let hexInput = $state('');
  let isEditing = $state(false);

  // Sync hex input when not editing
  $effect(() => {
    if (!isEditing) {
      hexInput = hexValue().toUpperCase();
    }
  });

  function handleColorPickerChange(e: Event) {
    const hex = (e.target as HTMLInputElement).value;
    const oklch = hexToOklch(hex);
    onChange(formatOklch(oklch));
  }

  function handleHexInput(e: Event) {
    let hex = (e.target as HTMLInputElement).value.toUpperCase();
    // Remove any non-hex characters except #
    hex = hex.replace(/[^#0-9A-F]/g, '');
    // Ensure it starts with #
    if (!hex.startsWith('#')) {
      hex = '#' + hex;
    }
    // Limit to 7 characters (#RRGGBB)
    hex = hex.slice(0, 7);
    hexInput = hex;
  }

  function handleHexBlur() {
    isEditing = false;
    // Validate and apply the hex color
    let hex = hexInput;
    if (hex.length === 4) {
      // Expand shorthand #RGB to #RRGGBB
      hex = '#' + hex[1] + hex[1] + hex[2] + hex[2] + hex[3] + hex[3];
    }
    if (/^#[0-9A-F]{6}$/i.test(hex)) {
      const oklch = hexToOklch(hex);
      onChange(formatOklch(oklch));
    } else {
      // Reset to current value if invalid
      hexInput = hexValue().toUpperCase();
    }
  }

  function handleHexKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      (e.target as HTMLInputElement).blur();
    } else if (e.key === 'Escape') {
      isEditing = false;
      hexInput = hexValue().toUpperCase();
      (e.target as HTMLInputElement).blur();
    }
  }
</script>

<div class="flex items-center gap-2.5 group">
  <label class="relative size-7 rounded-md overflow-hidden border border-base-300 cursor-pointer shadow-sm hover:shadow transition-shadow flex-shrink-0">
    <input
      type="color"
      value={hexValue()}
      onchange={handleColorPickerChange}
      class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
    />
    <div class="absolute inset-0" style="background-color: {hexValue()}"></div>
  </label>
  <div class="flex-1 min-w-0">
    <span class="text-xs font-medium text-base-content/80 block truncate mb-0.5">{label}</span>
    <input
      type="text"
      bind:value={hexInput}
      oninput={handleHexInput}
      onfocus={() => isEditing = true}
      onblur={handleHexBlur}
      onkeydown={handleHexKeydown}
      class="w-full text-[11px] font-mono bg-base-200 border border-base-300 rounded px-1.5 py-0.5 text-base-content/70 focus:outline-none focus:border-primary focus:text-base-content transition-colors"
      maxlength="7"
      spellcheck="false"
    />
  </div>
</div>

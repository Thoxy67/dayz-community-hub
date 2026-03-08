<script lang="ts">
  import { parseOklch, formatOklch, oklchToHex, hexToOklch } from '$lib/utils/oklch';

  interface Props {
    label: string;
    value: string; // CSS color value (OKLCH, hex, etc.)
    onChange: (value: string) => void;
  }

  let { label, value, onChange }: Props = $props();

  // Convert value to hex for the color picker preview
  const hexValue = $derived(() => {
    // If already hex, use directly (expand shorthand if needed)
    if (value.startsWith('#')) {
      if (value.length === 4) {
        return '#' + value[1] + value[1] + value[2] + value[2] + value[3] + value[3];
      }
      return value;
    }
    const parsed = parseOklch(value);
    if (!parsed) return '#808080';
    return oklchToHex(parsed);
  });

  // Local state for the raw input field
  let rawInput = $state('');
  let isEditing = $state(false);

  // Sync raw input when not editing
  $effect(() => {
    if (!isEditing) {
      rawInput = value;
    }
  });

  function handleColorPickerChange(e: Event) {
    const hex = (e.target as HTMLInputElement).value;
    const oklch = hexToOklch(hex);
    onChange(formatOklch(oklch));
  }

  function handleRawInput(e: Event) {
    rawInput = (e.target as HTMLInputElement).value;
    // Real-time update: pass raw value directly
    onChange(rawInput);
  }

  function handleBlur() {
    isEditing = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === 'Escape') {
      (e.target as HTMLInputElement).blur();
    }
  }
</script>

<div class="flex items-center gap-2.5 group">
  <label
    class="relative size-7 rounded-full overflow-hidden border border-base-300 cursor-pointer shadow-sm hover:shadow transition-shadow flex-shrink-0"
  >
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
      bind:value={rawInput}
      oninput={handleRawInput}
      onfocus={() => (isEditing = true)}
      onblur={handleBlur}
      onkeydown={handleKeydown}
      class="w-full text-[11px] font-mono bg-base-200 border border-base-300 rounded px-1.5 py-0.5 text-base-content/70 focus:outline-none focus:border-primary focus:text-base-content transition-colors"
      spellcheck="false"
    />
  </div>
</div>

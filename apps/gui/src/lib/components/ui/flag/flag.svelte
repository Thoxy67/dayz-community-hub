<script lang="ts" module>
  // The flag set is half a megabyte of SVG, so it is loaded the first time a
  // flag is drawn and shared after that. Emoji flags are not an option: Windows
  // draws them as two letters.
  type IconSet = { icons: Record<string, { body: string }>; width?: number; height?: number };
  let set: Promise<IconSet> | null = null;
  function load() {
    return (set ??= import("@iconify-json/circle-flags/icons.json").then(
      (m) => (m.default ?? m) as unknown as IconSet,
    ));
  }
</script>

<script lang="ts">
  import { cn } from "$lib/cx";

  /** A country's flag, in a circle, from its ISO 3166-1 alpha-2 code. */
  let {
    code,
    title = "",
    class: klass = "",
  }: { code: string; title?: string; class?: string } = $props();

  let body = $state<string | null>(null);
  $effect(() => {
    const c = code.toLowerCase();
    load().then((s) => (body = s.icons[c]?.body ?? null));
  });
</script>

{#if body}
  <svg
    viewBox="0 0 512 512"
    class={cn("size-icon shrink-0", klass)}
    role="img"
    aria-label={title || code}
  >
    {#if title}<title>{title}</title>{/if}
    {@html body}
  </svg>
{:else}
  <span
    class={cn(
      "inline-grid size-icon shrink-0 place-items-center rounded-full bg-raised font-mono text-3xs text-fg-faint",
      klass,
    )}
    title={title || code}>{code.toUpperCase()}</span
  >
{/if}

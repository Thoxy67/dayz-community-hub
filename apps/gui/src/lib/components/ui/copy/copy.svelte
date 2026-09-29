<script lang="ts">
  import type { Snippet } from "svelte";
  import Copy from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";
  import { dict } from "$lib/i18n";
  import { copyText } from "$lib/ipc/native";
  import { cn } from "$lib/cx";

  /**
   * Something the player may want to paste elsewhere, such as an address: the
   * text itself is the button, a copy glyph shows on hover and turns into a
   * tick for a moment once it is on the clipboard.
   */
  let {
    text,
    title = "",
    class: klass = "",
    children,
  }: { text: string; title?: string; class?: string; children?: Snippet } = $props();

  const c = dict("common");
  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(e: MouseEvent) {
    e.stopPropagation();
    await copyText(text);
    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), 1400);
  }
</script>

<button
  type="button"
  class={cn(
    "group/copy inline-flex min-w-0 items-center gap-1 rounded-xs font-mono text-2xs transition-colors",
    copied ? "text-ok" : "text-fg-faint hover:text-fg-muted",
    klass,
  )}
  title={copied ? String($c.copied.value) : title || String($c.copy.value)}
  onclick={copy}
>
  <span class="truncate">{#if children}{@render children()}{:else}{text}{/if}</span>
  {#if copied}
    <Check class="size-3 shrink-0" />
  {:else}
    <Copy class="size-3 shrink-0 opacity-0 transition-opacity group-hover/copy:opacity-100" />
  {/if}
</button>

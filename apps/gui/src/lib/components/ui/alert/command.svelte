<script lang="ts">
  import Copy from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";

  /**
   * A shell command to run by hand, with a way to take it.
   *
   * For when the app cannot do the thing itself, or somebody would rather see
   * what is run as root before it is. Selectable as well as copyable, because
   * WebKitGTK often has no clipboard API and then the button can only select.
   */
  let { command, copyLabel }: { command: string; copyLabel: string } = $props();
  // It takes the room on offer and gives it up first: beside a button it is
  // the one that shrinks, down to a width that still shows what it is.

  let code: HTMLElement | undefined = $state();
  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(command);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // No clipboard: leave it selected, so Ctrl+C is the one thing left to do.
      if (!code) return;
      const range = document.createRange();
      range.selectNodeContents(code);
      const selection = getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  }
</script>

<span
  class="flex h-control-sm max-w-full min-w-[min(12rem,100%)] flex-[1_1_12rem] items-stretch overflow-hidden rounded-sm border border-border bg-bg"
  title={command}
>
  <code
    bind:this={code}
    class="min-w-0 flex-1 truncate px-1.5 font-mono text-2xs leading-[calc(var(--spacing-control-sm)-2px)] text-fg-muted select-text"
  >
    {command}
  </code>
  <button
    type="button"
    aria-label={copyLabel}
    title={copyLabel}
    class="grid w-control-sm shrink-0 place-items-center border-l border-border text-fg-faint hover:bg-raised hover:text-fg"
    onclick={() => void copy()}
  >
    {#if copied}<Check class="size-icon-sm text-ok" />{:else}<Copy class="size-icon-sm" />{/if}
  </button>
</span>

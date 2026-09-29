<script lang="ts">
  import Terminal from "~icons/lucide/terminal";
  import Copy from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";
  import { copyText } from "$lib/ipc/native";
  import { cn } from "$lib/cx";
  import SectionCard from "./SectionCard.svelte";

  /**
   * A command line as it will run, coloured (flag, `=`, value) and wrapped
   * between flags, never inside one, with a copy button.
   */
  let {
    title,
    program = "",
    flags,
    empty,
    hint = "",
    copyLabel,
    class: klass = "",
  }: {
    title: string;
    /** Shown first, dimmed; not part of what is copied. */
    program?: string;
    /** `-flag` or `-flag=value`, in order. */
    flags: readonly string[];
    /** Said when there are no flags. */
    empty: string;
    hint?: string;
    copyLabel: string;
    class?: string;
  } = $props();

  let copied = $state(false);
  async function copy() {
    await copyText(flags.join(" "));
    copied = true;
    setTimeout(() => (copied = false), 1400);
  }
</script>

<SectionCard {title} icon={Terminal} class={klass}>
  {#snippet actions()}
    <button
      class={cn(
        "grid size-control-sm place-items-center rounded-sm hover:bg-raised disabled:opacity-40",
        copied ? "text-ok" : "text-fg-faint hover:text-fg",
      )}
      aria-label={copyLabel}
      title={copyLabel}
      disabled={flags.length === 0}
      onclick={copy}
    >
      {#if copied}<Check class="size-icon-sm" />{:else}<Copy class="size-icon-sm" />{/if}
    </button>
  {/snippet}
  <div class="p-2">
    <pre
      class="m-0 max-h-72 overflow-y-auto rounded-sm bg-plot px-2 py-1.5 font-mono text-2xs leading-relaxed whitespace-pre-wrap text-fg-muted"
      data-selectable>{#if program}<span class="text-fg-faint">{program}</span>{/if}{#each flags as f, i (i)}{program || i > 0 ? " " : ""}<span class="whitespace-nowrap"><span class="text-accent">{f.split("=")[0]}</span>{#if f.includes("=")}<span class="text-fg-faint">=</span><span class="text-ok">{f.slice(f.indexOf("=") + 1)}</span>{/if}</span>{/each}{#if flags.length === 0}{program ? "\n" : ""}<span class="text-fg-faint italic">{empty}</span>{/if}</pre>
    {#if hint}<p class="m-0 mt-1.5 text-3xs leading-snug text-fg-faint">{hint}</p>{/if}
  </div>
</SectionCard>

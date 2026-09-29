<script lang="ts">
  import { tick } from "svelte";
  import { dict } from "$lib/i18n";
  import Terminal from "~icons/lucide/terminal";
  import ArrowDownToLine from "~icons/lucide/arrow-down-to-line";
  import CopyIcon from "~icons/lucide/copy";
  import Check from "~icons/lucide/check";
  import { Input } from "$lib/components/ui/input";
  import { Tag } from "$lib/components/ui/tag";
  import { Button } from "$lib/components/ui/button";
  import { copyText } from "$lib/ipc/native";
  import { cn } from "$lib/cx";
  import { LINE_CLASS, classifyLine, clock, type LineKind } from "./log";

  /**
   * A tool's output, live: each line with the time it arrived, coloured by
   * what it says, filterable, copyable. It follows the end while the reader
   * is at the end; scrolling up stops it, and a button brings it back.
   */
  let {
    lines,
    times,
    title = "log",
    live = false,
    classify = classifyLine,
    class: klass = "",
  }: {
    lines: readonly string[];
    /** When each line arrived, in ms since the start (parallel to `lines`). */
    times?: readonly number[];
    title?: string;
    /** Still being written: shows a LIVE mark and a cursor. */
    live?: boolean;
    classify?: (line: string) => LineKind;
    class?: string;
  } = $props();

  const c = dict("common");
  let box = $state<HTMLDivElement | null>(null);
  let follow = $state(true);
  let filter = $state("");
  let copied = $state(false);

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const out: { i: number; text: string; at: number | null; cls: string }[] = [];
    lines.forEach((text, i) => {
      if (q && !text.toLowerCase().includes(q)) return;
      out.push({ i, text, at: times?.[i] ?? null, cls: LINE_CLASS[classify(text)] });
    });
    return out;
  });

  $effect(() => {
    void shown.length;
    void lines[lines.length - 1];
    if (follow && box) void tick().then(() => box && (box.scrollTop = box.scrollHeight));
  });

  function onscroll() {
    if (box) follow = box.scrollTop + box.clientHeight >= box.scrollHeight - 24;
  }
  function jump() {
    follow = true;
    if (box) box.scrollTop = box.scrollHeight;
  }
  async function copyAll() {
    await copyText(lines.map((l, i) => (times ? `[+${clock(times[i] ?? 0)}] ${l}` : l)).join("\n"));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<section class={cn("relative flex min-h-0 flex-col bg-plot", klass)}>
  <div class="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5">
    <Terminal class="size-icon-sm text-fg-faint" />
    <span class="font-mono text-2xs tracking-widest text-fg-faint uppercase">{title}</span>
    <Tag><span class="num font-mono">{lines.length}</span></Tag>
    {#if live}
      <span class="flex items-center gap-1 font-mono text-3xs text-ok">
        <span class="size-1.5 animate-pulse rounded-full bg-ok"></span>LIVE
      </span>
    {/if}
    <Input type="search" size="xs" class="ml-auto w-48" placeholder="grep…" bind:value={filter} />
    <Button variant="ghost" size="xs" onclick={copyAll}>
      {#if copied}<Check class="size-3 text-ok" />{$c.copied.value}{:else}<CopyIcon class="size-3" />{$c.copy.value}{/if}
    </Button>
  </div>
  <div
    bind:this={box}
    {onscroll}
    class="min-h-0 flex-1 overflow-y-auto py-2 font-mono text-[11px] leading-[1.55]"
    data-selectable
  >
    {#each shown as l (l.i)}
      <div class="flex gap-3 px-3 hover:bg-raised/30">
        {#if l.at !== null}
          <span class="w-14 shrink-0 text-right text-fg-faint/70 select-none">+{clock(l.at)}</span>
        {/if}
        <span class={cn("min-w-0 flex-1 break-all whitespace-pre-wrap", l.cls)}>{l.text}</span>
      </div>
    {/each}
    {#if live && !filter}
      <div class={cn("px-3", times && "pl-[5.25rem]")}>
        <span class="inline-block h-3 w-1.5 animate-pulse bg-accent align-middle"></span>
      </div>
    {/if}
  </div>
  {#if !follow}
    <button
      class="absolute right-4 bottom-4 flex items-center gap-1.5 rounded-full border border-accent/50 bg-accent px-3 py-1 text-2xs font-semibold text-accent-fg shadow-pop"
      onclick={jump}
    >
      <ArrowDownToLine class="size-3" />LIVE
    </button>
  {/if}
</section>

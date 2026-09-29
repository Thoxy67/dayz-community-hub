<script lang="ts">
  import { dict } from "$lib/i18n";
  import SlidersHorizontal from "~icons/lucide/sliders-horizontal";
  import Sparkles from "~icons/lucide/sparkles";
  import FolderOpen from "~icons/lucide/folder-open";
  import X from "~icons/lucide/x";
  import Check from "~icons/lucide/check";
  import { Input } from "$lib/components/ui/input";
  import { MiniSwitch } from "$lib/components/ui/switch";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import type { LaunchOptionDto } from "$lib/ipc/types";
  import { pickFile } from "$lib/ipc/native";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import { META, flagText, wordOf } from "./catalog";

  /**
   * One launch option: what it is, the flag it becomes, its value when it
   * takes one, and whether it is on. A value is written when the field is
   * left or Enter is pressed, and writing one turns the option on.
   */
  let { opt, tone, recommended }: { opt: LaunchOptionDto; tone: string; recommended?: string } =
    $props();

  const o = dict("options");
  const meta = $derived(META[opt.key]);
  const Icon = $derived(meta?.icon ?? SlidersHorizontal);
  const kind = $derived(meta?.value ?? (opt.value !== null ? "text" : "none"));

  // What is typed, reset whenever the saved value changes.
  let draft = $derived(opt.value ?? "");

  function commit() {
    const v = draft.trim();
    if (v === (opt.value ?? "")) return;
    void profile.setOptionValue(opt.key, v === "" ? null : v);
  }

  async function browse() {
    const path = await pickFile($o.browseFolder.value, { directory: true });
    if (path) {
      draft = path;
      commit();
    }
  }

  // What the exThreads bitmask means, for the one option whose value is a code.
  const threadsMeaning = $derived.by(() => {
    if (kind !== "threads") return "";
    const n = parseInt(draft, 10);
    if (Number.isNaN(n)) return "";
    if (n === 0) return $o.threadsNone.value;
    if (n === 7) return $o.threadsAll.value;
    if (n === 1) return $o.threadsFile.value;
    return `0b${n.toString(2).padStart(3, "0")}`;
  });

  const label = $derived(meta ? wordOf($o, meta.label) : opt.key);
  const desc = $derived(meta ? wordOf($o, meta.desc) : opt.description);
  const matches = $derived(recommended !== undefined && recommended === (opt.value ?? ""));
</script>

<div
  class={cn(
    "group grid grid-cols-[1.25rem_minmax(0,1fr)_auto_auto] items-center gap-x-3 px-3 py-2 transition-colors hover:bg-raised/40",
    !opt.enabled && "opacity-70",
  )}
>
  <Icon class={cn("size-icon", opt.enabled ? tone : "text-fg-faint")} />

  <div class="min-w-0">
    <div class="truncate text-xs font-medium text-fg">{label}</div>
    <p class="m-0 mt-0.5 flex min-w-0 items-center gap-1.5 text-2xs text-fg-faint" title={desc}>
      <code
        class={cn(
          "shrink-0 rounded-xs px-1 font-mono text-3xs leading-4",
          opt.enabled ? "bg-accent/12 text-accent" : "bg-raised text-fg-faint",
        )}>{flagText(opt.key, null)}</code
      >
      <span class="truncate">{desc}{#if threadsMeaning}<span class="text-fg-muted"> · {threadsMeaning}</span>{/if}</span>
    </p>
  </div>

  <div class="flex items-center gap-1">
    {#if kind !== "none"}
      {#if recommended !== undefined}
        {#if matches}
          <Tooltip text={$o.matchesRecommended.value}>
            <Check class="size-icon-sm text-ok" />
          </Tooltip>
        {:else}
          <Tooltip text={$o.useRecommended.value}>
            <button
              class="flex h-control-sm items-center gap-1 rounded-sm border border-accent/30 px-1.5 font-mono text-3xs text-accent hover:bg-accent/10"
              onclick={() => {
                draft = recommended;
                commit();
              }}
            >
              <Sparkles class="size-3" />{recommended}
            </button>
          </Tooltip>
        {/if}
      {/if}
      <span class="relative flex items-center">
        <Input
          size="xs"
          type={kind === "mb" || kind === "count" || kind === "threads" ? "number" : "text"}
          class={cn(kind === "folder" || kind === "text" ? "w-40" : "w-24", "font-mono")}
          placeholder={$o.valuePlaceholder.value}
          aria-label={label}
          bind:value={draft}
          onblur={commit}
          onkeydown={(e: KeyboardEvent) => {
            if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
            if (e.key === "Escape") draft = opt.value ?? "";
          }}
        />
        {#if kind === "mb"}<span class="pointer-events-none absolute right-5 text-3xs text-fg-faint"
            >{$o.unitMb.value}</span
          >{/if}
      </span>
      {#if kind === "folder"}
        <Tooltip text={$o.browseFolder.value}>
          <button
            class="grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            aria-label={$o.browseFolder.value}
            onclick={browse}><FolderOpen class="size-icon-sm" /></button
          >
        </Tooltip>
      {/if}
      {#if opt.value}
        <Tooltip text={$o.clearValue.value}>
          <button
            class="grid size-control-sm place-items-center rounded-sm text-fg-faint opacity-0 group-hover:opacity-100 hover:bg-raised hover:text-err focus-visible:opacity-100"
            aria-label={$o.clearValue.value}
            onclick={() => {
              draft = "";
              commit();
            }}><X class="size-icon-sm" /></button
          >
        </Tooltip>
      {/if}
    {/if}
  </div>

  <MiniSwitch
    bind:checked={() => opt.enabled, () => void profile.toggleOption(opt.key)}
    aria-label={label}
  />
</div>

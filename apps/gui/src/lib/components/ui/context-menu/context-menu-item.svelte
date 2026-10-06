<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { ContextMenu } from "bits-ui";
  import { Kbd } from "$lib/components/ui/kbd";
  import { cn } from "$lib/cx";

  let {
    icon,
    kbd = "",
    disabled = false,
    tone = "neutral",
    onselect,
    class: klass = "",
    children,
  }: {
    icon?: Component<{ class?: string }>;
    kbd?: string;
    disabled?: boolean;
    tone?: "neutral" | "danger";
    onselect?: () => void;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<ContextMenu.Item
  {disabled}
  onSelect={() => onselect?.()}
  class={cn(
    "flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs outline-none select-none",
    "data-[disabled]:cursor-not-allowed data-[disabled]:opacity-40 data-[highlighted]:bg-raised",
    tone === "danger" ? "text-err" : "text-fg-muted data-[highlighted]:text-fg",
    klass,
  )}
>
  {#if icon}
    {@const Icon = icon}
    <Icon class="size-icon shrink-0" />
  {/if}
  <span class="min-w-0 flex-1 truncate">{@render children()}</span>
  {#if kbd}<Kbd>{kbd}</Kbd>{/if}
</ContextMenu.Item>

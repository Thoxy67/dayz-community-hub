<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { DropdownMenu } from "bits-ui";
  import Check from "~icons/lucide/check";
  import { Kbd } from "$lib/components/ui/kbd";
  import { cn } from "$lib/cx";

  // A thing in a menu that is on or off. Choosing it leaves the menu open,
  // because whoever is switching layers on and off is usually switching
  // several, and a menu that closes after each is a menu opened five times.
  let {
    checked = $bindable(false),
    icon,
    kbd = "",
    disabled = false,
    onchange,
    class: klass = "",
    // Under another name: bits-ui's item takes a `children` snippet of its own
    // below, and inside it that name is bits-ui's.
    children: content,
  }: {
    checked?: boolean;
    icon?: Component<{ class?: string }>;
    kbd?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<DropdownMenu.CheckboxItem
  bind:checked={
    () => checked,
    (v) => {
      if (onchange) onchange(v);
      else checked = v;
    }
  }
  {disabled}
  closeOnSelect={false}
  class={cn(
    "flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none select-none",
    "data-[disabled]:cursor-not-allowed data-[disabled]:opacity-40",
    "data-[highlighted]:bg-raised data-[highlighted]:text-fg data-[state=checked]:text-fg",
    klass,
  )}
>
  {#snippet children({ checked: on })}
    <span
      class={cn(
        "grid size-3.5 shrink-0 place-items-center rounded-xs border",
        on ? "border-accent bg-accent text-accent-fg" : "border-border-strong bg-bg",
      )}
    >
      {#if on}<Check class="size-3" />{/if}
    </span>
    {#if icon}
      {@const Icon = icon}
      <Icon class="size-icon shrink-0 {on ? 'text-accent' : 'text-fg-faint'}" />
    {/if}
    <span class="min-w-0 flex-1 truncate">{@render content()}</span>
    {#if kbd}<Kbd>{kbd}</Kbd>{/if}
  {/snippet}
</DropdownMenu.CheckboxItem>

<script lang="ts">
  import { Select } from "bits-ui";
  import Check from "~icons/lucide/check";
  import ChevronDown from "~icons/lucide/chevron-down";
  import { cn } from "$lib/cx";

  type Option = { value: string; label: string; group?: string };

  // One value out of a list: bits-ui's Select.
  //
  // The engine's own `<select>` opened a popup the platform drew, which on
  // WebKitGTK is a GTK menu in GTK's colours at GTK's size, beside a window
  // that is neither. This one is made of elements, so it wears the theme, and
  // typing still jumps to a match and the arrows still move (bits-ui's).
  //
  // No `name` is passed, so no hidden input is rendered: see `switch`.
  let {
    value = $bindable(),
    options,
    placeholder = "",
    disabled = false,
    class: klass = "",
    "aria-label": label,
  }: {
    value: string | null;
    options: Option[];
    placeholder?: string;
    disabled?: boolean;
    class?: string;
    "aria-label": string;
  } = $props();

  // Grouped in first-seen order; options with no group are listed flat.
  const groups = $derived.by(() => {
    const m = new Map<string, Option[]>();
    for (const o of options) {
      const k = o.group ?? "";
      const list = m.get(k);
      if (list) list.push(o);
      else m.set(k, [o]);
    }
    return [...m];
  });
  const shown = $derived(options.find((o) => o.value === value)?.label ?? placeholder);
</script>

{#snippet item(o: Option)}
  <Select.Item
    value={o.value}
    label={o.label}
    class="flex h-row cursor-pointer items-center gap-1.5 rounded-xs px-1.5 text-xs text-fg-muted outline-none
           select-none data-[highlighted]:bg-raised data-[highlighted]:text-fg data-[selected]:text-accent"
  >
    {#snippet children({ selected })}
      <span class="min-w-0 flex-1 truncate">{o.label}</span>
      {#if selected}<Check class="size-icon-sm shrink-0" />{/if}
    {/snippet}
  </Select.Item>
{/snippet}

<Select.Root
  type="single"
  bind:value={() => value ?? "", (v) => (value = v === "" ? null : v)}
  {disabled}
  items={options}
>
  <Select.Trigger
    aria-label={label}
    class={cn(
      "inline-flex h-control min-w-0 items-center gap-1 rounded-sm border border-border bg-bg px-1.5 text-xs text-fg",
      "hover:border-border-strong focus-visible:border-accent/60 focus-visible:outline-none",
      "data-[state=open]:border-accent/60 disabled:cursor-not-allowed disabled:opacity-40",
      klass,
    )}
  >
    <span class={cn("min-w-0 flex-1 truncate text-left", value === null && "text-fg-faint")}
      >{shown}</span
    >
    <ChevronDown class="size-icon-sm shrink-0 text-fg-faint" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content
      sideOffset={4}
      collisionPadding={4}
      class="z-popover max-h-[min(20rem,var(--bits-select-content-available-height,20rem))] w-[var(--bits-select-anchor-width)]
             min-w-32 overflow-hidden rounded-md border border-border bg-overlay shadow-pop outline-none"
    >
      <Select.Viewport class="p-1">
        {#if placeholder}
          <Select.Item
            value=""
            label={placeholder}
            class="flex h-row cursor-pointer items-center rounded-xs px-1.5 text-xs text-fg-faint outline-none
                   select-none data-[highlighted]:bg-raised data-[highlighted]:text-fg"
          >
            {placeholder}
          </Select.Item>
        {/if}
        {#each groups as [group, items] (group)}
          {#if group}
            <Select.Group>
              <Select.GroupHeading
                class="px-1.5 pt-1.5 pb-0.5 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase"
              >
                {group}
              </Select.GroupHeading>
              {#each items as o (o.value)}{@render item(o)}{/each}
            </Select.Group>
          {:else}
            {#each items as o (o.value)}{@render item(o)}{/each}
          {/if}
        {/each}
      </Select.Viewport>
    </Select.Content>
  </Select.Portal>
</Select.Root>

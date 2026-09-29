<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { useIntlayer } from "svelte-intlayer";
  import Palette from "~icons/lucide/palette";
  import Monitor from "~icons/lucide/monitor";
  import Pencil from "~icons/lucide/pencil";
  import Check from "~icons/lucide/check";
  import { DropdownMenuContent, DropdownMenuLabel, DropdownMenuSeparator } from "$lib/components/ui/dropdown-menu";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { theme, PRESETS } from "$lib/theme/theme.svelte";
  import type { Preset } from "$lib/theme/presets";
  import { app } from "$lib/stores/app.svelte";

  const t = useIntlayer("theme");

  // Each preset is shown by its own colours: ground, panel, accent and the
  // two status colours, so choosing is looking, not reading names.
  const dark = PRESETS.filter((p) => p.scheme === "dark");
  const light = PRESETS.filter((p) => p.scheme === "light");

  function label(id: string): string {
    const key = `preset${id.replace(/(^|_)(\w)/g, (_, __, c: string) => c.toUpperCase())}` as keyof typeof $t;
    const node = $t[key] as { value?: string } | undefined;
    return node?.value ?? id.replace(/_/g, " ");
  }
</script>

{#snippet swatch(p: Preset)}
  <span class="flex h-4 overflow-hidden rounded-xs border border-border" aria-hidden="true">
    {#each [p.tokens.bg, p.tokens.panel, p.tokens.accent, p.tokens.ok, p.tokens.err] as c, i (i)}
      <span class="w-2" style="background: {c}"></span>
    {/each}
  </span>
{/snippet}

{#snippet item(p: Preset)}
  <Menu.Item
    onSelect={() => theme.use(p.id)}
    class="flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none
           data-[highlighted]:bg-raised data-[highlighted]:text-fg"
  >
    {@render swatch(p)}
    <span class="min-w-0 flex-1 truncate capitalize">{label(p.id)}</span>
    {#if theme.selected === p.id}<Check class="size-icon-sm text-accent" />{/if}
  </Menu.Item>
{/snippet}

<Menu.Root>
  <Tooltip text={$t.change.value} side="bottom">
    <Menu.Trigger
      class="grid h-titlebar w-9 place-items-center text-fg-muted hover:bg-raised hover:text-fg"
      aria-label={$t.change.value}
    >
      <Palette class="size-icon" />
    </Menu.Trigger>
  </Tooltip>
  <DropdownMenuContent class="max-h-[70vh] w-56">
    <Menu.Item
      onSelect={() => theme.use(null)}
      class="flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none
             data-[highlighted]:bg-raised data-[highlighted]:text-fg"
    >
      <Monitor class="size-icon shrink-0" />
      <span class="flex-1">{$t.default.value}</span>
      {#if theme.selected === null}<Check class="size-icon-sm text-accent" />{/if}
    </Menu.Item>
    <DropdownMenuSeparator />
    <Menu.Group>
      <DropdownMenuLabel>{$t.dark.value}</DropdownMenuLabel>
      {#each dark as p (p.id)}{@render item(p)}{/each}
    </Menu.Group>
    <DropdownMenuSeparator />
    <Menu.Group>
      <DropdownMenuLabel>{$t.light.value}</DropdownMenuLabel>
      {#each light as p (p.id)}{@render item(p)}{/each}
    </Menu.Group>
    <DropdownMenuSeparator />
    <Menu.Item
      onSelect={() => app.go("settings", "appearance")}
      class="flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none
             data-[highlighted]:bg-raised data-[highlighted]:text-fg"
    >
      <Pencil class="size-icon shrink-0" />
      <span class="flex-1">{$t.editCustom.value}</span>
      {#if theme.selected === "custom"}<Check class="size-icon-sm text-accent" />{/if}
    </Menu.Item>
  </DropdownMenuContent>
</Menu.Root>

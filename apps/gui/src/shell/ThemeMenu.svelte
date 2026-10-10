<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Palette from "~icons/lucide/palette";
  import Monitor from "~icons/lucide/monitor";
  import Moon from "~icons/lucide/moon";
  import Sun from "~icons/lucide/sun";
  import Pencil from "~icons/lucide/pencil";
  import Check from "~icons/lucide/check";
  import ChevronLeft from "~icons/lucide/chevron-left";
  import {
    DropdownMenuContent,
    DropdownMenuLabel,
    DropdownMenuSeparator,
  } from "$lib/components/ui/dropdown-menu";
  import DropdownMenuSubContent from "$lib/components/ui/dropdown-menu/dropdown-menu-sub-content.svelte";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { theme, PRESETS } from "$lib/theme/theme.svelte";
  import { presetName } from "$lib/theme/names";
  import type { Scheme, ThemeTokens } from "$lib/theme/presets";
  import { app } from "$lib/stores/app.svelte";

  const t = dict("theme");

  const dark = PRESETS.filter((p) => p.scheme === "dark");
  const light = PRESETS.filter((p) => p.scheme === "light");

  // Hover preview: resting on an entry wears it for as long as the pointer
  // stays; leaving, choosing or closing ends it. Never stored (theme.preview).
  let timer: ReturnType<typeof setTimeout> | undefined;
  function hover(id: string | null) {
    clearTimeout(timer);
    if (id === null) theme.preview(null);
    else timer = setTimeout(() => theme.preview(id), 150);
  }
  function choose(id: string | null) {
    clearTimeout(timer);
    theme.use(id);
  }

  const row =
    "flex h-control cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none select-none data-[highlighted]:bg-raised data-[highlighted]:text-fg";
</script>

{#snippet swatch(tokens: ThemeTokens)}
  <span
    class="flex h-4 shrink-0 overflow-hidden rounded-xs border border-border"
    aria-hidden="true"
  >
    {#each [tokens.bg, tokens.panel, tokens.accent, tokens.ok, tokens.err] as c, i (i)}
      <span class="w-2" style="background: {c}"></span>
    {/each}
  </span>
{/snippet}

{#snippet entry(id: string, name: string, tokens: ThemeTokens, on: boolean)}
  <Menu.Item
    onSelect={() => choose(id)}
    onpointerenter={() => hover(id)}
    onpointerleave={() => hover(null)}
    class={row}
  >
    {@render swatch(tokens)}
    <span class="min-w-0 flex-1 truncate">{name}</span>
    {#if on}<Check class="size-icon-sm shrink-0 text-accent" />{/if}
  </Menu.Item>
{/snippet}

<!-- Which preset "follow the system" wears for one OS scheme. -->
{#snippet pairChoice(scheme: Scheme)}
  {@const Icon = scheme === "dark" ? Moon : Sun}
  <Menu.Sub>
    <Menu.SubTrigger class="{row} pl-7">
      <Icon class="size-icon-sm shrink-0" />
      <span class="shrink-0">{scheme === "dark" ? $t.whenDark.value : $t.whenLight.value}</span>
      <span class="min-w-0 flex-1 truncate text-right text-fg-faint"
        >{presetName(theme.pair[scheme], $t)}</span
      >
      <ChevronLeft class="size-icon-sm shrink-0 text-fg-faint" />
    </Menu.SubTrigger>
    <DropdownMenuSubContent class="w-52">
      {#each scheme === "dark" ? dark : light as p (p.id)}
        <Menu.Item onSelect={() => theme.setPair(scheme, p.id)} class={row}>
          {@render swatch(p.tokens)}
          <span class="min-w-0 flex-1 truncate">{presetName(p.id, $t)}</span>
          {#if theme.pair[scheme] === p.id}<Check class="size-icon-sm shrink-0 text-accent" />{/if}
        </Menu.Item>
      {/each}
    </DropdownMenuSubContent>
  </Menu.Sub>
{/snippet}

<Menu.Root
  onOpenChange={(open) => {
    if (!open) hover(null);
  }}
>
  <Tooltip text={$t.change.value} side="bottom">
    <Menu.Trigger
      class="grid h-titlebar w-9 place-items-center text-fg-muted transition-colors duration-fast hover:bg-raised hover:text-fg"
      aria-label={$t.change.value}
    >
      <Palette class="size-icon" />
    </Menu.Trigger>
  </Tooltip>
  <DropdownMenuContent class="max-h-[calc(100vh-var(--spacing-titlebar)-16px)] w-64">
    <Menu.Item onSelect={() => choose(null)} class={row}>
      <Monitor class="size-icon shrink-0" />
      <span class="flex-1">{$t.followSystem.value}</span>
      {#if theme.selected === null}<Check class="size-icon-sm text-accent" />{/if}
    </Menu.Item>
    {@render pairChoice("dark")}
    {@render pairChoice("light")}
    <DropdownMenuSeparator />
    <Menu.Group>
      <DropdownMenuLabel>{$t.dark.value}</DropdownMenuLabel>
      {#each dark as p (p.id)}{@render entry(
          p.id,
          presetName(p.id, $t),
          p.tokens,
          theme.worn === p.id,
        )}{/each}
    </Menu.Group>
    <DropdownMenuSeparator />
    <Menu.Group>
      <DropdownMenuLabel>{$t.light.value}</DropdownMenuLabel>
      {#each light as p (p.id)}{@render entry(
          p.id,
          presetName(p.id, $t),
          p.tokens,
          theme.worn === p.id,
        )}{/each}
    </Menu.Group>
    {#if theme.customs.length}
      <DropdownMenuSeparator />
      <Menu.Group>
        <DropdownMenuLabel>{$t.custom.value}</DropdownMenuLabel>
        {#each theme.customs as c (c.id)}{@render entry(
            c.id,
            c.name,
            c.tokens,
            theme.worn === c.id,
          )}{/each}
      </Menu.Group>
    {/if}
    <DropdownMenuSeparator />
    <Menu.Item onSelect={() => app.go("settings", "appearance")} class={row}>
      <Pencil class="size-icon shrink-0" />
      <span class="flex-1">{$t.editCustom.value}</span>
    </Menu.Item>
  </DropdownMenuContent>
</Menu.Root>

<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { useIntlayer } from "svelte-intlayer";
  import FlagGb from "~icons/circle-flags/gb";
  import FlagFr from "~icons/circle-flags/fr";
  import FlagDe from "~icons/circle-flags/de";
  import FlagEs from "~icons/circle-flags/es";
  import FlagRu from "~icons/circle-flags/ru";
  import Check from "~icons/lucide/check";
  import { DropdownMenuContent } from "$lib/components/ui/dropdown-menu";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { LOCALES, LOCALE_LABELS, getLocale, setLocale, type Locale } from "$lib/i18n";

  const FLAGS = { en: FlagGb, fr: FlagFr, de: FlagDe, es: FlagEs, ru: FlagRu } as const;
  const s = useIntlayer("shell");
  let current = $state<Locale>(getLocale());
  const Flag = $derived(FLAGS[current]);
</script>

<Menu.Root>
  <Tooltip text={$s.langChange.value} side="bottom">
    <Menu.Trigger
      class="grid h-titlebar w-9 place-items-center text-fg-muted hover:bg-raised hover:text-fg"
      aria-label={$s.langChange.value}
    >
      <Flag class="size-4" />
    </Menu.Trigger>
  </Tooltip>
  <DropdownMenuContent class="min-w-40">
    {#each LOCALES as l (l)}
      {@const F = FLAGS[l]}
      <Menu.Item
        onSelect={() => {
          current = l;
          setLocale(l);
        }}
        class="flex h-row cursor-pointer items-center gap-2 rounded-xs px-1.5 text-xs text-fg-muted outline-none
               data-[highlighted]:bg-raised data-[highlighted]:text-fg"
      >
        <F class="size-4 shrink-0" />
        <span class="flex-1">{LOCALE_LABELS[l]}</span>
        <span class="font-mono text-3xs text-fg-faint uppercase">{l}</span>
        {#if l === current}<Check class="size-icon-sm text-accent" />{/if}
      </Menu.Item>
    {/each}
  </DropdownMenuContent>
</Menu.Root>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import Languages from "~icons/lucide/languages";
  import Check from "~icons/lucide/check";
  import FlagGb from "~icons/circle-flags/gb";
  import FlagFr from "~icons/circle-flags/fr";
  import FlagDe from "~icons/circle-flags/de";
  import FlagEs from "~icons/circle-flags/es";
  import FlagRu from "~icons/circle-flags/ru";
  import { cn } from "$lib/cx";
  import { LOCALES, LOCALE_LABELS, getLocale, setLocale, type Locale } from "$lib/i18n";
  import { SettingsSection as Section } from "$lib/components/app";

  const s = dict("settings");
  const FLAGS = { en: FlagGb, fr: FlagFr, de: FlagDe, es: FlagEs, ru: FlagRu } as const;
  let current = $state<Locale>(getLocale());
</script>

<Section id="language" title={$s.sectionLanguage.value} description={$s.languageHint.value} icon={Languages}>
  <div class="grid grid-cols-5 gap-2 p-2.5">
    {#each LOCALES as l (l)}
      {@const F = FLAGS[l]}
      {@const on = current === l}
      <button
        type="button"
        class={cn(
          "flex items-center gap-2 rounded-md border px-2.5 py-2 text-left transition-colors",
          on ? "border-accent bg-accent/8" : "border-border hover:border-border-strong hover:bg-raised/40",
        )}
        aria-pressed={on}
        onclick={() => {
          current = l;
          setLocale(l);
        }}
      >
        <F class="size-5 shrink-0" />
        <span class="min-w-0 flex-1">
          <span class="block truncate text-xs font-medium text-fg">{LOCALE_LABELS[l]}</span>
          <span class="block font-mono text-3xs text-fg-faint uppercase">{l}</span>
        </span>
        {#if on}<Check class="size-icon-sm text-accent" />{/if}
      </button>
    {/each}
  </div>
</Section>

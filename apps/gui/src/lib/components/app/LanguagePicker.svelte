<script lang="ts">
  import FlagGb from "~icons/circle-flags/gb";
  import FlagFr from "~icons/circle-flags/fr";
  import FlagDe from "~icons/circle-flags/de";
  import FlagEs from "~icons/circle-flags/es";
  import FlagRu from "~icons/circle-flags/ru";
  import { LOCALES, LOCALE_LABELS, getLocale, setLocale, type Locale } from "$lib/i18n";
  import { cn } from "$lib/cx";

  /** The five languages as flags with their own names; choosing one switches at once. */
  let { class: klass = "", "aria-label": ariaLabel }: { class?: string; "aria-label": string } = $props();

  const FLAGS = { en: FlagGb, fr: FlagFr, de: FlagDe, es: FlagEs, ru: FlagRu } as const;
  let current = $state<Locale>(getLocale());
</script>

<div class={cn("grid grid-cols-5 gap-1.5", klass)} role="radiogroup" aria-label={ariaLabel}>
  {#each LOCALES as l (l)}
    {@const F = FLAGS[l]}
    {@const on = l === current}
    <button
      role="radio"
      aria-checked={on}
      class={cn(
        "flex flex-col items-center gap-1.5 rounded-md border px-2 py-2.5 text-2xs transition-colors",
        on ? "border-accent bg-accent/10 text-fg shadow-[var(--glow-accent-soft)]" : "border-border text-fg-muted hover:border-border-strong hover:text-fg",
      )}
      onclick={() => {
        current = l;
        setLocale(l);
      }}
    >
      <F class="size-6" />
      <span class="truncate">{LOCALE_LABELS[l]}</span>
    </button>
  {/each}
</div>

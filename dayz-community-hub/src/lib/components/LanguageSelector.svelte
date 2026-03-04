<script lang="ts">
  import { type Locale } from '$lib/paraglide/runtime.js';
  import { localeState } from '$lib/locale.svelte';
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  let dropdownOpen = $state(false);
  const currentLocale = $derived(localeState.locale);

  function selectLocale(locale: Locale) {
    localeState.setLocale(locale);
    dropdownOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      dropdownOpen = false;
    }
  }

  $effect(() => {
    if (dropdownOpen) {
      const handleClickOutside = (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        if (!target.closest('.language-dropdown')) {
          dropdownOpen = false;
        }
      };
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="relative language-dropdown" onkeydown={handleKeydown}>
  <button
    class="inline-flex items-center justify-center gap-1.5 h-9 px-2 text-base-content/50 hover:bg-base-300 hover:text-base-content transition-colors"
    onclick={(e) => {
      e.stopPropagation();
      dropdownOpen = !dropdownOpen;
    }}
    title={m.lang_change()}
  >
    {#if currentLocale === 'de'}
      <Icon icon="emojione:flag-for-germany" class="size-4" />
    {:else if currentLocale === 'fr'}
      <Icon icon="emojione:flag-for-france" class="size-4" />
    {:else if currentLocale === 'ru'}
      <Icon icon="emojione:flag-for-russia" class="size-4" />
    {:else if currentLocale === 'es'}
      <Icon icon="emojione:flag-for-spain" class="size-4" />
    {:else}
      <Icon icon="emojione:flag-for-united-states" class="size-4" />
    {/if}
    <Icon icon="ph:caret-down" class="size-3 opacity-50" />
  </button>

  {#if dropdownOpen}
    <div
      class="absolute right-0 top-full mt-1 w-40 bg-base-200 border border-base-300 rounded-lg shadow-xl z-50 py-1 overflow-hidden"
    >
      <button
        class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {currentLocale ===
        'en'
          ? 'text-primary bg-base-300/50'
          : 'text-base-content/70'}"
        onclick={() => selectLocale('en')}
      >
        <Icon icon="emojione:flag-for-united-states" class="size-5" />
        <span class="flex-1">{m.lang_en()}</span>
        {#if currentLocale === 'en'}
          <Icon icon="ph:check" class="size-4 text-primary" />
        {/if}
      </button>
      <button
        class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {currentLocale ===
        'de'
          ? 'text-primary bg-base-300/50'
          : 'text-base-content/70'}"
        onclick={() => selectLocale('de')}
      >
        <Icon icon="emojione:flag-for-germany" class="size-5" />
        <span class="flex-1">{m.lang_de()}</span>
        {#if currentLocale === 'de'}
          <Icon icon="ph:check" class="size-4 text-primary" />
        {/if}
      </button>
      <button
        class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {currentLocale ===
        'fr'
          ? 'text-primary bg-base-300/50'
          : 'text-base-content/70'}"
        onclick={() => selectLocale('fr')}
      >
        <Icon icon="emojione:flag-for-france" class="size-5" />
        <span class="flex-1">{m.lang_fr()}</span>
        {#if currentLocale === 'fr'}
          <Icon icon="ph:check" class="size-4 text-primary" />
        {/if}
      </button>
      <button
        class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {currentLocale ===
        'ru'
          ? 'text-primary bg-base-300/50'
          : 'text-base-content/70'}"
        onclick={() => selectLocale('ru')}
      >
        <Icon icon="emojione:flag-for-russia" class="size-5" />
        <span class="flex-1">{m.lang_ru()}</span>
        {#if currentLocale === 'ru'}
          <Icon icon="ph:check" class="size-4 text-primary" />
        {/if}
      </button>
      <button
        class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {currentLocale ===
        'es'
          ? 'text-primary bg-base-300/50'
          : 'text-base-content/70'}"
        onclick={() => selectLocale('es')}
      >
        <Icon icon="emojione:flag-for-spain" class="size-5" />
        <span class="flex-1">{m.lang_es()}</span>
        {#if currentLocale === 'es'}
          <Icon icon="ph:check" class="size-4 text-primary" />
        {/if}
      </button>
    </div>
  {/if}
</div>

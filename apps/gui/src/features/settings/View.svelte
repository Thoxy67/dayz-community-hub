<script lang="ts">
  import { dict } from "$lib/i18n";
  import Save from "~icons/lucide/save";
  import Undo from "~icons/lucide/undo-2";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { SectionNav } from "$lib/components/app";
  import { app } from "$lib/stores/app.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { form } from "./account-form.svelte";
  import AccountSection from "./AccountSection.svelte";
  import SteamSection from "./SteamSection.svelte";
  import ApisSection from "./ApisSection.svelte";
  import LocationSection from "./LocationSection.svelte";
  import PingSection from "./PingSection.svelte";
  import ExcludedSection from "./ExcludedSection.svelte";
  import AppearanceSection from "./AppearanceSection.svelte";
  import LanguageSection from "./LanguageSection.svelte";
  import WindowSection from "./WindowSection.svelte";
  import DataSection from "./DataSection.svelte";

  /**
   * Every setting on one page, with its own table of contents on the left.
   * The account fields are edited together and saved together; everything
   * else (ping, theme, language, window) takes effect as it is changed.
   */
  const s = dict("settings");

  const SECTIONS = [
    { id: "account", label: () => $s.sectionAccount.value },
    { id: "steam", label: () => $s.sectionSteam.value },
    { id: "apis", label: () => $s.sectionApis.value },
    { id: "location", label: () => $s.sectionLocation.value },
    { id: "ping", label: () => $s.sectionPing.value },
    { id: "excluded", label: () => $s.sectionExcluded.value },
    { id: "appearance", label: () => $s.sectionAppearance.value },
    { id: "language", label: () => $s.sectionLanguage.value },
    { id: "window", label: () => $s.sectionWindow.value },
    { id: "data", label: () => $s.sectionData.value },
  ];
  // The account form covers these; they get a dot while it has unsaved edits.
  const FORM = new Set(["account", "steam", "apis", "location"]);

  let scroller: HTMLDivElement | undefined = $state();
  let nav: SectionNav | undefined = $state();

  $effect(() => {
    void profile.data;
    form.sync();
  });

  // Asked to open at a section (the title bar's account chip, the theme menu).
  $effect(() => {
    if (app.view === "settings" && app.focus && nav) {
      const id = app.focus;
      app.focus = null;
      requestAnimationFrame(() => nav?.go(id));
    }
  });

  const entries = $derived(
    SECTIONS.map((sec) => ({
      id: sec.id,
      label: sec.label(),
      dot: form.dirty && FORM.has(sec.id),
    })),
  );
</script>

<div class="flex h-full min-h-0">
  <!-- The page's own index gives its width back to the fields below 1024 px,
       where it squeezed them to a dozen characters. -->
  <div class="flex max-lg:hidden">
    <SectionNav bind:this={nav} title={$s.onThisPage.value} sections={entries} root={scroller} />
  </div>

  <div class="relative flex min-w-0 flex-1 flex-col">
    <div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto flex max-w-4xl flex-col gap-7 px-6 py-5 pb-24 max-lg:px-4">
        <AccountSection />
        <SteamSection />
        <ApisSection />
        <LocationSection />
        <PingSection />
        <ExcludedSection />
        <AppearanceSection />
        <LanguageSection />
        <WindowSection />
        <DataSection />
      </div>
    </div>

    <!-- Unsaved account edits: said, and one click from saved or undone. -->
    {#if form.dirty}
      <div
        class="absolute inset-x-6 bottom-4 mx-auto flex max-w-4xl animate-slide-up items-center gap-3 rounded-md border border-warn/50
               bg-overlay px-3 py-2 shadow-pop"
      >
        <TriangleAlert class="size-icon shrink-0 text-warn" />
        <span class="flex-1 text-xs text-fg">{$s.unsaved.value}</span>
        <Button variant="ghost" onclick={() => form.discard()} disabled={form.saving}
          ><Undo class="size-icon-sm" />{$s.discard.value}</Button
        >
        <Button variant="accent" onclick={() => form.save()} disabled={form.saving}>
          {#if form.saving}<Spinner class="size-3 text-accent-fg" />{:else}<Save
              class="size-icon-sm"
            />{/if}
          {$s.save.value}
        </Button>
      </div>
    {/if}
  </div>
</div>

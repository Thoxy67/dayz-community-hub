<script lang="ts">
  import { dict } from "$lib/i18n";
  import UserRound from "~icons/lucide/user-round";
  import { Field } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { profile } from "$lib/stores/profile.svelte";
  import { SettingsSection as Section } from "$lib/components/app";
  import { form } from "./account-form.svelte";

  const s = dict("settings");
</script>

<Section id="account" title={$s.sectionAccount.value} description={$s.identity.value} icon={UserRound}>
  <div class="flex items-center gap-3 border-b border-border/60 px-pad py-3">
    <div class="grid size-12 shrink-0 place-items-center overflow-hidden rounded-full bg-raised ring-1 ring-border-strong">
      {#if profile.avatarUrl}
        <img src={profile.avatarUrl} alt="" class="size-full object-cover" />
      {:else}
        <UserRound class="size-6 text-fg-faint" />
      {/if}
    </div>
    <div class="min-w-0">
      <p class="m-0 truncate title-display text-lg text-fg">{form.f.player || $s.unnamedPlayer.value}</p>
      <p class="m-0 truncate font-mono text-2xs text-fg-faint">
        {form.f.steamLogin ? $s.steamLinked({ login: form.f.steamLogin }).value : $s.noSteamLinked.value}
      </p>
    </div>
  </div>
  <Field label={$s.ingameName.value} hint={$s.ingameNameHint.value} for="set-player">
    <Input
      id="set-player"
      bind:value={form.f.player}
      placeholder={$s.ingameNamePlaceholder.value}
      autocomplete="nickname"
      class="flex-1"
    />
  </Field>
</Section>

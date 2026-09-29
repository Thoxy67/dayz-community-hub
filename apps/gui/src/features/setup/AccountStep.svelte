<script lang="ts">
  import { dict } from "$lib/i18n";
  import Gamepad from "~icons/lucide/gamepad-2";
  import KeyRound from "~icons/lucide/key-round";
  import ShieldCheck from "~icons/lucide/shield-check";
  import { Field } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import { wizard } from "./wizard.svelte";

  const w = dict("setup");
</script>

<div class="space-y-3">
  <SectionCard title={$w.steamcmdLogin.value} icon={KeyRound}>
    <Field label={$w.username.value} for="wiz-login">
      <Input
        id="wiz-login"
        class="flex-1 [&>input]:font-mono"
        autocomplete="username"
        placeholder={$w.steamUsernamePlaceholder.value}
        bind:value={wizard.steamLogin}
      />
    </Field>
    {#if wizard.steamLogin.trim()}
      <p
        class="m-0 flex items-start gap-2 border-t border-border/60 px-3 py-2 text-2xs text-fg-muted"
      >
        <ShieldCheck class="mt-px size-icon-sm shrink-0 text-ok" />{$w.loginOnce.value}
      </p>
    {:else}
      <p class="m-0 border-t border-border/60 px-3 py-2 text-2xs text-fg-faint">
        {$w.steamRequired.value}
      </p>
    {/if}
  </SectionCard>

  <SectionCard title={$w.ingameName.value} icon={Gamepad}>
    <Field label={$w.ingameName.value} hint={$w.ingameHint({ flag: "-name" }).value} for="wiz-name">
      <Input
        id="wiz-name"
        class="flex-1"
        autocomplete="nickname"
        placeholder={$w.ingamePlaceholder.value}
        bind:value={wizard.player}
      />
    </Field>
  </SectionCard>
</div>

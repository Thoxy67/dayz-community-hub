<script lang="ts">
  import { dict } from "$lib/i18n";
  import KeyRound from "~icons/lucide/key-round";
  import ExternalLink from "~icons/lucide/external-link";
  import { Field } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Tag } from "$lib/components/ui/tag";
  import { openUrl } from "$lib/ipc/native";
  import { SettingsSection as Section } from "$lib/components/app";
  import { SecretInput as Secret } from "$lib/components/app";
  import { form } from "./account-form.svelte";

  const s = dict("settings");
  const a = dict("about");
  const steamReady = $derived(!!form.f.steamApiKey && !!form.f.steamId);
  const bmReady = $derived(!!form.f.battlemetricsApiKey);
</script>

{#snippet link(url: string, label: string)}
  <button
    type="button"
    class="inline-flex shrink-0 items-center gap-1 text-2xs text-accent hover:underline"
    onclick={() => openUrl(url)}>{label}<ExternalLink class="size-3" /></button
  >
{/snippet}

<Section id="apis" title={$s.sectionApis.value} description={$a.apiKeys.value} icon={KeyRound}>
  <div class="flex items-center gap-2 border-b border-border/60 bg-raised/30 px-pad py-1.5">
    <span class="label-stencil text-fg-muted">{$s.steamApi.value}</span>
    <Tag tone={steamReady ? "ok" : "neutral"}>{steamReady ? "✓" : "—"}</Tag>
    <span class="ml-auto truncate text-2xs text-fg-faint">{$a.apiSteamHint.value}</span>
  </div>
  <Field label={$s.apiKey.value} hint={$s.steamApiHint.value} for="set-apikey">
    <Secret id="set-apikey" bind:value={form.f.steamApiKey} placeholder="XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" />
    {@render link("https://steamcommunity.com/dev/apikey", $s.getKey.value)}
  </Field>
  <Field label={$s.steamId.value} hint={$s.steamIdHint.value} for="set-steamid">
    <Input
      id="set-steamid"
      bind:value={form.f.steamId}
      placeholder="76561198000000000"
      inputmode="numeric"
      spellcheck={false}
      class="flex-1 font-mono"
    />
    {@render link("https://steamid.io", $s.findSteamId.value)}
  </Field>
  <div class="flex items-center gap-2 border-y border-border/60 bg-raised/30 px-pad py-1.5">
    <span class="label-stencil text-fg-muted">{$s.battlemetrics.value}</span>
    <Tag tone={bmReady ? "ok" : "neutral"}>{bmReady ? "✓" : "—"}</Tag>
    <span class="ml-auto truncate text-2xs text-fg-faint">{$a.apiBmHint.value}</span>
  </div>
  <Field label={$s.apiToken.value} hint={$s.bmHint.value} for="set-bm">
    <Secret id="set-bm" bind:value={form.f.battlemetricsApiKey} placeholder="eyJ…" />
    {@render link("https://www.battlemetrics.com/developers", $s.getKey.value)}
  </Field>
</Section>

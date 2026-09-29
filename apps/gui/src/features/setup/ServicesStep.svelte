<script lang="ts">
  import { dict } from "$lib/i18n";
  import IdCard from "~icons/lucide/id-card";
  import ChartLine from "~icons/lucide/chart-line";
  import ExternalLink from "~icons/lucide/external-link";
  import { Field } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import SecretInput from "$lib/components/app/SecretInput.svelte";
  import { openUrl } from "$lib/ipc/native";
  import { wizard } from "./wizard.svelte";

  const w = dict("setup");
</script>

<div class="space-y-3">
  <SectionCard title={$w.steamApi.value} description={$w.optionalStep.value} icon={IdCard}>
    {#snippet actions()}
      <Button
        size="xs"
        variant="ghost"
        onclick={() => openUrl("https://steamcommunity.com/dev/apikey")}
      >
        <ExternalLink class="size-icon-sm" />{$w.getApiKey.value}
      </Button>
    {/snippet}
    <p class="m-0 border-b border-border/60 px-3 py-2 text-2xs leading-snug text-fg-muted">
      {$w.steamApiDesc.value}
    </p>
    <Field label={$w.apiKey.value} for="wiz-api">
      <SecretInput
        id="wiz-api"
        placeholder={$w.apiKeyPlaceholder.value}
        bind:value={wizard.steamApiKey}
      />
    </Field>
    <Field label={$w.steamIdLabel.value} for="wiz-sid">
      <Input
        id="wiz-sid"
        class="flex-1 [&>input]:font-mono"
        inputmode="numeric"
        placeholder={$w.steamIdPlaceholder.value}
        bind:value={wizard.steamId}
      />
    </Field>
  </SectionCard>

  <SectionCard
    title={$w.bmTitle.value}
    description={$w.optionalStep.value}
    icon={ChartLine}
    tone="text-ok"
  >
    {#snippet actions()}
      <Button
        size="xs"
        variant="ghost"
        onclick={() => openUrl("https://www.battlemetrics.com/developers")}
      >
        <ExternalLink class="size-icon-sm" />{$w.getToken.value}
      </Button>
    {/snippet}
    <p class="m-0 border-b border-border/60 px-3 py-2 text-2xs leading-snug text-fg-muted">
      {$w.bmDesc.value}
    </p>
    <Field label={$w.bmToken.value} for="wiz-bm">
      <SecretInput
        id="wiz-bm"
        placeholder={$w.bmTokenPlaceholder.value}
        bind:value={wizard.battlemetricsKey}
      />
    </Field>
  </SectionCard>
</div>

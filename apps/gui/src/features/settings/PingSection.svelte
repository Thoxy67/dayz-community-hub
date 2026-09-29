<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Radar from "~icons/lucide/radar";
  import { Field } from "$lib/components/ui/field";
  import { Slider } from "$lib/components/ui/slider";
  import { Switch } from "$lib/components/ui/switch";
  import { profile, type PingSettings } from "$lib/stores/profile.svelte";
  import { SettingsSection as Section } from "$lib/components/app";

  const s = useIntlayer("settings");
  const a = useIntlayer("about");

  // Local while dragging; written when the thumb is let go.
  let v = $state<PingSettings>(read());
  function read(): PingSettings {
    const p = profile.data;
    return {
      pingConcurrency: p?.ping_concurrency ?? 64,
      pingTimeoutAuto: p?.ping_timeout_auto ?? 2000,
      pingTimeoutManual: p?.ping_timeout_manual ?? 10000,
      pingMaxRetries: p?.ping_max_retries ?? 0,
      pingScanFavorites: p?.ping_scan_favorites ?? true,
      pingScanHistory: p?.ping_scan_history ?? true,
      pingScanServers: p?.ping_scan_servers ?? true,
    };
  }
  $effect(() => {
    void profile.data;
    v = read();
  });

  const save = () => profile.savePing($state.snapshot(v));
  const secs = (ms: number) => `${(ms / 1000).toFixed(ms % 1000 ? 2 : 0)} s`;

  const scopes = [
    { key: "pingScanFavorites", label: () => $a.pingScanFavorites.value },
    { key: "pingScanHistory", label: () => $a.pingScanHistory.value },
    { key: "pingScanServers", label: () => $a.pingScanServers.value },
  ] as const;
</script>

{#snippet slider(key: "pingConcurrency" | "pingTimeoutAuto" | "pingTimeoutManual" | "pingMaxRetries", min: number, max: number, step: number, shown: string, label: string, hint: string)}
  <Field {label} {hint}>
    <Slider bind:value={v[key]} {min} {max} {step} oncommit={save} aria-label={label} class="flex-1" />
    <span class="num w-16 shrink-0 text-right font-mono text-xs text-fg">{shown}</span>
  </Field>
{/snippet}

<Section id="ping" title={$s.sectionPing.value} description={$s.pingIntro.value} icon={Radar}>
  {@render slider("pingConcurrency", 5, 100, 1, String(v.pingConcurrency), $a.pingConcurrency.value, $s.pingConcurrencyHint.value)}
  {@render slider("pingTimeoutAuto", 1000, 5000, 250, secs(v.pingTimeoutAuto), $a.pingTimeoutAuto.value, $s.pingTimeoutAutoHint.value)}
  {@render slider("pingTimeoutManual", 1000, 30000, 500, secs(v.pingTimeoutManual), $a.pingTimeoutManual.value, $s.pingTimeoutManualHint.value)}
  {@render slider(
    "pingMaxRetries",
    0,
    5,
    1,
    v.pingMaxRetries === 0 ? $a.pingRetryDisabled.value : String(v.pingMaxRetries),
    $a.pingMaxRetries.value,
    $s.pingMaxRetriesHint.value,
  )}
  <Field label={$a.pingScanScope.value} hint={$s.pingScopeHint.value}>
    <div class="grid flex-1 grid-cols-3 overflow-hidden rounded-sm border border-border">
      {#each scopes as sc, i (sc.key)}
        <Switch
          bind:checked={() => v[sc.key], (c) => ((v[sc.key] = c), save())}
          label={`${i + 1}. ${sc.label()}`}
          class={i > 0 ? "border-l border-border" : ""}
        />
      {/each}
    </div>
  </Field>
</Section>

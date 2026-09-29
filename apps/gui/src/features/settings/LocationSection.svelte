<script lang="ts">
  import { dict } from "$lib/i18n";
  import MapPin from "~icons/lucide/map-pin";
  import Crosshair from "~icons/lucide/crosshair";
  import MapIcon from "~icons/lucide/map";
  import Trash from "~icons/lucide/trash-2";
  import Check from "~icons/lucide/check";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Flag } from "$lib/components/ui/flag";
  import { geolocateIp, openUrl } from "$lib/ipc/native";
  import { errorText } from "$lib/ipc/core";
  import { SettingsSection as Section } from "$lib/components/app";
  import { form } from "./account-form.svelte";

  const s = dict("settings");

  let detecting = $state(false);
  let error = $state("");
  let place = $state<{ city: string; country: string; code: string } | null>(null);
  let lat = $state("");
  let lon = $state("");

  // The manual fields follow whatever location is set.
  $effect(() => {
    const loc = form.f.userLocation;
    lat = loc ? loc[1].toFixed(4) : "";
    lon = loc ? loc[0].toFixed(4) : "";
  });

  async function detect() {
    detecting = true;
    error = "";
    try {
      const g = await geolocateIp();
      if (g.lon == null || g.lat == null) throw new Error($s.locationInvalid.value);
      form.f.userLocation = [g.lon, g.lat];
      place = { city: g.city, country: g.country, code: g.country_code };
    } catch (e) {
      error = errorText(e);
    } finally {
      detecting = false;
    }
  }

  function apply() {
    const la = parseFloat(lat);
    const lo = parseFloat(lon);
    if (Number.isNaN(la) || Number.isNaN(lo) || la < -90 || la > 90 || lo < -180 || lo > 180) {
      error = $s.locationInvalid.value;
      return;
    }
    error = "";
    place = null;
    form.f.userLocation = [lo, la];
  }

  function clear() {
    form.f.userLocation = null;
    place = null;
    error = "";
  }
</script>

<Section id="location" title={$s.sectionLocation.value} description={$s.locationHelp.value} icon={MapPin}>
  <div class="flex items-center gap-3 border-b border-border/60 px-pad py-3">
    {#if form.f.userLocation}
      <div class="grid size-10 shrink-0 place-items-center rounded-full bg-ok/10 text-ok">
        {#if place?.code}<Flag code={place.code} class="size-6" />{:else}<MapPin class="size-5" />{/if}
      </div>
      <div class="min-w-0 flex-1">
        <p class="m-0 truncate text-sm font-medium text-fg">
          {place ? [place.city, place.country].filter(Boolean).join(", ") : $s.locationSet.value}
        </p>
        <p class="m-0 font-mono text-2xs text-fg-faint">
          {form.f.userLocation[1].toFixed(4)}, {form.f.userLocation[0].toFixed(4)}
          {#if place}· {$s.locationDetected.value}{/if}
        </p>
      </div>
      <Button
        variant="ghost"
        onclick={() => openUrl(`https://www.google.com/maps?q=${form.f.userLocation![1]},${form.f.userLocation![0]}`)}
      >
        <MapIcon class="size-icon-sm" />{$s.openMaps.value}
      </Button>
      <Button variant="danger" onclick={clear}><Trash class="size-icon-sm" />{$s.clearLocation.value}</Button>
    {:else}
      <MapPin class="size-5 shrink-0 text-fg-faint" />
      <p class="m-0 flex-1 text-xs text-fg-muted">{$s.locationNone.value}</p>
    {/if}
  </div>
  <div class="flex flex-wrap items-center gap-2 px-pad py-2.5">
    <Button variant="accent" onclick={detect} disabled={detecting}>
      {#if detecting}<Spinner class="size-3 text-accent-fg" />{$s.detecting.value}{:else}<Crosshair
          class="size-icon-sm"
        />{$s.autodetectIp.value}{/if}
    </Button>
    <span class="mx-1 h-4 w-px bg-border"></span>
    <span class="text-2xs text-fg-faint">{$s.manual.value}</span>
    <Input bind:value={lat} placeholder={$s.lat.value} aria-label={$s.lat.value} class="w-24 font-mono" />
    <Input bind:value={lon} placeholder={$s.lon.value} aria-label={$s.lon.value} class="w-24 font-mono" />
    <Button onclick={apply} disabled={!lat || !lon}><Check class="size-icon-sm" />{$s.apply.value}</Button>
  </div>
  {#if error}<p class="m-0 border-t border-border/60 px-pad py-1.5 text-2xs text-err" data-selectable>{error}</p>{/if}
</Section>

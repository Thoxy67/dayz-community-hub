<script lang="ts">
  import { dict } from "$lib/i18n";
  import Database from "~icons/lucide/database";
  import Upload from "~icons/lucide/upload";
  import Download from "~icons/lucide/download";
  import RotateCcw from "~icons/lucide/rotate-ccw";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { profile } from "$lib/stores/profile.svelte";
  import { SettingsSection as Section } from "$lib/components/app";

  const s = dict("settings");
  const a = dict("about");
  let includeMods = $state(true);
</script>

<Section id="data" title={$s.sectionData.value} description={$s.dataHint.value} icon={Database}>
  <div class="flex items-center gap-3 border-b border-border/60 px-pad py-2.5">
    <div class="min-w-0 flex-1">
      <p class="m-0 text-xs font-medium text-fg">{$a.profileExport.value}</p>
      <p class="m-0 text-2xs text-fg-faint">{$a.profileExportDesc({ file: ".dchub" }).value}</p>
      <label class="mt-1 flex items-center gap-1.5 text-2xs text-fg-muted">
        <Checkbox bind:checked={includeMods} aria-label={$a.profileIncludeMods.value} />
        {$a.profileIncludeMods.value}
      </label>
    </div>
    <Button variant="accent" onclick={() => profile.exportTo(includeMods)}><Download class="size-icon-sm" />{$a.profileExport.value}</Button>
  </div>
  <div class="flex items-center gap-3 border-b border-border/60 px-pad py-2.5">
    <div class="min-w-0 flex-1">
      <p class="m-0 text-xs font-medium text-fg">{$a.profileImport.value}</p>
      <p class="m-0 text-2xs text-fg-faint">{$a.profileImportDesc({ file: ".dchub" }).value}</p>
    </div>
    <Button onclick={() => profile.importFrom()}><Upload class="size-icon-sm" />{$a.profileImport.value}</Button>
  </div>
  <div class="flex items-center gap-3 bg-err/5 px-pad py-2.5">
    <div class="min-w-0 flex-1">
      <p class="m-0 label-stencil text-err">{$s.dangerZone.value}</p>
      <p class="m-0 mt-1 text-xs font-medium text-fg">{$a.profileReset.value}</p>
      <p class="m-0 text-2xs text-fg-faint">{$a.profileResetDesc.value}</p>
    </div>
    <Button variant="danger" onclick={() => profile.reset()}><RotateCcw class="size-icon-sm" />{$a.profileReset.value}</Button>
  </div>
</Section>

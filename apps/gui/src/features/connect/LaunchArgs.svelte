<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import ChevronUp from "~icons/lucide/chevron-up";
  import ChevronDown from "~icons/lucide/chevron-down";
  import X from "~icons/lucide/x";
  import Plus from "~icons/lucide/plus";
  import Terminal from "~icons/lucide/terminal";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Select } from "$lib/components/ui/select";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Tag } from "$lib/components/ui/tag";
  import { mods } from "$lib/stores/mods.svelte";
  import { cn } from "$lib/cx";
  import { direct } from "./direct.svelte";

  /**
   * The arguments added to the launch: the server's mods (found by the query,
   * listed first), any installed mod the query did not report, and any flag
   * typed by hand. Each can be switched off, moved and removed; the exact
   * command line is previewed underneath.
   */
  const c = useIntlayer("connect");

  let pick = $state<string | null>(null);
  let custom = $state("");

  const available = $derived(
    mods.installed
      .filter((m) => !direct.args.some((a) => a.kind === "mod" && a.value === String(m.id)))
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((m) => ({ value: String(m.id), label: m.name })),
  );
</script>

<div class="flex flex-col">
  <p class="m-0 px-3 pt-2 text-3xs leading-snug text-fg-faint">{$c.extraModsHint.value}</p>

  {#if direct.args.length === 0}
    <p class="m-0 px-3 py-2 text-2xs text-fg-muted">{$c.noModes.value}</p>
  {:else}
    <ul class="m-0 mt-1.5 max-h-64 list-none overflow-y-auto border-y border-border/60 p-0">
      {#each direct.args as a, i (a.id)}
        <li class={cn("group flex h-row items-center gap-2 border-b border-border/40 px-3 last:border-0", !a.enabled && "opacity-50")}>
          <Checkbox
            checked={a.enabled}
            onchange={() => direct.toggleArg(a.id)}
            aria-label={a.enabled ? $c.disable.value : $c.enable.value}
          />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-2xs text-fg">{a.label}</span>
            <span class="block truncate font-mono text-3xs text-fg-faint">
              {a.kind === "mod" ? `-mod=@${a.value}` : a.value}
            </span>
          </span>
          <Tag tone={a.fromServer ? "accent" : "neutral"}>
            {a.fromServer ? $c.detected.value : a.kind === "mod" ? $c.mod.value : $c.custom.value}
          </Tag>
          <span class="flex opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
            <button
              class="grid size-5 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-30"
              aria-label={$c.moveUp.value}
              title={$c.moveUp.value}
              disabled={i === 0}
              onclick={() => direct.moveArg(a.id, -1)}><ChevronUp class="size-3" /></button
            >
            <button
              class="grid size-5 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-30"
              aria-label={$c.moveDown.value}
              title={$c.moveDown.value}
              disabled={i === direct.args.length - 1}
              onclick={() => direct.moveArg(a.id, 1)}><ChevronDown class="size-3" /></button
            >
            <button
              class="grid size-5 place-items-center rounded-xs text-fg-faint hover:bg-err/15 hover:text-err"
              aria-label={$c.remove.value}
              title={$c.remove.value}
              onclick={() => direct.removeArg(a.id)}><X class="size-3" /></button
            >
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex flex-col gap-1.5 px-3 py-2">
    {#if mods.installed.length === 0}
      <p class="m-0 text-3xs text-fg-faint">{$c.noInstalledMods.value}</p>
    {:else if available.length === 0}
      <p class="m-0 text-3xs text-fg-faint">{$c.allModsAdded.value}</p>
    {:else}
      <div class="flex gap-1.5">
        <Select bind:value={pick} options={available} placeholder={$c.pickMod.value} aria-label={$c.pickMod.value} class="min-w-0 flex-1" />
        <Button
          disabled={!pick}
          onclick={() => {
            if (pick) direct.addMod(Number(pick));
            pick = null;
          }}><Plus class="size-icon-sm" />{$c.addMod.value}</Button
        >
      </div>
    {/if}
    <form
      class="flex gap-1.5"
      onsubmit={(e) => {
        e.preventDefault();
        direct.addCustom(custom);
        custom = "";
      }}
    >
      <Input bind:value={custom} placeholder={$c.customArgPlaceholder.value} aria-label={$c.customArg.value} class="min-w-0 flex-1 font-mono" />
      <Button type="submit" disabled={!custom.trim()}><Plus class="size-icon-sm" />{$c.addArg.value}</Button>
    </form>
  </div>

  <div class="border-t border-border/60 px-3 py-2">
    <div class="mb-1 flex items-center gap-1.5 text-3xs text-fg-faint uppercase">
      <Terminal class="size-3" />{$c.launchPreview.value}
    </div>
    <code
      class="block max-h-24 overflow-y-auto rounded-sm border border-border bg-plot px-2 py-1.5 font-mono text-3xs leading-relaxed break-all text-fg-muted"
      data-selectable
    >
      -connect={direct.ip || "…"} -port={direct.gamePort || "…"}{direct.password ? " -password=••••" : ""}{#each direct.launchArgs as arg (arg)}{" "}<span class="text-mods">{arg}</span>{/each}
    </code>
  </div>
</div>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import Penguin from "~icons/lucide/terminal-square";
  import Check from "~icons/lucide/check";
  import CopyIcon from "~icons/lucide/copy";
  import { Tag } from "$lib/components/ui/tag";
  import { Copy } from "$lib/components/ui/copy";
  import { Disclosure } from "$lib/components/ui/disclosure";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import { copyText } from "$lib/ipc/native";
  import type { SteamLaunchInfoDto } from "$lib/ipc/bindings";
  import { cn } from "$lib/cx";
  import { parseSteamOptions } from "./steam-options";

  /**
   * Linux only: how DayZ really starts. The launcher's own command (with the
   * options picked on this page), then what the Steam client wraps it in:
   * the Proton build, the prefix it runs in, and the launch options set in
   * Steam, taken apart into variables, wrappers and arguments.
   */
  let { info, flags }: { info: SteamLaunchInfoDto; flags: readonly string[] } = $props();
  const o = dict("options");

  const program = $derived(info.launcher.join(" "));
  const full = $derived([...info.launcher, ...info.applaunch, ...flags].join(" "));
  const kind = $derived(
    info.launcher.length === 0
      ? $o.linuxSteamMissing.value
      : info.launcher.includes("com.valvesoftware.Steam")
        ? $o.linuxSteamFlatpak.value
        : info.launcher[0]!.includes("snap")
          ? $o.linuxSteamSnap.value
          : $o.linuxSteamNative.value,
  );
  const steam = $derived(info.launch_options ? parseSteamOptions(info.launch_options) : null);

  let copied = $state(false);
  async function copy() {
    await copyText(full);
    copied = true;
    setTimeout(() => (copied = false), 1400);
  }
</script>

{#snippet row(label: string)}
  <dt class="pt-px text-2xs text-fg-faint">{label}</dt>
{/snippet}

<SectionCard title={$o.linuxTitle.value} icon={Penguin}>
  <div class="flex flex-col gap-3 px-3 py-2.5">
    <p class="m-0 text-2xs leading-snug text-fg-muted">{$o.linuxDesc.value}</p>

    <dl class="m-0 grid grid-cols-[6.5rem_minmax(0,1fr)] gap-x-3 gap-y-2">
      {@render row($o.linuxSteam.value)}
      <dd class="m-0 min-w-0">
        <span class="text-xs text-fg">{kind}</span>
        {#if program}<span class="block truncate font-mono text-3xs text-fg-faint" title={program}
            >{program}</span
          >{/if}
      </dd>

      {@render row($o.linuxProton.value)}
      <dd class="m-0 flex min-w-0 flex-wrap items-center gap-1.5">
        {#if info.compat_tool}
          <span class="font-mono text-xs text-fg">{info.compat_tool}</span>
          <Tag tone={info.compat_tool_default ? "neutral" : "accent"}
            >{info.compat_tool_default
              ? $o.linuxProtonDefault.value
              : $o.linuxProtonChosen.value}</Tag
          >
        {:else}
          <span class="text-xs text-warn">{$o.linuxProtonNone.value}</span>
        {/if}
      </dd>

      {@render row($o.linuxPrefix.value)}
      <dd class="m-0 min-w-0">
        {#if info.prefix}
          <Copy
            text={info.prefix}
            class="max-w-full text-left text-fg-muted [&>span]:break-all [&>span]:whitespace-normal"
          />
          <span class="mt-0.5 block text-3xs leading-snug text-fg-faint"
            >{$o.linuxPrefixHint.value}</span
          >
        {:else}
          <span class="text-2xs text-fg-faint">{$o.linuxPrefixNone.value}</span>
        {/if}
      </dd>
    </dl>

    <!-- The launcher's command, as it will run. -->
    <div class="flex flex-col gap-1">
      <div class="flex items-center gap-2">
        <span class="label-stencil text-fg-faint">{$o.linuxCommand.value}</span>
        <button
          class={cn(
            "ml-auto grid size-control-sm place-items-center rounded-sm hover:bg-raised",
            copied ? "text-ok" : "text-fg-faint hover:text-fg",
          )}
          aria-label={$o.copyCommand.value}
          title={$o.copyCommand.value}
          onclick={copy}
        >
          {#if copied}<Check class="size-3.5" />{:else}<CopyIcon class="size-3.5" />{/if}
        </button>
      </div>
      <code
        class="block rounded-sm border border-border bg-bg px-2 py-1.5 font-mono text-3xs leading-relaxed break-words text-fg-muted"
        data-selectable
      >
        <span class="text-fg-faint">{program || "steam"}</span>
        {#each info.applaunch as a (a)}{" "}<span
            class="whitespace-nowrap {a === '-malloc=system' ? 'text-accent' : 'text-fg'}">{a}</span
          >{/each}
        {#each flags as f (f)}{" "}<span class="whitespace-nowrap text-info">{f}</span>{/each}
      </code>
      <p class="m-0 text-3xs leading-snug text-fg-faint">
        {$o.linuxMalloc.value}
        {$o.linuxCommandHint.value}
      </p>
    </div>

    <!-- What Steam adds from the game's properties. -->
    <div class="flex flex-col gap-1.5 border-t border-border/60 pt-2.5">
      <span class="label-stencil text-fg-faint">{$o.linuxSteamOptions.value}</span>
      {#if steam}
        {#if steam.env.length}
          <p class="m-0 text-2xs text-fg-faint">{$o.linuxEnv.value}</p>
          <ul class="m-0 flex list-none flex-col gap-0.5 p-0" data-selectable>
            {#each steam.env as [k, v] (k)}
              <li class="flex min-w-0 gap-1 font-mono text-3xs">
                <span class="shrink-0 text-accent">{k}</span><span class="text-fg-faint">=</span
                ><span class="min-w-0 break-all text-fg">{v}</span>
              </li>
            {/each}
          </ul>
        {/if}
        {#if steam.wrappers.length}
          <p class="m-0 text-2xs text-fg-faint">{$o.linuxWrappers.value}</p>
          <div class="flex flex-wrap gap-1">
            {#each steam.wrappers as w, i (i)}<code
                class="rounded-xs bg-raised px-1.5 py-px font-mono text-3xs text-fg">{w}</code
              >{/each}
          </div>
        {/if}
        {#if steam.args.length}
          <p class="m-0 text-2xs text-fg-faint">{$o.linuxArgs.value}</p>
          <div class="flex flex-wrap gap-1">
            {#each steam.args as a, i (i)}<code
                class="rounded-xs bg-raised px-1.5 py-px font-mono text-3xs text-info">{a}</code
              >{/each}
          </div>
          {#if !steam.hasCommand}
            <p class="m-0 text-3xs text-fg-faint">{$o.linuxNoCommand.value}</p>
          {/if}
        {/if}
        <Disclosure label={$o.linuxRaw.value} class="-mx-2">
          <code
            class="mx-2 mb-1 block rounded-sm border border-border bg-bg px-2 py-1.5 font-mono text-3xs break-all text-fg-muted"
            data-selectable>{info.launch_options}</code
          >
        </Disclosure>
      {:else}
        <p class="m-0 text-2xs text-fg-faint">{$o.linuxNoOptions.value}</p>
      {/if}
      <p class="m-0 text-3xs leading-snug text-fg-faint">{$o.linuxSteamOptionsHint.value}</p>
    </div>
  </div>
</SectionCard>

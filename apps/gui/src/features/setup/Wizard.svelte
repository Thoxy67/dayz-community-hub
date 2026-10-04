<script lang="ts">
  import { dict } from "$lib/i18n";
  import ServerIcon from "~icons/lucide/server";
  import Puzzle from "~icons/lucide/puzzle";
  import Star from "~icons/lucide/star";
  import Play from "~icons/lucide/play";
  import Check from "~icons/lucide/check";
  import ArrowLeft from "~icons/lucide/arrow-left";
  import ArrowRight from "~icons/lucide/arrow-right";
  import FileUp from "~icons/lucide/file-up";
  import CircleCheck from "~icons/lucide/circle-check";
  import CircleDashed from "~icons/lucide/circle-dashed";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Languages from "~icons/lucide/languages";
  import Palette from "~icons/lucide/palette";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import SectionCard from "$lib/components/app/SectionCard.svelte";
  import LanguagePicker from "$lib/components/app/LanguagePicker.svelte";
  import ThemeSwatches from "$lib/components/app/ThemeSwatches.svelte";
  import { LOCALE_LABELS, getLocale } from "$lib/i18n";
  import { theme } from "$lib/theme/theme.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import { STEPS, wizard, type Step } from "./wizard.svelte";
  import SteamcmdStep from "./SteamcmdStep.svelte";
  import AccountStep from "./AccountStep.svelte";
  import ServicesStep from "./ServicesStep.svelte";

  /**
   * The first thing a new player sees: six steps down the left, the one in
   * hand on the right, and a way out at every step. Everything asked here
   * can be changed later in Settings.
   */
  const w = dict("setup");
  const nav = dict("nav");
  const tt = dict("theme");

  /** The worn preset's own name. */
  function themeName(id: string | null): string {
    if (!id) return $w.followSystem.value;
    const key = `preset${id.replace(/(^|_)(\w)/g, (_m, _s, c: string) => c.toUpperCase())}`;
    return ($tt as unknown as Record<string, { value?: string } | undefined>)[key]?.value ?? id;
  }

  const TITLE: Record<Step, () => string> = {
    welcome: () => $w.welcomeTitle.value,
    steamcmd: () => $w.steamcmdTitle.value,
    account: () => $w.configTitle.value,
    services: () => $w.stepServices.value,
    appearance: () => $w.appearanceTitle.value,
    done: () => $w.doneTitle.value,
  };
  const DESC: Record<Step, () => string> = {
    welcome: () => $w.welcomeSubtitle.value,
    steamcmd: () => $w.steamcmdDesc.value,
    account: () => $w.accountDesc.value,
    services: () => $w.servicesDesc.value,
    appearance: () => $w.appearanceDesc.value,
    done: () => $w.doneDesc({ button: $w.launch.value }).value,
  };
  const LABEL: Record<Step, () => string> = {
    welcome: () => $w.stepWelcome.value,
    steamcmd: () => $w.stepSteamcmd.value,
    account: () => $w.stepAccount.value,
    services: () => $w.stepServices.value,
    appearance: () => $w.stepAppearance.value,
    done: () => $w.stepDone.value,
  };

  const progress = $derived(((wizard.index + 1) / STEPS.length) * 100);

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Enter" || (e.target as HTMLElement).closest("button, textarea")) return;
    e.preventDefault();
    if (wizard.step === "done") void wizard.finish();
    else wizard.next();
  }
</script>

<div
  class="fixed inset-x-0 top-titlebar bottom-0 z-dialog flex animate-fade-in bg-bg"
  role="dialog"
  aria-modal="true"
  aria-label={$w.welcomeTitle.value}
  tabindex="-1"
  {onkeydown}
>
  <!-- The steps. -->
  <aside class="relative flex w-72 shrink-0 flex-col overflow-hidden border-r border-border bg-bg">
    <Topo opacity={0.55} />
    <div class="relative px-6 pt-7">
      <img src="/icon.svg" alt="" class="size-10" />
      <p class="m-0 mt-3 title-display text-2xl leading-none text-fg">
        DayZ<br /><span class="text-accent">Community Hub</span>
      </p>
    </div>
    <ol class="relative m-0 mt-8 flex list-none flex-col gap-0.5 px-4 py-0">
      {#each STEPS as s, i (s)}
        {@const done = i < wizard.index}
        {@const on = i === wizard.index}
        <li>
          <button
            class={cn(
              "flex w-full items-center gap-3 rounded-sm px-2 py-2 text-left transition-colors",
              on ? "bg-raised" : "hover:bg-raised/50",
              i > wizard.index && "opacity-60",
            )}
            disabled={i > wizard.index + 1 || (i > wizard.index && !wizard.canAdvance)}
            onclick={() => wizard.go(s)}
          >
            <span
              class={cn(
                "grid size-6 shrink-0 place-items-center rounded-full border font-mono text-2xs",
                on
                  ? "border-accent bg-accent text-accent-fg"
                  : done
                    ? "border-ok/50 bg-ok/15 text-ok"
                    : "border-border-strong text-fg-faint",
              )}
            >
              {#if done}<Check class="size-3.5" />{:else}{i + 1}{/if}
            </span>
            <span class={cn("text-sm", on ? "font-semibold text-fg" : "text-fg-muted")}
              >{LABEL[s]()}</span
            >
            {#if s === "services"}<span class="ml-auto text-3xs text-fg-faint italic"
                >{$w.optionalStep.value}</span
              >{/if}
          </button>
        </li>
      {/each}
    </ol>
    <div class="relative mt-auto border-t border-border/70 px-6 py-4">
      <p class="m-0 text-2xs leading-snug text-fg-faint">{$w.importHint.value}</p>
      <Button size="xs" variant="default" class="mt-2" onclick={() => profile.importFrom()}>
        <FileUp class="size-icon-sm" />{$w.importProfile.value}
      </Button>
    </div>
  </aside>

  <!-- The step in hand. -->
  <section class="flex min-w-0 flex-1 flex-col">
    <div class="h-0.5 shrink-0 bg-raised">
      <div
        class="h-full bg-accent transition-[width] duration-300"
        style="width: {progress}%"
      ></div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto max-w-2xl px-8 pt-10 pb-8">
        <p class="m-0 label-stencil text-accent">
          {$w.stepOf({ n: wizard.index + 1, total: STEPS.length }).value}
        </p>
        <h1 class="m-0 mt-2 title-display text-3xl text-fg">{TITLE[wizard.step]()}</h1>
        <p class="m-0 mt-2 max-w-xl text-sm leading-relaxed text-fg-muted">{DESC[wizard.step]()}</p>

        <div class="mt-7 animate-slide-up">
          {#key wizard.step}
            {#if wizard.step === "welcome"}
              <ul class="m-0 grid list-none grid-cols-2 gap-2 p-0">
                {#each [[ServerIcon, $w.welcomeFeat1.value, "text-info"], [Puzzle, $w.welcomeFeat2.value, "text-mods"], [Star, $w.welcomeFeat3.value, "text-warn"], [Play, $w.welcomeFeat4.value, "text-accent"]] as [Icon, text, tone], i (i)}
                  {@const I = Icon as typeof ServerIcon}
                  <li
                    class="flex items-start gap-3 rounded-md border border-border bg-panel px-3 py-3"
                  >
                    <span class="grid size-8 shrink-0 place-items-center rounded-sm bg-raised">
                      <I class={cn("size-icon", tone as string)} />
                    </span>
                    <span class="text-xs leading-snug text-fg">{text}</span>
                  </li>
                {/each}
              </ul>
              <p class="m-0 mt-4 text-2xs text-fg-faint">{$w.welcomeHint.value}</p>
            {:else if wizard.step === "steamcmd"}
              <SteamcmdStep />
            {:else if wizard.step === "account"}
              <AccountStep />
            {:else if wizard.step === "services"}
              <ServicesStep />
            {:else if wizard.step === "appearance"}
              <div class="space-y-3">
                <SectionCard title={$w.language.value} icon={Languages} padded>
                  <LanguagePicker aria-label={$w.language.value} />
                </SectionCard>
                <SectionCard title={$w.theme.value} icon={Palette} padded>
                  <ThemeSwatches systemLabel={$w.followSystem.value} columns={5} />
                </SectionCard>
              </div>
            {:else}
              {@const rows = [
                {
                  label: $w.stepSteamcmd.value,
                  ok: wizard.found,
                  value: wizard.steamcmdPath || wizard.status?.path || $w.missing.value,
                  warn: !wizard.found,
                },
                {
                  label: $w.username.value,
                  ok: !!wizard.steamLogin.trim(),
                  value: wizard.steamLogin || $w.missing.value,
                  warn: !wizard.steamLogin.trim(),
                },
                {
                  label: $w.ingameName.value,
                  ok: !!wizard.player.trim(),
                  value: wizard.player || $w.notSet.value,
                },
                {
                  label: $w.apiKey.value,
                  ok: !!wizard.steamApiKey.trim(),
                  value: wizard.steamApiKey ? $w.configured.value : $w.notSet.value,
                },
                { label: $w.language.value, ok: true, value: LOCALE_LABELS[getLocale()] },
                { label: $w.theme.value, ok: true, value: themeName(theme.selected) },
              ]}
              <SectionCard title={$w.summary.value} icon={CircleCheck} tone="text-ok">
                <dl class="m-0 divide-y divide-border/50">
                  {#each rows as r (r.label)}
                    <div class="flex items-center gap-3 px-3 py-2">
                      {#if r.warn}<TriangleAlert
                          class="size-icon-sm shrink-0 text-warn"
                        />{:else if r.ok}<CircleCheck
                          class="size-icon-sm shrink-0 text-ok"
                        />{:else}<CircleDashed class="size-icon-sm shrink-0 text-fg-faint" />{/if}
                      <dt class="w-40 shrink-0 text-xs text-fg-muted">{r.label}</dt>
                      <dd
                        class={cn(
                          "m-0 min-w-0 truncate font-mono text-2xs",
                          r.ok ? "text-fg" : r.warn ? "text-warn" : "text-fg-faint",
                        )}
                      >
                        {r.value}
                      </dd>
                    </div>
                  {/each}
                </dl>
              </SectionCard>
              <p class="m-0 mt-4 text-2xs text-fg-faint">{$w.doneSettingsHint.value}</p>
              <p class="m-0 mt-1 text-2xs text-fg-faint">
                {$w.doneAboutHint({ tab: $nav.about.value }).value}
              </p>
            {/if}
          {/key}
        </div>
      </div>
    </div>

    <footer class="flex shrink-0 items-center gap-2 border-t border-border bg-panel px-8 py-3">
      {#if wizard.index > 0}
        <Button variant="ghost" onclick={() => wizard.back()}
          ><ArrowLeft class="size-icon-sm" />{$w.back.value}</Button
        >
      {/if}
      <span class="ml-auto font-mono text-3xs text-fg-faint"
        >{wizard.index + 1} / {STEPS.length}</span
      >
      {#if wizard.step === "done"}
        <Button variant="play" size="lg" disabled={wizard.saving} onclick={() => wizard.finish()}>
          {#if wizard.saving}<Spinner class="size-icon-sm text-accent-fg" />{$w.saving
              .value}{:else}<Play class="size-icon-sm" />{$w.launch.value}{/if}
        </Button>
      {:else}
        <Button
          variant="accent"
          size="lg"
          disabled={!wizard.canAdvance}
          onclick={() => wizard.next()}
        >
          {wizard.step === "welcome" ? $w.getStarted.value : $w.next.value}<ArrowRight
            class="size-icon-sm"
          />
        </Button>
      {/if}
    </footer>
  </section>
</div>

<script lang="ts">
  import { dict } from "$lib/i18n";
  import Keyboard from "~icons/lucide/keyboard";
  import Gamepad from "~icons/lucide/gamepad-2";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Segmented } from "$lib/components/ui/segmented";
  import { cn } from "$lib/cx";
  import { pad } from "$lib/gamepad/state.svelte";
  import {
    chips,
    KEY_GROUPS,
    PAD_GROUPS,
    SHORTCUTS,
    type Scope,
    type Shortcut,
  } from "$lib/shortcuts/registry";
  import PadGlyph from "./PadGlyph.svelte";

  /**
   * Every shortcut, by where it works, read from the one registry
   * (`lib/shortcuts/registry.ts`): the keyboard's, or the controller's in
   * the glyphs of the pad in hand. The `?` sheet, the About page and the
   * Controller settings all show this, so none can tell a different story.
   */
  let {
    mode = $bindable(pad.mode === "gamepad" ? "pad" : "keys"),
    tabs = true,
    class: klass = "",
  }: { mode?: "keys" | "pad"; tabs?: boolean; class?: string } = $props();

  const k = dict("keys");
  const nav = dict("nav");

  const GROUP: Partial<Record<Scope, string>> = {
    global: "groupGlobal",
    views: "groupViews",
    list: "groupList",
    saved: "groupSaved",
    mods: "groupMods",
    news: "groupNews",
    palette: "groupPalette",
    dialog: "groupDialog",
    pad: "groupPad",
  };
  const words = (key: string) => String(($k as unknown as Record<string, unknown>)[key] ?? key);
  const titleOf = (scope: Scope) =>
    GROUP[scope] ? words(GROUP[scope]) : String($nav[scope as keyof typeof $nav] ?? scope);
  const labelOf = (s: Shortcut) =>
    s.view ? String($nav[s.view as keyof typeof $nav]) : words(s.label ?? "");

  const groups = $derived(
    (mode === "keys" ? KEY_GROUPS : PAD_GROUPS)
      .map((scope) => ({
        scope,
        rows: SHORTCUTS.filter(
          (s) => s.scope === scope && (mode === "keys" ? s.keys.length > 0 : !!s.pad?.length),
        ),
      }))
      .filter((g) => g.rows.length > 0),
  );
</script>

{#snippet combo(c: string)}
  <span class="inline-flex items-center gap-0.5">
    {#each chips(c) as part, i (i)}{#if i > 0}<span class="text-3xs text-fg-faint">+</span>{/if}<Kbd
        >{part}</Kbd
      >{/each}
  </span>
{/snippet}

<div class="flex flex-col gap-3">
  {#if tabs}
    <Segmented
      bind:value={mode}
      size="xs"
      fill
      aria-label={$k.title.value}
      options={[
        { value: "keys", label: $k.tabKeys.value, icon: Keyboard },
        { value: "pad", label: $k.tabPad.value, icon: Gamepad },
      ]}
    />
  {/if}
  <div class={cn("flex flex-col gap-3", klass)}>
    {#each groups as g (g.scope)}
      <section class="min-w-0 break-inside-avoid">
        <h3
          class="m-0 mb-1 font-mono text-3xs font-normal tracking-[0.08em] text-fg-faint uppercase"
        >
          {titleOf(g.scope)}
        </h3>
        {#if g.scope === "views" && mode === "keys"}
          <!-- Twelve views: two columns of chips rather than twelve long rows. -->
          <ul class="m-0 grid list-none grid-cols-2 gap-x-3 p-0">
            {#each g.rows as r (r.id)}
              <li class="flex min-w-0 items-center gap-2 border-b border-border/40 py-1">
                <span class="min-w-0 flex-1 truncate text-2xs text-fg-muted" title={labelOf(r)}
                  >{labelOf(r)}</span
                >
                {#each r.keys as c (c)}{@render combo(c)}{/each}
              </li>
            {/each}
          </ul>
        {:else}
          <ul class="m-0 flex list-none flex-col p-0">
            {#each g.rows as r (r.id)}
              <li class="flex items-center gap-2 border-b border-border/40 py-1 last:border-b-0">
                <span class="min-w-0 flex-1 text-2xs text-fg-muted">{labelOf(r)}</span>
                <span class="flex shrink-0 items-center gap-1">
                  {#if mode === "keys"}
                    {#each r.keys as c, i (c)}
                      {#if i > 0}<span class="text-3xs text-fg-faint"
                          >{r.pair ? "/" : $k.or.value}</span
                        >{/if}
                      {@render combo(c)}
                    {/each}
                  {:else}
                    {#each r.pad ?? [] as b, i (b)}
                      {#if i > 0}<span class="text-3xs text-fg-faint">/</span>{/if}
                      <PadGlyph button={b} kind={pad.kind} />
                    {/each}
                  {/if}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}
  </div>
  <p class="m-0 text-2xs leading-snug text-fg-faint">
    {#if mode === "keys"}{$k.typingNote.value} {$k.rightClick.value}{:else}{$k.padNote.value}{/if}
  </p>
</div>

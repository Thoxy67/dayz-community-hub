<script lang="ts">
  import Gamepad from "~icons/lucide/gamepad-2";
  import { dict } from "$lib/i18n";
  import { PadGlyph, type PadButton } from "$lib/components/app";
  import { registry } from "$lib/gamepad/actions.svelte";
  import { pad } from "$lib/gamepad/state.svelte";
  import { app } from "$lib/stores/app.svelte";

  /**
   * What each button does here, while a controller drives the window: the
   * fixed ones (select, back, views, pages) and the view's own (X, Y), which
   * come and go with what is focused.
   */
  const p = dict("pad");

  const typing = $derived(
    pad.focused?.matches("input:not([type=checkbox],[type=radio],[type=range]), textarea") ?? false,
  );
  // Read `pad.focused` so a view's `when` is asked again as the focus moves.
  const own = $derived(
    (void pad.focused,
    (["x", "y", "menu", "view"] as const).map((b) => {
      const c = registry.get(app.view, b === "x" ? "primary" : b === "y" ? "secondary" : b);
      return { b, label: c?.label() ?? null };
    })),
  );
  const label = (b: "menu" | "view", fallback: string) =>
    own.find((o) => o.b === b)?.label ?? fallback;
</script>

{#snippet hint(buttons: PadButton[], text: string)}
  <span class="flex shrink-0 items-center gap-1">
    {#each buttons as b (b)}<PadGlyph button={b} kind={pad.kind} />{/each}
    <span class="text-fg-muted">{text}</span>
  </span>
{/snippet}

<div
  class="flex h-7 shrink-0 items-center gap-3.5 overflow-hidden border-t border-border bg-panel px-pad text-2xs"
  role="note"
  aria-label={$p.hints.value}
  data-pad-skip
>
  {@render hint(["a"], typing ? $p.type.value : $p.select.value)}
  {@render hint(["b"], $p.back.value)}
  {#each own as o (o.b)}
    {#if (o.b === "x" || o.b === "y") && o.label}{@render hint([o.b], o.label)}{/if}
  {/each}
  {@render hint(["lb", "rb"], $p.views.value)}
  {@render hint(["lt", "rt"], $p.page.value)}
  {@render hint(["menu"], label("menu", $p.settings.value))}
  {@render hint(["view"], label("view", $p.search.value))}
  <span class="flex-1"></span>
  {#if pad.pads[0]}
    <span class="flex min-w-0 items-center gap-1.5 text-fg-faint">
      <Gamepad class="size-3.5 shrink-0" />
      <span class="truncate">{pad.pads[0].name}</span>
    </span>
  {/if}
</div>

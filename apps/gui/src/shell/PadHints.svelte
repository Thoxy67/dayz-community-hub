<script lang="ts">
  import Gamepad from "~icons/lucide/gamepad-2";
  import { dict } from "$lib/i18n";
  import { PadGlyph, type PadButton } from "$lib/components/app";
  import { registry } from "$lib/gamepad/actions.svelte";
  import { dialogConfirm } from "$lib/gamepad/nav";
  import { pad } from "$lib/gamepad/state.svelte";
  import { shortcut } from "$lib/shortcuts/registry";
  import { app } from "$lib/stores/app.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";

  /**
   * What the buttons do on what is focused, while a controller drives the
   * window: at most five, always in the same order (A, B, X, Y, then Menu,
   * the bumpers, R3, View as room allows), and only those that apply: X and
   * Y appear on a row and vanish off it, a dialog shows its own three.
   * The words are the registry's (`lib/shortcuts/registry.ts`).
   */
  const p = dict("pad");
  const MAX = 5;

  // Asked again when a dialog's portal comes or goes on the body (a dialog's
  // focus trap keeps focus events from reaching the document), and once it
  // is drawn, since at mount it may not count as visible yet.
  let moved = $state(0);
  $effect(() => {
    const bump = () => {
      moved++;
      requestAnimationFrame(() => moved++);
      setTimeout(() => moved++, 300);
    };
    const watch = new MutationObserver(bump);
    watch.observe(document.body, { childList: true });
    document.addEventListener("focusin", bump, true);
    return () => {
      watch.disconnect();
      document.removeEventListener("focusin", bump, true);
    };
  });

  const word = (key: string | undefined) =>
    key ? String(($p as unknown as Record<string, unknown>)[key] ?? key) : "";
  const hintOf = (id: string) => word(shortcut(id).hint);

  type Hint = { buttons: PadButton[]; text: string };

  const hints = $derived.by((): Hint[] => {
    void moved;
    const el = pad.focused;
    const typing =
      el?.matches("input:not([type=checkbox],[type=radio],[type=range]), textarea") ?? false;
    const out: Hint[] = [];
    if (dialogs.palette) {
      out.push({ buttons: ["a"], text: hintOf("palette.run") });
      out.push({ buttons: ["b"], text: hintOf("palette.close") });
      out.push({ buttons: ["lb", "rb"], text: hintOf("palette.section") });
      if (pad.steamUi) out.push({ buttons: ["y"], text: hintOf("palette.keyboard") });
      return out;
    }
    const confirm = dialogConfirm() !== null;
    const inDialog = !!document.querySelector(
      "[data-dialog-content][data-state=open],[role=alertdialog]",
    );
    const onRow = !!el?.closest("[data-pad-index]");
    const selected = el?.querySelector("[aria-selected=true]") ?? null;
    out.push({
      buttons: ["a"],
      text: typing ? $p.type.value : onRow && selected ? $p.join.value : $p.select.value,
    });
    out.push({
      buttons: ["b"],
      text: inDialog ? $p.close.value : typing ? $p.done.value : $p.back.value,
    });
    if (inDialog) {
      if (confirm) out.push({ buttons: ["x"], text: hintOf("dialog.confirm") });
      return out;
    }
    for (const b of ["x", "y"] as const) {
      const c = registry.get(app.view, b);
      if (c) out.push({ buttons: [b], text: hintOf(c.id) });
    }
    out.push({ buttons: ["menu"], text: hintOf("palette") });
    out.push({ buttons: ["lb", "rb"], text: hintOf("view.prev") });
    const r3 = registry.get(app.view, "r3");
    if (r3) out.push({ buttons: ["r3"], text: hintOf(r3.id) });
    out.push({ buttons: ["view"], text: hintOf("sheet") });
    return out.slice(0, MAX);
  });
</script>

<div
  class="flex h-7 shrink-0 items-center gap-3.5 overflow-hidden border-t border-border bg-panel px-pad text-2xs"
  role="note"
  aria-label={$p.hints.value}
  data-pad-skip
>
  {#each hints as h (h.buttons.join())}
    <span class="flex shrink-0 items-center gap-1">
      {#each h.buttons as b (b)}<PadGlyph button={b} kind={pad.kind} />{/each}
      <span class="text-fg-muted">{h.text}</span>
    </span>
  {/each}
  <span class="flex-1"></span>
  {#if pad.pads[0]}
    <span class="flex min-w-0 items-center gap-1.5 text-fg-faint">
      <Gamepad class="size-3.5 shrink-0" />
      <span class="truncate">{pad.pads[0].name}</span>
    </span>
  {/if}
</div>

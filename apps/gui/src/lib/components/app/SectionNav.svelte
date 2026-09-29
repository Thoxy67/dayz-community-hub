<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * A page's own table of contents: sticky beside a long scrolling page of
   * anchored sections (`SettingsSection` ids). It follows the reader down the
   * page and scrolls to a section on click; `go(id)` does the same from code.
   */
  type Entry = {
    id: string;
    label: string;
    /** A small warning dot: unsaved edits there. */ dot?: boolean;
  };

  let {
    title,
    sections,
    root,
  }: {
    title: string;
    sections: readonly Entry[];
    /** The scrolling element the sections live in. */
    root: HTMLElement | undefined;
  } = $props();

  let active = $state("");

  export function go(id: string) {
    root
      ?.querySelector(`#${CSS.escape(id)}`)
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  $effect(() => {
    const el = root;
    if (!el) return;
    // The section whose top has passed the top of the page is the one being read.
    const spy = () => {
      const top = el.getBoundingClientRect().top + 24;
      let best = sections[0]?.id ?? "";
      for (const s of sections) {
        const sec = el.querySelector(`#${CSS.escape(s.id)}`);
        if (sec && sec.getBoundingClientRect().top <= top) best = s.id;
      }
      active = best;
    };
    spy();
    el.addEventListener("scroll", spy, { passive: true });
    return () => el.removeEventListener("scroll", spy);
  });
</script>

<nav
  class="flex w-52 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-border bg-bg/40 px-2 py-4"
  aria-label={title}
>
  <p class="m-0 px-2 pb-2 label-stencil text-fg-faint">{title}</p>
  {#each sections as s (s.id)}
    <button
      type="button"
      class={cn(
        "relative flex h-control shrink-0 items-center gap-2 rounded-sm px-2 text-left text-xs transition-colors",
        active === s.id ? "bg-raised text-fg" : "text-fg-muted hover:bg-raised/60 hover:text-fg",
      )}
      aria-current={active === s.id ? "location" : undefined}
      onclick={() => go(s.id)}
    >
      {#if active === s.id}<span class="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-accent"
        ></span>{/if}
      <span class="flex-1 truncate">{s.label}</span>
      {#if s.dot}<span class="size-1.5 rounded-full bg-warn"></span>{/if}
    </button>
  {/each}
</nav>

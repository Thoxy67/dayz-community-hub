<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Kbd } from "$lib/components/ui/kbd";
  import { cn } from "$lib/cx";

  /**
   * Every keyboard shortcut, by where it works. One list for the About page
   * and the sheet `?` opens, so the two never tell different stories.
   */
  let { class: klass = "" }: { class?: string } = $props();
  const a = dict("about");

  const SHORTCUTS: { group: () => string; rows: { keys: string[]; label: () => string }[] }[] = [
    {
      group: () => $a.shortcutsGlobal.value,
      rows: [
        { keys: ["Ctrl", "K"], label: () => $a.shortcutPalette.value },
        { keys: ["Ctrl", "1…9"], label: () => $a.shortcutTab.value },
        { keys: ["Ctrl", "R"], label: () => $a.shortcutRefresh.value },
        { keys: ["Ctrl", "U"], label: () => $a.shortcutUpdate.value },
        { keys: ["Ctrl", "L"], label: () => $a.shortcutReconnect.value },
        { keys: ["?"], label: () => $a.shortcutSheet.value },
      ],
    },
    {
      group: () => $a.shortcutsServers.value,
      rows: [
        { keys: ["↑", "↓"], label: () => $a.shortcutNav.value },
        { keys: ["Enter"], label: () => $a.shortcutConnect.value },
        { keys: ["Dbl-click"], label: () => $a.shortcutDblclick.value },
        { keys: [$a.shortcutRightClick.value], label: () => $a.shortcutMenu.value },
        { keys: ["F"], label: () => $a.shortcutFav.value },
        { keys: ["I"], label: () => $a.shortcutInfo.value },
        { keys: ["P"], label: () => $a.shortcutPing.value },
        { keys: ["D"], label: () => $a.shortcutDirect.value },
        { keys: ["L"], label: () => $a.shortcutLink.value },
        { keys: ["Del"], label: () => $a.shortcutRemove.value },
        { keys: ["Esc"], label: () => $a.shortcutClose.value },
      ],
    },
    {
      group: () => $a.shortcutsMods.value,
      rows: [
        { keys: ["↑", "↓"], label: () => $a.shortcutNav.value },
        { keys: ["Space"], label: () => $a.shortcutSpace.value },
        { keys: ["M"], label: () => $a.shortcutManaged.value },
        { keys: ["U"], label: () => $a.shortcutModUpdate.value },
        { keys: ["Del"], label: () => $a.shortcutModDelete.value },
      ],
    },
    {
      group: () => $a.shortcutsNews.value,
      rows: [{ keys: ["↑", "↓", "J", "K"], label: () => $a.shortcutArticle.value }],
    },
  ];
</script>

<div class={cn("flex flex-col gap-3", klass)}>
  {#each SHORTCUTS as g, gi (gi)}
    <div>
      <p class="m-0 mb-1 font-mono text-3xs tracking-[0.08em] text-fg-faint uppercase">
        {g.group()}
      </p>
      <ul class="m-0 flex list-none flex-col p-0">
        {#each g.rows as r, ri (ri)}
          <li class="flex items-center gap-2 border-b border-border/40 py-1 last:border-b-0">
            <span class="flex-1 text-2xs text-fg-muted">{r.label()}</span>
            <span class="flex shrink-0 items-center gap-0.5">
              {#each r.keys as k, i (i)}{#if i > 0}<span class="text-3xs text-fg-faint"
                    >{r.keys[0] === "Ctrl" ? "+" : "/"}</span
                  >{/if}<Kbd>{k}</Kbd>{/each}
            </span>
          </li>
        {/each}
      </ul>
    </div>
  {/each}
</div>

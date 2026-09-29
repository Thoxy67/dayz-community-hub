# ui

The shadcn-svelte layout: one folder per component, the component in
`<name>.svelte`, and an `index.ts` that names what the folder offers.

```ts
import { Button } from "$lib/components/ui/button";
```

The layout, and not the registry. `components.json` is here so the paths are
written down, but **do not run `shadcn-svelte init` or `add` in this repo**.
The theme wipes Tailwind's stock palette, sizes and radii on purpose
(`styles/tokens.css`), so a registry component's `bg-background`, `text-sm`,
`rounded-md` and `ring-ring` produce no CSS and no error: it would arrive
invisible. Port by hand and re-class with this theme's tokens.

Rules that are not visible from the code:

- **Never pass `name` to a bits-ui primitive.** That is what makes Switch,
  Checkbox, RadioGroup and Select render a visually hidden input, and a
  clipped checkbox is what made WebKitGTK's web process abort on every flip.
  `Switch` and `Toggle` here are hand-made for that reason and stay so.
- **`tv` comes from `tailwind-variants/lite`**, never the package root, which
  brings a second copy of tailwind-merge.
- **Measures are tokens**: `h-control`, `h-row`, `px-pad`, `size-icon`. A pixel
  height in a component is a component that ignores the density setting.
- **Nothing here imports bits-ui if a pane uses it.** The two windows over the
  game get `PaneTip` and plain buttons; floating-ui stays out of their chunks.
- **No `backdrop-filter`**, and no animation that repeats without an end.

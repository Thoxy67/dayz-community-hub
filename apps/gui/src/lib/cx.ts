import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * `tailwind-merge`, told about this theme's own scales, so `h-control` over
 * `h-control-sm` replaces it instead of both landing and the stylesheet's
 * order deciding.
 */
const merge = extendTailwindMerge({
  extend: {
    theme: {
      spacing: [
        "control-sm",
        "control",
        "control-lg",
        "row",
        "titlebar",
        "rail",
        "rail-collapsed",
        "icon-sm",
        "icon",
        "icon-lg",
        "pad",
      ],
      radius: ["xs", "sm", "md", "lg", "full"],
      text: ["3xs", "2xs", "xs", "sm", "base", "lg", "xl", "2xl", "3xl"],
      shadow: ["pop", "glow"],
    },
    classGroups: {
      z: [{ z: ["raised", "popover", "dialog", "toast", "titlebar", "tip"] }],
    },
  },
});

/** Join classes; later ones override earlier ones. */
export function cn(...inputs: ClassValue[]) {
  return merge(clsx(inputs));
}

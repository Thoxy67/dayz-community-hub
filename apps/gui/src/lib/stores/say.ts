/**
 * Toasts in the current language, for stores and actions. Every message
 * passes through `String()` because intlayer hands back nodes that render in
 * a template but are objects elsewhere.
 */
import { toasts } from "$lib/components/ui/toast";
import { errorText } from "$lib/ipc/core";

type Node = unknown;

export const say = {
  info: (n: Node) => toasts.say(String(n), "neutral"),
  ok: (n: Node) => toasts.ok(String(n)),
  warn: (n: Node) => toasts.warn(String(n)),
  err: (n: Node) => toasts.err(String(n)),
};

export { errorText };

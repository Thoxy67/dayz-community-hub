/**
 * Where the server list comes from and how far each source has got: the
 * launcher list, Steam's master (Web API or client), the servers kept from
 * earlier lists. The backend sends it as `sources-changed`.
 */
import { events, inTauri } from "$lib/ipc/core";
import { sourcesStatus } from "$lib/ipc/servers";
import type { SourcesChanged } from "$lib/ipc/bindings";

class Sources {
  now = $state.raw<SourcesChanged | null>(null);
  #listening = false;

  async start() {
    try {
      this.now = await sourcesStatus();
    } catch {
      // The bar stays hidden.
    }
    if (inTauri && !this.#listening) {
      this.#listening = true;
      void events.sourcesChanged.listen((e) => (this.now = e.payload));
    }
  }
}

export const sources = new Sources();

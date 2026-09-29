/**
 * The dayz.com articles and their pictures. Pictures are downloaded once by
 * the backend into the app's cache folder and shown through Tauri's asset
 * protocol; the ones already on disk are resolved in one call when the list
 * arrives, the rest fetched six at a time.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { SvelteMap } from "svelte/reactivity";
import { errorText } from "$lib/ipc/core";
import { fetchNews } from "$lib/ipc/news";
import { fetchImage, resolveCachedImages } from "$lib/ipc/system";
import type { ArticleDto } from "$lib/ipc/types";

const MAX_CONCURRENT = 6;

class News {
  articles = $state.raw<ArticleDto[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  selected = $state(0);
  /** Remote image URL → local asset URL, once it is on disk. */
  images = new SvelteMap<string, string>();

  #active = 0;
  #queue: Array<() => void> = [];
  #flight = new Map<string, Promise<string>>();

  async load(force = false) {
    if (this.loading || (!force && this.articles.length > 0)) return;
    this.loading = true;
    this.error = null;
    try {
      this.articles = await fetchNews();
      this.selected = 0;
      void this.#warm();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  /** Everything already cached, in one call; then fetch the missing thumbnails. */
  async #warm() {
    const urls = this.articles.map((a) => a.image_url).filter((u): u is string => !!u);
    if (urls.length === 0) return;
    try {
      const cached = await resolveCachedImages(urls);
      for (const [url, path] of cached ?? []) if (path) this.images.set(url, convertFileSrc(path));
    } catch {
      // Fetched one by one below.
    }
    for (const u of urls) if (!this.images.has(u)) void this.image(u).catch(() => {});
  }

  /** A remote picture as a local URL, downloading it if needed. */
  image(url: string): Promise<string> {
    const hit = this.images.get(url);
    if (hit) return Promise.resolve(hit);
    let p = this.#flight.get(url);
    if (!p) {
      p = this.#slot()
        .then(() => fetchImage(url))
        .then((path) => {
          if (!path) throw new Error("no image");
          const src = convertFileSrc(path);
          this.images.set(url, src);
          return src;
        })
        .finally(() => {
          this.#active--;
          this.#flight.delete(url);
          this.#queue.shift()?.();
        });
      this.#flight.set(url, p);
    }
    return p;
  }

  #slot(): Promise<void> {
    if (this.#active < MAX_CONCURRENT) {
      this.#active++;
      return Promise.resolve();
    }
    return new Promise((r) =>
      this.#queue.push(() => {
        this.#active++;
        r();
      }),
    );
  }
}

export const news = new News();

/** Minutes to read, at 200 words a minute. */
export function readMinutes(a: ArticleDto): number {
  const words = (a.content_text || a.excerpt || "").split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.round(words / 200));
}

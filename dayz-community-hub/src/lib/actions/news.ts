import { invoke } from '@tauri-apps/api/core';
import type { ArticleDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';

export async function loadNews() {
  if (s.articles.length > 0) return;
  s.newsLoading = true;
  try {
    s.articles = await invoke<ArticleDto[]>('fetch_news');
  } catch (e) {
    s.setStatus(`News fetch failed: ${e}`, 'error');
  } finally {
    s.newsLoading = false;
  }
}

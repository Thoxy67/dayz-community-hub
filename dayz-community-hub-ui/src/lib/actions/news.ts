import { invoke } from '@tauri-apps/api/core';
import type { ArticleDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import * as m from '$lib/paraglide/messages.js';

export async function loadNews() {
  if (s.articles.length > 0) return;
  s.newsLoading = true;
  try {
    s.articles = await invoke<ArticleDto[]>('fetch_news');
  } catch (e) {
    s.setStatus(m.news_fetch_failed({ error: String(e) }), 'error');
  } finally {
    s.newsLoading = false;
  }
}

/** dayz.com articles. */
import { call } from "./core";
import type { ArticleDto } from "./types";

export const fetchNews = () => call<ArticleDto[]>("fetch_news");

//! The news view (= `features/news/View.svelte` and `news.svelte.ts`): the dayz.com
//! articles, a list on the left, the article being read on the right, the categories as
//! chips under the header.
//!
//! The articles load once per run, the first time the view shows; Refresh forces. The
//! list fetched last time (kept on disk by the hub) shows meanwhile, so the first visit
//! has articles at once and swaps the fresh list in when it comes, on the same article. The
//! list has no bodies: the article on screen fetches its own (and the next one's, ahead
//! of the reader), kept for the run. Each answer is read off the frame: the HTML is
//! parsed into an [`HtmlDoc`] (the body, and an update's patch notes per platform) and
//! the reading time worked out on the runtime, so drawing only lays out text.
//! When dayz.com turns the request away (Cloudflare, the hub's `blocked:` errors) the
//! reader says so and offers the site in the system browser (the Tauri WebView fallback
//! is not ported).
//!
//! Files: `images` (the picture cache), `list` (the left pane), `reader` (the article).
//! The view's state lives in one [`NewsState`] in egui's memory, so the commands of
//! `commands.rs` (keys, palette, pad, control channel) and the buttons change the same
//! thing.

mod images;
mod list;
mod reader;

use std::collections::HashMap;
use std::sync::Arc;

use dz_hub::Hub;
use dz_hub::features::news::{ArticleBodyDto, ArticleDto, BLOCKED_PREFIX};
use egui::mutex::Mutex;
use egui::{Id, Ui, UiBuilder, Vec2};
use serde_json::Value;

use crate::app::Ctx;
use crate::commands::{self, Scope};
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::{Button, IconButton, Side, Size, Variant};
use crate::parts::{Figure, HtmlDoc, PageHeader};
use crate::rt::Fetch;
use crate::store::Bus;
use crate::theme::TokensExt;
use crate::views::{View, ViewId};

pub use images::Images;

/// "Open every article on dayz.com".
pub const SITE: &str = "https://dayz.com/news";
/// How many articles' pictures are fetched with the list.
const WARM: usize = 10;
/// The list's width (`w-[22rem]`).
const LIST_W: f32 = 352.0;

/// One article as the view draws it: read once, when the list arrives.
pub struct Item {
    pub a: ArticleDto,
    /// Unix seconds (0 when the date cannot be read).
    pub ts: i64,
}

/// An article's body, read once, when it arrives.
pub struct Body {
    /// Sanitised; `None` when the article has no text.
    pub doc: Option<HtmlDoc>,
    /// Minutes to read, at 200 words a minute (at least 1).
    pub minutes: usize,
    /// An update's patch notes: the platform, and its notes.
    pub notes: Vec<(String, HtmlDoc)>,
}

impl Body {
    fn new(b: &ArticleBodyDto, base: &str, excerpt: Option<&str>) -> Self {
        let doc = HtmlDoc::parse(&b.html, base);
        let words = doc.words();
        let words = if words == 0 {
            excerpt.unwrap_or("").split_whitespace().count()
        } else {
            words
        };
        Self {
            minutes: read_minutes(words),
            doc: (!doc.blocks.is_empty()).then_some(doc),
            notes: b
                .notes
                .iter()
                .map(|n| (n.platform.clone(), HtmlDoc::parse(&n.html, base)))
                .filter(|(_, d)| !d.blocks.is_empty())
                .collect(),
        }
    }

    /// Every picture the body shows.
    fn images(&self) -> Vec<String> {
        self.doc.iter().flat_map(HtmlDoc::images).collect()
    }
}

/// The articles and their categories (in order of first appearance).
#[derive(Default)]
pub struct Feed {
    pub items: Vec<Item>,
    pub categories: Vec<String>,
}

impl Feed {
    fn new(articles: Vec<ArticleDto>) -> Self {
        let mut categories: Vec<String> = Vec::new();
        let items = articles
            .into_iter()
            .map(|a| {
                if let Some(c) = a.category.as_deref().filter(|c| !c.is_empty())
                    && !categories.iter().any(|x| x == c)
                {
                    categories.push(c.to_owned());
                }
                Item {
                    ts: timestamp(&a.date),
                    a,
                }
            })
            .collect();
        Self { items, categories }
    }

    /// The pictures to have at hand: the first page's heroes (dayz.com's are the
    /// full-size originals, a megabyte or more; the others come as their rows show).
    fn thumbs(&self) -> Vec<String> {
        self.items
            .iter()
            .take(WARM)
            .filter_map(|i| i.a.image_url.clone())
            .filter(|u| !u.is_empty())
            .collect()
    }

    /// Indices of the articles in `category` (all of them for `None`).
    pub fn shown(&self, category: Option<&str>) -> Vec<usize> {
        (0..self.items.len())
            .filter(|i| {
                category.is_none_or(|c| {
                    self.items
                        .get(*i)
                        .is_some_and(|x| x.a.category.as_deref() == Some(c))
                })
            })
            .collect()
    }
}

/// Minutes to read `words`, at 200 words a minute (`readMinutes`).
pub fn read_minutes(words: usize) -> usize {
    ((words as f64 / 200.0).round() as usize).max(1)
}

/// An article's date as Unix seconds: RFC 3339, or `YYYY-MM-DD` (midnight UTC, as
/// `new Date("2026-10-21")` reads it).
pub fn timestamp(d: &str) -> i64 {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(d.trim()) {
        return t.timestamp();
    }
    d.trim()
        .get(..10)
        .and_then(|p| chrono::NaiveDate::parse_from_str(p, "%Y-%m-%d").ok())
        .and_then(|n| n.and_hms_opt(0, 0, 0))
        .map_or(0, |n| n.and_utc().timestamp())
}

/// What the view remembers (for the run, as the Svelte store did).
#[derive(Default)]
pub struct NewsState {
    pub feed: Fetch<Arc<Feed>>,
    /// The list kept from the last run, read while the first fetch runs.
    kept: Fetch<Arc<Feed>>,
    /// The list shown is the kept one: the fetched list replaces it in place.
    from_kept: bool,
    /// The article on screen, as an index into the filtered list.
    pub selected: usize,
    pub category: Option<String>,
    /// The picture enlarged: what was shown, and the original to swap in once it is here.
    pub lightbox: Option<(String, String)>,
    pub images: Images,
    /// The bodies asked for, by slug.
    pub bodies: HashMap<String, Fetch<Arc<Body>>>,
    /// The patch notes shown (by platform); `None`: Steam's, else the first.
    pub platform: Option<String>,
    /// The article the reader shows, and when it came (egui time): its fade and the
    /// scroll back to the top.
    shown_slug: Option<String>,
    shown_at: f64,
    /// The first visit asked for the articles (later visits do not, even after a failure).
    started: bool,
}

impl NewsState {
    /// Load the articles: once, unless `force`; a load under way is not repeated.
    pub fn load(&mut self, hub: &Hub, force: bool) {
        if self.feed.loading() || (!force && self.feed.value.is_some()) {
            return;
        }
        self.feed.error = None;
        if self.feed.value.is_none() && !force {
            let hub = hub.clone();
            self.kept.load(async move {
                let articles = hub.cached_news().await.ok_or_else(String::new)?;
                tokio::task::spawn_blocking(move || Arc::new(Feed::new(articles)))
                    .await
                    .map_err(|e| e.to_string())
            });
        }
        let (hub, images) = (hub.clone(), self.images.clone());
        self.feed.load(async move {
            let articles = hub.fetch_news().await?;
            let feed = tokio::task::spawn_blocking(move || Feed::new(articles))
                .await
                .map_err(|e| e.to_string())?;
            let urls = feed.thumbs();
            crate::rt::fire(async move { images.warm(&hub, urls).await });
            Ok(Arc::new(feed))
        });
    }

    /// Take finished loads in; a new list starts at its first article, and its bodies
    /// are fetched again, unless it replaces the kept list: the same article stays on
    /// screen with the bodies already here. The article on screen and the next one have
    /// theirs asked for. Returns a failed fetch's error when a list is still shown
    /// (the reader tells a failure only with nothing to show).
    fn poll(&mut self, hub: &Hub) -> Option<String> {
        if self.kept.poll()
            && let Some(kept) = self.kept.value.take()
            && self.feed.value.is_none()
        {
            self.feed.value = Some(kept);
            self.from_kept = true;
            self.selected = 0;
        }
        let was = self
            .current()
            .and_then(|i| self.item(i))
            .map(|x| x.a.slug.clone());
        let mut failed = None;
        if self.feed.poll() {
            if self.feed.error.is_none() {
                if std::mem::take(&mut self.from_kept) {
                    let slugs: std::collections::HashSet<String> = self
                        .feed
                        .value
                        .iter()
                        .flat_map(|f| f.items.iter().map(|x| x.a.slug.clone()))
                        .collect();
                    self.bodies.retain(|slug, _| slugs.contains(slug));
                    self.selected = was
                        .and_then(|slug| {
                            self.shown()
                                .iter()
                                .position(|&i| self.item(i).is_some_and(|x| x.a.slug == slug))
                        })
                        .unwrap_or(0);
                } else {
                    self.selected = 0;
                    self.bodies.clear();
                }
            } else if self.feed.value.is_some() {
                failed.clone_from(&self.feed.error);
            }
        }
        for b in self.bodies.values_mut() {
            b.poll();
        }
        let shown = self.shown();
        let at = self.selected.min(shown.len().saturating_sub(1));
        for i in shown.into_iter().skip(at).take(2) {
            self.want_body(hub, i, false);
        }
        failed
    }

    /// Fetch article `i`'s body: once, unless `retry` and it failed.
    pub fn want_body(&mut self, hub: &Hub, i: usize, retry: bool) {
        let Some(a) = self.item(i).map(|x| x.a.clone()) else {
            return;
        };
        if let Some(b) = self.bodies.get(&a.slug)
            && (b.loading() || b.value.is_some() || (b.error.is_some() && !retry))
        {
            return;
        }
        let (hub, images) = (hub.clone(), self.images.clone());
        let slot = self.bodies.entry(a.slug.clone()).or_default();
        slot.error = None;
        slot.load(async move {
            let cat = a.category_code.clone().unwrap_or_else(|| "news".into());
            let dto = hub.fetch_article(cat, a.slug.clone()).await?;
            let body =
                tokio::task::spawn_blocking(move || Body::new(&dto, &a.url, a.excerpt.as_deref()))
                    .await
                    .map_err(|e| e.to_string())?;
            let urls = body.images();
            crate::rt::fire(async move { images.warm(&hub, urls).await });
            Ok(Arc::new(body))
        });
    }

    /// Article `i`'s body: what came, whether it is coming, what failed.
    pub fn body(&self, i: usize) -> Option<&Fetch<Arc<Body>>> {
        self.bodies.get(&self.item(i)?.a.slug)
    }

    pub fn loading(&self) -> bool {
        self.feed.loading()
    }

    /// The articles in the chosen category.
    pub fn shown(&self) -> Vec<usize> {
        self.feed
            .value
            .as_ref()
            .map(|f| f.shown(self.category.as_deref()))
            .unwrap_or_default()
    }

    /// The article on screen: an index into the feed.
    pub fn current(&self) -> Option<usize> {
        let shown = self.shown();
        shown.get(self.selected).or(shown.first()).copied()
    }

    pub fn item(&self, i: usize) -> Option<&Item> {
        self.feed.value.as_ref()?.items.get(i)
    }

    /// Pick a category (`None`: all) and start at its first article.
    pub fn set_category(&mut self, c: Option<String>) {
        self.category = c;
        self.selected = 0;
    }

    /// One article up or down the list, clamped at its ends.
    pub fn step(&mut self, down: bool) {
        let n = self.shown().len();
        if n == 0 {
            return;
        }
        let cur = self.selected.min(n - 1);
        self.selected = if down {
            (cur + 1).min(n - 1)
        } else {
            cur.saturating_sub(1)
        };
    }

    /// Enlarge a picture: `shown` at once, `original` once it is downloaded.
    pub fn zoom(&mut self, shown: String, original: String) {
        self.lightbox = Some((shown, original));
    }

    /// Whether the error is Cloudflare's: the site is to be opened in a browser.
    pub fn blocked(&self) -> bool {
        self.feed
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with(BLOCKED_PREFIX))
    }
}

type Shared = Arc<Mutex<NewsState>>;

/// The view's state, shared with its commands.
pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::unique("dzch.news.view"), || {
            Arc::new(Mutex::new(NewsState::default()))
        })
        .clone()
    })
}

fn with<R>(cx: &Ctx, f: impl FnOnce(&mut NewsState) -> R) -> R {
    let s = shared(cx.egui);
    let mut st = s.lock();
    f(&mut st)
}

/// Open an http(s) address in the system browser, off the frame.
pub fn open_url(bus: &Bus, url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    let (bus, url) = (bus.clone(), url.to_owned());
    crate::rt::fire(async move {
        if let Err(e) = crate::platform::deck::open(&url) {
            bus.err(e.to_string());
        }
    });
}

pub struct NewsView;

impl NewsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NewsView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for NewsView {
    fn on_show(&mut self, cx: &mut Ctx) {
        with(cx, |st| {
            if !std::mem::replace(&mut st.started, true) {
                st.load(cx.hub, false);
            }
        });
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run: Vec<(&'static str, Value)> = Vec::new();
        {
            let mut st = shared.lock();
            if let Some(e) = st.poll(cx.hub) {
                cx.stores.bus.err(e);
            }
            keys(ui.ctx(), cx, &mut st, &mut run);
            draw(ui, cx, &mut st, &mut run);
        }
        for (id, args) in run {
            if let Err(e) = cx.run(id, &args) {
                cx.stores.bus.err(e);
            }
        }
        lightbox(ui.ctx(), cx, &shared);
    }
}

/// The view's keys (registry scope `news`), while nothing else has them.
fn keys(ctx: &egui::Context, cx: &Ctx, st: &mut NewsState, run: &mut Vec<(&'static str, Value)>) {
    let overlay = cx.stores.dialogs.open()
        || egui::Popup::is_any_open(ctx)
        || ctx.memory(|m| m.top_modal_layer().is_some())
        || st.lightbox.is_some();
    if overlay {
        return;
    }
    let down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J));
    match commands::scoped(ctx, Scope::News) {
        Some("news.move") => st.step(down),
        Some("news.open") => run.push(("news.open", Value::Null)),
        _ => {}
    }
}

/// Header, then the list and the reader side by side.
fn draw(ui: &mut Ui, cx: &Ctx, st: &mut NewsState, run: &mut Vec<(&'static str, Value)>) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, egui::CornerRadius::ZERO, tk.panel);
    let mut v = ui.new_child(
        UiBuilder::new()
            .max_rect(full)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    v.spacing_mut().item_spacing = Vec2::ZERO;
    header(&mut v, st, run);
    let rest = v.available_rect_before_wrap();
    let list_w = LIST_W.min(rest.width() * 0.5);
    let list_rect = egui::Rect::from_min_size(rest.min, Vec2::new(list_w, rest.height().max(0.0)));
    let reader_rect = egui::Rect::from_min_max(
        egui::pos2(list_rect.right(), rest.top()),
        egui::pos2(rest.right(), rest.bottom().max(rest.top())),
    );
    let mut lui = v.new_child(
        UiBuilder::new()
            .max_rect(list_rect)
            .id_salt("news-list")
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    lui.set_clip_rect(list_rect.intersect(ui.clip_rect()));
    list::show(&mut lui, cx, st, run);
    let mut rui = v.new_child(
        UiBuilder::new()
            .max_rect(reader_rect)
            .id_salt("news-reader")
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    rui.set_clip_rect(reader_rect.intersect(ui.clip_rect()));
    reader::show(&mut rui, cx, st, run);
}

fn header(ui: &mut Ui, st: &NewsState, out: &mut Vec<(&'static str, Value)>) {
    let tk = ui.tokens();
    let run = std::cell::RefCell::new(Vec::<(&'static str, Value)>::new());
    let feed = st.feed.value.clone();
    let n = feed.as_ref().map_or(0, |f| f.items.len());
    let newest = feed
        .as_ref()
        .and_then(|f| f.items.first())
        .map(|i| (format::relative(i.ts), format::date_str(&i.a.date)));
    let loading = st.loading();
    let cats: Vec<String> = feed
        .as_ref()
        .map(|f| f.categories.clone())
        .unwrap_or_default();
    let category = st.category.clone();
    let toolbar = (cats.len() > 1).then_some(|ui: &mut Ui| {
        // `flex flex-wrap items-center gap-1.5`, the header's pixel under it.
        let w = ui.available_width();
        let layout = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(Vec2::new(w, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if ui
                .add(Button::chip(t("news", "allCategories"), category.is_none()))
                .clicked()
            {
                run.borrow_mut().push((
                    "news.filter_category",
                    serde_json::json!({ "category": null }),
                ));
            }
            for c in &cats {
                if ui
                    .add(Button::chip(c, category.as_deref() == Some(c.as_str())))
                    .clicked()
                {
                    run.borrow_mut()
                        .push(("news.filter_category", serde_json::json!({ "category": c })));
                }
            }
        });
        ui.add_space(1.0);
    });
    PageHeader::new(t("nav", "news")).show(
        ui,
        |ui| {
            ui.add(Figure::new(t("news", "statArticles")).amount(n as f64));
            if let Some((rel, date)) = &newest {
                ui.add(
                    Figure::new(t("news", "latest"))
                        .value(rel)
                        .tip(date)
                        .tone(tk.accent),
                );
            }
        },
        |ui| {
            if ui
                .add(
                    IconButton::new(lucide::EXTERNAL_LINK, t("news", "openAllOnSite"))
                        .size(Size::Icon)
                        .variant(Variant::Ghost)
                        .side(Side::Bottom),
                )
                .clicked()
            {
                run.borrow_mut().push(("news.open_site", Value::Null));
            }
            if ui
                .add(
                    Button::new(t("news", "refresh"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(loading)
                        .enabled(!loading),
                )
                .clicked()
            {
                run.borrow_mut().push(("news.refresh", Value::Null));
            }
        },
        toolbar,
    );
    out.extend(run.into_inner());
}

/// The enlarged picture over everything, while one is asked for.
fn lightbox(ctx: &egui::Context, cx: &Ctx, shared: &Shared) {
    let (shown, original, images) = {
        let st = shared.lock();
        let Some((s, o)) = st.lightbox.clone() else {
            return;
        };
        (s, o, st.images.clone())
    };
    let tex = images
        .texture(ctx, cx.hub, &original)
        .or_else(|| images.texture(ctx, cx.hub, &shown));
    let mut open = true;
    match tex {
        Some(tex) => {
            let src = egui::ImageSource::Texture(tex.sized());
            crate::parts::Lightbox::new(src, t("news", "close")).show(ctx, &mut open);
        }
        // Nothing to show (yet): Escape still closes; a picture that never comes closes.
        None => {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape))
                || (images.failed(&original) && images.failed(&shown))
            {
                open = false;
            }
        }
    }
    if !open {
        shared.lock().lightbox = None;
    }
}

// ── commands ────────────────────────────────────────────────────────────────

/// `news.refresh`: load the articles again.
pub fn cmd_refresh(cx: &mut Ctx, _: &Value) {
    with(cx, |st| st.load(cx.hub, true));
}

/// `news.open_site`: dayz.com's news page in the system browser.
pub fn cmd_open_site(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, SITE);
}

/// `news.filter_category {"category": "Updates" | null}`: one category, or all.
pub fn cmd_filter_category(cx: &mut Ctx, a: &Value) {
    let want = a.get("category").and_then(Value::as_str).map(str::to_owned);
    with(cx, |st| {
        let known = want.as_ref().is_none_or(|c| {
            st.feed
                .value
                .as_ref()
                .is_some_and(|f| f.categories.iter().any(|x| x == c))
        });
        if known {
            st.set_category(want);
        }
    });
    cx.go(ViewId::News);
}

/// `news.select {"index": n}` (in the list as filtered) or `{"slug": "…"}`: read it.
pub fn cmd_select(cx: &mut Ctx, a: &Value) {
    with(cx, |st| {
        if let Some(slug) = a.get("slug").and_then(Value::as_str) {
            let Some(feed) = st.feed.value.clone() else {
                return;
            };
            let find = |shown: &[usize]| {
                shown
                    .iter()
                    .position(|i| feed.items.get(*i).is_some_and(|x| x.a.slug == slug))
            };
            if let Some(p) = find(&st.shown()) {
                st.selected = p;
            } else if let Some(p) = find(&feed.shown(None)) {
                st.category = None;
                st.selected = p;
            }
        } else if let Some(i) = a.get("index").and_then(Value::as_u64) {
            let n = st.shown().len();
            if let Ok(i) = usize::try_from(i)
                && i < n
            {
                st.selected = i;
            }
        }
    });
    cx.go(ViewId::News);
}

/// `news.move {"dir": "up" | "down"}`: the previous or next article.
pub fn cmd_move(cx: &mut Ctx, a: &Value) {
    let down = a.get("dir").and_then(Value::as_str) != Some("up");
    with(cx, |st| st.step(down));
}

/// `news.open` / `news.open_article`: the article on dayz.com.
pub fn cmd_open_article(cx: &mut Ctx, _: &Value) {
    let url = with(cx, |st| {
        st.current()
            .and_then(|i| st.item(i))
            .map(|x| x.a.url.clone())
    });
    if let Some(url) = url {
        open_url(&cx.stores.bus, &url);
    }
}

fn has_article(cx: &Ctx) -> bool {
    with(cx, |st| {
        st.current()
            .and_then(|i| st.item(i))
            .is_some_and(|x| !x.a.url.is_empty())
    })
}

/// Enabled when an article with an address is on screen.
pub fn can_open_article(cx: &Ctx) -> bool {
    has_article(cx)
}

/// `news.open_link {"url": "https://…"}`: a link of the article, in the system browser.
pub fn cmd_open_link(cx: &mut Ctx, a: &Value) {
    if let Some(url) = a.get("url").and_then(Value::as_str) {
        open_url(&cx.stores.bus, url);
    }
}

/// `news.zoom_image {"src"?, "original"?}`: enlarge a picture (the hero by default).
pub fn cmd_zoom_image(cx: &mut Ctx, a: &Value) {
    let src = a.get("src").and_then(Value::as_str).map(str::to_owned);
    let original = a.get("original").and_then(Value::as_str).map(str::to_owned);
    with(cx, |st| {
        let hero = st
            .current()
            .and_then(|i| st.item(i))
            .and_then(|x| x.a.image_url.clone());
        if let Some(s) = src.or(hero).filter(|s| !s.is_empty()) {
            let o = original.unwrap_or_else(|| s.clone());
            st.zoom(s, o);
        }
    });
}

/// `news.retry_article`: fetch the body of the article on screen again.
pub fn cmd_retry_article(cx: &mut Ctx, _: &Value) {
    with(cx, |st| {
        if let Some(i) = st.current() {
            st.want_body(cx.hub, i, true);
        }
    });
}

/// `news.patch_notes {"platform": "Steam"}`: whose patch notes to show.
pub fn cmd_patch_notes(cx: &mut Ctx, a: &Value) {
    let p = a.get("platform").and_then(Value::as_str).map(str::to_owned);
    with(cx, |st| st.platform = p);
}

/// `news.close_lightbox`: put the picture away.
pub fn cmd_close_lightbox(cx: &mut Ctx, _: &Value) {
    with(cx, |st| st.lightbox = None);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn article(slug: &str, cat: Option<&str>) -> ArticleDto {
        ArticleDto {
            title: slug.into(),
            slug: slug.into(),
            excerpt: Some("one two three".into()),
            date: "2026-10-21T10:00:00Z".into(),
            url: "https://dayz.com/article/x".into(),
            image_url: None,
            category: cat.map(Into::into),
            category_code: None,
            author: None,
        }
    }

    #[test]
    fn categories_keep_their_first_order() {
        let f = Feed::new(vec![
            article("a", Some("Updates")),
            article("b", Some("Status Report")),
            article("c", Some("Updates")),
            article("d", None),
        ]);
        assert_eq!(f.categories, ["Updates", "Status Report"]);
        assert_eq!(f.shown(Some("Updates")), [0, 2]);
        assert_eq!(f.shown(None).len(), 4);
    }

    #[test]
    fn reading_time_and_dates() {
        assert_eq!(read_minutes(0), 1);
        assert_eq!(read_minutes(1000), 5);
        let long = format!("<p>{}</p>", "word ".repeat(1000));
        let b = ArticleBodyDto {
            html: long,
            notes: Vec::new(),
        };
        assert_eq!(Body::new(&b, "https://dayz.com/", None).minutes, 5);
        let empty = Body::new(&ArticleBodyDto::default(), "https://dayz.com/", Some("a b"));
        assert!(empty.doc.is_none());
        assert_eq!(empty.minutes, 1);
        assert_eq!(timestamp("1970-01-02"), 86_400);
        assert_eq!(timestamp("1970-01-01T00:01:00Z"), 60);
        assert_eq!(timestamp("garbage"), 0);
        assert_eq!(timestamp("é"), 0);
    }

    #[test]
    fn moving_is_clamped() {
        let mut st = NewsState::default();
        st.step(true);
        assert_eq!(st.selected, 0);
        st.feed.value = Some(Arc::new(Feed::new(vec![
            article("a", Some("U")),
            article("b", Some("S")),
        ])));
        st.step(true);
        st.step(true);
        assert_eq!(st.selected, 1);
        st.step(false);
        st.step(false);
        assert_eq!(st.selected, 0);
        st.set_category(Some("S".into()));
        assert_eq!(st.current(), Some(1));
    }
}

//! `SafeHtml`: somebody else's article in the app's type (`prose.css`). The HTML is read
//! once into [`HtmlDoc`] blocks, which is the sanitising: scripts, styles, frames, forms and
//! everything but text, headings, paragraphs, lists, quotes, code, rules, links and
//! pictures are dropped, and a link is kept only when it resolves to http(s). A YouTube
//! frame becomes its video's picture, a link to the video. Drawing it answers what was
//! clicked: a link to open in the system browser, a picture to enlarge. Blocks out of
//! sight are not laid out: their height from the last time they were is skipped.

use std::ops::Range;
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2,
};

use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// A run of text and how it is set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// Index into [`HtmlDoc::links`].
    pub link: Option<usize>,
}

/// One block of an article.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Level 1 to 3 (h4–h6 read as 3).
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    /// A list item: its marker (`None` for a bullet, the number in an ordered list) and its
    /// nesting depth (1 for a top-level list).
    Item(Option<u32>, u8, Vec<Span>),
    Quote(Vec<Span>),
    Pre(String),
    Image {
        src: String,
        full: String,
    },
    /// A YouTube video, by id.
    Video(String),
    Rule,
}

/// A YouTube video's picture.
pub fn video_thumb(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")
}

/// A YouTube video's page.
pub fn video_url(id: &str) -> String {
    format!("https://www.youtube.com/watch?v={id}")
}

/// The id of the video a YouTube embed address plays.
fn youtube_id(src: &str) -> Option<String> {
    let rest = src.trim().split_once("://").map_or(src, |(_, r)| r);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let id = rest
        .strip_prefix("youtube.com/embed/")
        .or_else(|| rest.strip_prefix("youtube-nocookie.com/embed/"))?;
    let id: String = id
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (6..=32).contains(&id.len()).then_some(id)
}

/// An article read and made safe, once (`HtmlDoc::parse`), drawn every frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HtmlDoc {
    pub blocks: Vec<Block>,
    /// Absolute http(s) links, by index.
    pub links: Vec<String>,
}

/// What a click on an article asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlAction {
    /// Open this http(s) address in the system browser.
    Open(String),
    /// Enlarge this picture (`shown` is its source, `original` the full-size one).
    Image { shown: String, original: String },
}

/// Elements whose whole content is dropped.
const DROP: &[&str] = &[
    "script", "style", "iframe", "object", "embed", "form", "noscript", "svg", "head", "title",
    "template", "select", "textarea", "button", "canvas", "video", "audio",
];

/// Elements without content.
const VOID: &[&str] = &[
    "br", "img", "hr", "link", "meta", "base", "input", "source", "col", "wbr", "area", "track",
];

/// `href` resolved against `base`, kept only when it is http(s).
pub fn resolve_url(raw: &str, base: &str) -> Option<String> {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(raw.to_owned());
    }
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let (scheme, rest) = base.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    if let Some(r) = raw.strip_prefix("//") {
        return Some(format!("{scheme}://{r}"));
    }
    // Another scheme (javascript:, mailto:, data:): a colon before any slash.
    let first_slash = raw.find('/').unwrap_or(raw.len());
    if raw
        .get(..first_slash)
        .is_some_and(|head| head.contains(':'))
    {
        return None;
    }
    let host = rest.split('/').next().unwrap_or(rest);
    if raw.starts_with('/') {
        return Some(format!("{scheme}://{host}{raw}"));
    }
    // Relative: against the base's directory.
    let path = rest.get(host.len()..).unwrap_or("");
    let dir = match path.rfind('/') {
        Some(i) => path.get(..=i).unwrap_or("/"),
        None => "/",
    };
    Some(format!("{scheme}://{host}{dir}{raw}"))
}

fn decode_entities(s: &str, out: &mut String) {
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(rest.get(..i).unwrap_or(""));
        let after = rest.get(i + 1..).unwrap_or("");
        let end = after.find(';').filter(|e| *e <= 10);
        let Some(end) = end else {
            out.push('&');
            rest = after;
            continue;
        };
        let name = after.get(..end).unwrap_or("");
        let c = if let Some(num) = name.strip_prefix('#') {
            let v = match num.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok(),
                None => num.parse().ok(),
            };
            v.and_then(char::from_u32)
        } else {
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "rsquo" => Some('’'),
                "lsquo" => Some('‘'),
                "rdquo" => Some('”'),
                "ldquo" => Some('“'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "hellip" => Some('…'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "trade" => Some('™'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                "eacute" => Some('é'),
                "egrave" => Some('è'),
                "agrave" => Some('à'),
                "ccedil" => Some('ç'),
                "uuml" => Some('ü'),
                "ouml" => Some('ö'),
                "auml" => Some('ä'),
                "szlig" => Some('ß'),
                "bull" => Some('•'),
                "middot" => Some('·'),
                "times" => Some('×'),
                "deg" => Some('°'),
                _ => None,
            }
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = after.get(end + 1..).unwrap_or("");
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
}

/// The attributes of a tag's inside (`a href="…" class=x`), lower-cased names.
fn attrs(inside: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = inside.as_bytes();
    let mut i = 0;
    // Skip the tag name.
    while i < b.len() && !b[i].is_ascii_whitespace() {
        i += 1;
    }
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let ns = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        let name = inside.get(ns..i).unwrap_or("").to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                decode_entities(inside.get(vs..i).unwrap_or(""), &mut value);
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                decode_entities(inside.get(vs..i).unwrap_or(""), &mut value);
            }
        }
        if !name.is_empty() {
            out.push((name, value));
        }
    }
    out
}

#[derive(Default)]
struct Builder {
    doc: HtmlDoc,
    spans: Vec<Span>,
    kind: Option<Block>,
    bold: u32,
    italic: u32,
    code: u32,
    link: Vec<Option<usize>>,
    lists: Vec<Option<u32>>,
    quote: u32,
    pre: Option<String>,
    heading: Option<u8>,
}

impl Builder {
    fn text(&mut self, raw: &str) {
        if let Some(p) = &mut self.pre {
            decode_entities(raw, p);
            return;
        }
        let mut decoded = String::new();
        decode_entities(raw, &mut decoded);
        // Collapse white space, as HTML does.
        let mut t = String::with_capacity(decoded.len());
        let mut space = self
            .spans
            .last()
            .is_none_or(|s| s.text.ends_with(' ') || s.text.ends_with('\n'));
        for c in decoded.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                if !space {
                    t.push(' ');
                    space = true;
                }
            } else {
                t.push(c);
                space = false;
            }
        }
        if t.is_empty() {
            return;
        }
        if self.heading.is_some() {
            t = t.to_uppercase();
        }
        let link = self.link.last().copied().flatten();
        let (bold, italic, code) = (self.bold > 0, self.italic > 0, self.code > 0);
        if let Some(last) = self.spans.last_mut()
            && last.bold == bold
            && last.italic == italic
            && last.code == code
            && last.link == link
        {
            last.text.push_str(&t);
            return;
        }
        self.spans.push(Span {
            text: t,
            bold,
            italic,
            code,
            link,
        });
    }

    fn line_break(&mut self) {
        if let Some(p) = &mut self.pre {
            p.push('\n');
        } else if let Some(last) = self.spans.last_mut() {
            let trimmed = last.text.trim_end_matches(' ').len();
            last.text.truncate(trimmed);
            last.text.push('\n');
        }
    }

    /// Ends the block being filled, if it has words.
    fn flush(&mut self) {
        let mut spans = std::mem::take(&mut self.spans);
        if let Some(last) = spans.last_mut() {
            let t = last.text.trim_end().len();
            last.text.truncate(t);
        }
        if let Some(first) = spans.first_mut() {
            let t = first.text.trim_start().to_owned();
            first.text = t;
        }
        spans.retain(|s| !s.text.is_empty());
        if spans.is_empty() {
            // Nothing written yet: the block keeps its kind (`<li><p>…`).
            return;
        }
        let kind = self.kind.take();
        let block = match kind {
            Some(Block::Heading(l, _)) => Block::Heading(l, spans),
            Some(Block::Item(m, d, _)) => Block::Item(m, d, spans),
            _ if self.quote > 0 => Block::Quote(spans),
            _ => Block::Para(spans),
        };
        self.doc.blocks.push(block);
    }

    fn open(&mut self, name: &str, inside: &str, base: &str) {
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                let l = match name {
                    "h1" => 1,
                    "h2" => 2,
                    _ => 3,
                };
                self.heading = Some(l);
                self.kind = Some(Block::Heading(l, Vec::new()));
            }
            "p" | "div" | "section" | "article" | "header" | "footer" | "figure" | "figcaption"
            | "tr" | "table" | "dl" | "dt" | "dd" | "main" | "aside" | "nav" => {
                self.flush();
            }
            "td" | "th" => {
                if !self.spans.is_empty() {
                    self.text(" · ");
                }
            }
            "br" => self.line_break(),
            "ul" => {
                self.flush();
                self.lists.push(None);
            }
            "ol" => {
                self.flush();
                self.lists.push(Some(0));
            }
            "li" => {
                self.flush();
                let depth = u8::try_from(self.lists.len().max(1)).unwrap_or(u8::MAX);
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        Some(*n)
                    }
                    _ => None,
                };
                self.kind = Some(Block::Item(marker, depth, Vec::new()));
            }
            "blockquote" => {
                self.flush();
                self.quote += 1;
            }
            "pre" => {
                self.flush();
                self.pre = Some(String::new());
            }
            "hr" => {
                self.flush();
                self.doc.blocks.push(Block::Rule);
            }
            "strong" | "b" => self.bold += 1,
            "em" | "i" => self.italic += 1,
            "code" | "kbd" | "samp" => self.code += 1,
            "a" => {
                let href = attrs(inside)
                    .into_iter()
                    .find(|(n, _)| n == "href")
                    .and_then(|(_, v)| resolve_url(&v, base));
                let idx = href.map(|h| {
                    self.doc.links.push(h);
                    self.doc.links.len() - 1
                });
                self.link.push(idx);
            }
            "img" => {
                let a = attrs(inside);
                let get = |k: &str| a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
                let src = get("src").or_else(|| get("data-src")).unwrap_or("");
                if let Some(src) = resolve_url(src, base) {
                    let full = get("data-full")
                        .and_then(|f| resolve_url(f, base))
                        .unwrap_or_else(|| src.clone());
                    self.flush();
                    self.doc.blocks.push(Block::Image { src, full });
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, name: &str) {
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                self.heading = None;
                self.kind = None;
            }
            "p" | "div" | "section" | "article" | "header" | "footer" | "figure" | "figcaption"
            | "tr" | "table" | "dl" | "dt" | "dd" | "main" | "aside" | "nav" => self.flush(),
            "li" => {
                self.flush();
                self.kind = None;
            }
            "ul" | "ol" => {
                self.flush();
                self.lists.pop();
            }
            "blockquote" => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
            }
            "pre" => {
                if let Some(p) = self.pre.take() {
                    let p = p.trim_matches('\n').to_owned();
                    if !p.is_empty() {
                        self.doc.blocks.push(Block::Pre(p));
                    }
                }
            }
            "strong" | "b" => self.bold = self.bold.saturating_sub(1),
            "em" | "i" => self.italic = self.italic.saturating_sub(1),
            "code" | "kbd" | "samp" => self.code = self.code.saturating_sub(1),
            "a" => {
                self.link.pop();
            }
            _ => {}
        }
    }
}

impl HtmlDoc {
    /// Reads `html`, relative links resolved against `base` (the page it came from).
    pub fn parse(html: &str, base: &str) -> Self {
        let mut b = Builder::default();
        let mut rest = html;
        let mut drop_until: Option<String> = None;
        while !rest.is_empty() {
            let Some(lt) = rest.find('<') else {
                if drop_until.is_none() {
                    b.text(rest);
                }
                break;
            };
            if lt > 0 && drop_until.is_none() {
                b.text(rest.get(..lt).unwrap_or(""));
            }
            let after = rest.get(lt..).unwrap_or("");
            if let Some(c) = after.strip_prefix("<!--") {
                rest = c.find("-->").and_then(|e| c.get(e + 3..)).unwrap_or("");
                continue;
            }
            let Some(gt) = after.find('>') else {
                break;
            };
            let inside = after.get(1..gt).unwrap_or("");
            rest = after.get(gt + 1..).unwrap_or("");
            if inside.starts_with('!') || inside.starts_with('?') {
                continue;
            }
            let closing = inside.starts_with('/');
            let body = inside.trim_start_matches('/');
            let name: String = body
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            if let Some(d) = &drop_until {
                if closing && *d == name {
                    drop_until = None;
                }
                continue;
            }
            if closing {
                b.close(&name);
            } else if DROP.contains(&name.as_str()) {
                if name == "iframe" {
                    let a = attrs(body);
                    if let Some(id) = a
                        .iter()
                        .find(|(n, _)| n == "src")
                        .and_then(|(_, v)| youtube_id(v))
                    {
                        b.flush();
                        b.doc.blocks.push(Block::Video(id));
                    }
                }
                if !inside.ends_with('/') {
                    drop_until = Some(name);
                }
            } else {
                b.open(&name, body, base);
                if !VOID.contains(&name.as_str()) && inside.ends_with('/') {
                    b.close(&name);
                }
            }
        }
        if let Some(p) = b.pre.take()
            && !p.trim().is_empty()
        {
            b.doc.blocks.push(Block::Pre(p));
        }
        b.flush();
        b.doc
    }

    /// Every picture's source, for the caller to fetch and cache before they are drawn
    /// (a video's picture included).
    pub fn images(&self) -> impl Iterator<Item = String> + '_ {
        self.blocks.iter().filter_map(|b| match b {
            Block::Image { src, .. } => Some(src.clone()),
            Block::Video(id) => Some(video_thumb(id)),
            _ => None,
        })
    }

    /// How many words the text has.
    pub fn words(&self) -> usize {
        // Over the block's whole text: a word can run across spans (`<b>rel</b>.`).
        let count = |spans: &[Span]| {
            spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
                .split_whitespace()
                .count()
        };
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Heading(_, s) | Block::Para(s) | Block::Item(_, _, s) | Block::Quote(s) => {
                    count(s)
                }
                Block::Pre(t) => t.split_whitespace().count(),
                _ => 0,
            })
            .sum()
    }
}

// ---------------------------------------------------------------------------------------

/// The body's measure: `text-sm`, leading 1.7.
const BODY: f32 = size::TEXT_SM;

fn job(
    spans: &[Span],
    base: FontId,
    ink: Color32,
    t: &Tokens,
    width: f32,
    lh: f32,
) -> (LayoutJob, Vec<(Range<usize>, usize)>) {
    let mut j = LayoutJob::default();
    let mut links = Vec::new();
    let mut at = 0usize;
    for s in spans {
        let mut fmt = TextFormat {
            font_id: base.clone(),
            color: ink,
            line_height: Some(lh),
            ..Default::default()
        };
        if s.bold {
            fmt.font_id = font(Face::SansSemibold, base.size);
            fmt.color = t.fg;
        }
        if s.italic {
            fmt.italics = true;
        }
        if s.code {
            fmt.font_id = font(Face::Mono, size::TEXT_XS);
            fmt.background = t.plot;
        }
        if s.link.is_some() {
            fmt.color = t.accent;
            fmt.underline = Stroke::new(1.0, t.accent);
        }
        let n = s.text.chars().count();
        if let Some(l) = s.link {
            links.push((at..at + n, l));
        }
        at += n;
        j.append(&s.text, 0.0, fmt);
    }
    j.wrap.max_width = width.max(20.0);
    (j, links)
}

/// `SafeHtml`: an [`HtmlDoc`] drawn in the app's type, the width on offer. `resolve` maps a
/// picture's source to what egui loads (a cached file's `file://` URI, say); a picture it
/// cannot resolve yet is not drawn. Answers what a click asked for.
pub struct SafeHtml<'a> {
    doc: &'a HtmlDoc,
    zoomable: bool,
}

impl<'a> SafeHtml<'a> {
    pub fn new(doc: &'a HtmlDoc) -> Self {
        Self {
            doc,
            zoomable: false,
        }
    }

    /// Pictures can be clicked to enlarge them (`onimage`).
    pub fn zoomable(mut self) -> Self {
        self.zoomable = true;
        self
    }

    #[allow(clippy::too_many_lines)]
    pub fn show(
        self,
        ui: &mut Ui,
        resolve: impl Fn(&str) -> Option<egui::ImageSource<'static>>,
    ) -> Option<HtmlAction> {
        let tk = ui.tokens();
        let w = ui.available_width();
        let mut out = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut prev_gap = 0.0_f32;
        let mut first = true;
        let mut after_list = false;
        // Each block's height at this width, the last time it was laid out.
        let key = ui.id().with((
            "html-heights",
            std::ptr::from_ref(self.doc) as usize,
            w.round() as i32,
        ));
        let mut heights: Vec<f32> = ui
            .data(|d| d.get_temp::<Arc<Vec<f32>>>(key))
            .map(|h| h.as_ref().clone())
            .unwrap_or_default();
        heights.resize(self.doc.blocks.len(), 0.0);
        let clip = ui.clip_rect();
        for (bi, block) in self.doc.blocks.iter().enumerate() {
            // CSS margins collapse: the larger of the two between blocks.
            let (top, bottom) = match block {
                Block::Heading(l, _) => {
                    let s = heading_size(*l);
                    (1.6 * s, 0.5 * s)
                }
                Block::Para(_) | Block::Pre(_) => (0.0, BODY),
                Block::Item(..) => (0.25 * BODY, 0.25 * BODY),
                Block::Quote(_) => (BODY, BODY),
                Block::Image { .. } | Block::Video(_) => (1.2 * BODY, 1.2 * BODY),
                Block::Rule => (2.0 * BODY, 2.0 * BODY),
            };
            // A list ends with its own margin (`ul { margin-bottom }`).
            let is_item = matches!(block, Block::Item(..));
            let top = if after_list && !is_item {
                top.max(BODY)
            } else {
                top
            };
            after_list = is_item;
            if !first {
                ui.add_space(prev_gap.max(top));
            }
            first = false;
            prev_gap = bottom;
            let start = ui.cursor().top();
            let known = heights.get(bi).copied().unwrap_or(0.0);
            if known > 0.0 && (start > clip.bottom() || start + known < clip.top()) {
                ui.add_space(known);
                continue;
            }
            let id = ui.id().with(("html-block", bi));
            match block {
                Block::Heading(l, spans) => {
                    let s = heading_size(*l);
                    let (mut j, links) =
                        job(spans, font(Face::DisplayBold, s), tk.fg, &tk, w, s * 1.05);
                    for sec in &mut j.sections {
                        sec.format.extra_letter_spacing = s * 0.02;
                        sec.format.font_id = font(Face::DisplayBold, s);
                    }
                    if let Some(a) = self.text(ui, j, &links, id) {
                        out = Some(a);
                    }
                }
                Block::Para(spans) => {
                    let (j, links) = job(
                        spans,
                        font(Face::Sans, BODY),
                        tk.fg_muted,
                        &tk,
                        w,
                        BODY * 1.7,
                    );
                    if let Some(a) = self.text(ui, j, &links, id) {
                        out = Some(a);
                    }
                }
                Block::Item(marker, depth, spans) => {
                    let indent = 18.0 * f32::from(*depth);
                    let (j, links) = job(
                        spans,
                        font(Face::Sans, BODY),
                        tk.fg_muted,
                        &tk,
                        w - indent,
                        BODY * 1.7,
                    );
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let (mr, _) =
                            ui.allocate_exact_size(Vec2::new(indent, BODY * 1.7), Sense::hover());
                        match marker {
                            Some(n) => {
                                let g =
                                    super::text::ph(ui, &format!("{n}."), font(Face::Sans, BODY));
                                ui.painter().galley(
                                    Pos2::new(
                                        mr.right() - 6.0 - g.size().x,
                                        mr.center().y - g.size().y / 2.0,
                                    ),
                                    g,
                                    tk.accent,
                                );
                            }
                            // `disc`: a round mark at the middle of the first line.
                            None => {
                                ui.painter().circle_filled(
                                    Pos2::new(mr.right() - 9.0, mr.center().y),
                                    2.5,
                                    tk.accent,
                                );
                            }
                        }
                        if let Some(a) = self.text(ui, j, &links, id) {
                            out = Some(a);
                        }
                    });
                }
                Block::Quote(spans) => {
                    let (j, links) = job(
                        spans,
                        font(Face::Sans, BODY),
                        tk.fg,
                        &tk,
                        w - 2.0 - 26.0,
                        BODY * 1.7,
                    );
                    let slot = ui.painter().add(egui::Shape::Noop);
                    let r = egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 15,
                            right: 13,
                            top: 6,
                            bottom: 6,
                        })
                        .show(ui, |ui| {
                            ui.set_width(w - 28.0);
                            self.text(ui, j, &links, id)
                        });
                    if let Some(a) = r.inner {
                        out = Some(a);
                    }
                    let rect = r.response.rect;
                    ui.painter().set(
                        slot,
                        egui::Shape::rect_filled(
                            rect,
                            CornerRadius::ZERO,
                            crate::kit::paint::alpha(tk.raised, 0.5),
                        ),
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                        CornerRadius::ZERO,
                        tk.accent,
                    );
                }
                Block::Pre(text) => {
                    let g = ui.painter().layout(
                        text.clone(),
                        font(Face::Mono, size::TEXT_XS),
                        tk.fg_muted,
                        w - 16.0,
                    );
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(w, g.size().y + 12.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.plot);
                    ui.painter()
                        .galley(r.min + Vec2::new(8.0, 6.0), g, tk.fg_muted);
                }
                Block::Image { src, full } => {
                    if let Some(source) = resolve(src) {
                        let img = egui::Image::new(source)
                            .max_width(w)
                            .maintain_aspect_ratio(true)
                            .corner_radius(CornerRadius::same(size::RADIUS_MD))
                            .sense(Sense::click());
                        let r = ui.add(img);
                        ui.painter().rect_stroke(
                            r.rect,
                            CornerRadius::same(size::RADIUS_MD),
                            Stroke::new(1.0, tk.border),
                            StrokeKind::Inside,
                        );
                        if self.zoomable {
                            let r = r.on_hover_cursor(egui::CursorIcon::ZoomIn);
                            if r.clicked() {
                                out = Some(HtmlAction::Image {
                                    shown: src.clone(),
                                    original: full.clone(),
                                });
                            }
                        }
                    }
                }
                Block::Video(vid) => {
                    if let Some(a) = video(ui, &tk, w, vid, resolve(&video_thumb(vid))) {
                        out = Some(a);
                    }
                }
                Block::Rule => {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
                    ui.painter()
                        .hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, tk.border));
                }
            }
            if let Some(h) = heights.get_mut(bi) {
                *h = (ui.cursor().top() - start).max(0.0);
            }
        }
        ui.data_mut(|d| d.insert_temp(key, Arc::new(heights)));
        out
    }

    /// Lays out and draws one block's text; a click on a link answers it.
    fn text(
        &self,
        ui: &mut Ui,
        j: LayoutJob,
        links: &[(Range<usize>, usize)],
        id: egui::Id,
    ) -> Option<HtmlAction> {
        let g = ui.painter().layout_job(j);
        let sense = if links.is_empty() {
            Sense::hover()
        } else {
            Sense::click()
        };
        // A selectable label: the article's words can be selected and copied.
        let resp = ui.add(
            egui::Label::new(egui::WidgetText::Galley(g.clone()))
                .selectable(true)
                .sense(sense),
        );
        let rect = resp.rect;
        let _ = id;
        let link_at = |pos: Pos2| {
            let c = g.cursor_from_pos(pos - rect.min);
            links
                .iter()
                .find(|(r, _)| r.contains(&c.index.0))
                .and_then(|(_, l)| self.doc.links.get(*l))
        };
        let mut out = None;
        if let Some(pos) = resp.hover_pos()
            && let Some(url) = link_at(pos)
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                out = Some(HtmlAction::Open(url.clone()));
            }
        }
        out
    }
}

/// A video as its picture (16:9, the letterbox of YouTube's 4:3 picture cut off), a play
/// mark over it; a click opens it on YouTube.
fn video(
    ui: &mut Ui,
    tk: &Tokens,
    w: f32,
    id: &str,
    thumb: Option<egui::ImageSource<'static>>,
) -> Option<HtmlAction> {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, w * 9.0 / 16.0), Sense::click());
    let radius = CornerRadius::same(size::RADIUS_MD);
    ui.painter().rect_filled(r, radius, tk.plot);
    if let Some(src) = thumb {
        egui::Image::new(src)
            .uv(Rect::from_min_max(
                Pos2::new(0.0, 0.125),
                Pos2::new(1.0, 0.875),
            ))
            .corner_radius(radius)
            .paint_at(ui, r);
    }
    let hover = resp.hovered();
    ui.painter().rect_filled(
        r,
        radius,
        crate::kit::paint::alpha(tk.scrim, if hover { 0.15 } else { 0.3 }),
    );
    let c = r.center();
    ui.painter()
        .circle_filled(c, if hover { 30.0 } else { 28.0 }, tk.accent);
    crate::icons::lucide::PLAY.paint(
        ui.painter(),
        Rect::from_center_size(c + Vec2::new(2.0, 0.0), Vec2::splat(24.0)),
        tk.accent_fg,
    );
    ui.painter()
        .rect_stroke(r, radius, Stroke::new(1.0, tk.border), StrokeKind::Inside);
    let resp = crate::kit::TipExt::tip(
        resp.on_hover_cursor(egui::CursorIcon::PointingHand),
        crate::i18n::t("news", "watchVideo"),
    );
    resp.clicked().then(|| HtmlAction::Open(video_url(id)))
}

fn heading_size(l: u8) -> f32 {
    match l {
        1 => size::TEXT_2XL,
        2 => size::TEXT_XL,
        _ => size::TEXT_LG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://dayz.com/article/news/x";

    #[test]
    fn drops_what_is_unsafe() {
        let d = HtmlDoc::parse(
            r#"<p onclick="x()">Hi <b>there</b><script>alert(1)</script></p><style>p{}</style><a href="javascript:alert(1)">no</a><iframe src="x">y</iframe>"#,
            BASE,
        );
        assert_eq!(d.blocks.len(), 2, "{d:?}");
        assert!(d.links.is_empty());
        assert!(
            matches!(&d.blocks[0], Block::Para(s) if s[0].text == "Hi " && s[1].bold && s[1].text == "there"),
            "{d:?}"
        );
    }

    #[test]
    fn resolves_links_and_pictures() {
        let d = HtmlDoc::parse(
            r#"<h2>Update &amp; more</h2><p>See <a href="/news">news</a> and <a href='rel'>rel</a>.</p><img src="//cdn.x/a.png" data-full="/b.png"><ul><li>one</li><li>two</li></ul><ol><li>first</li></ol><hr>"#,
            BASE,
        );
        assert_eq!(
            d.links,
            ["https://dayz.com/news", "https://dayz.com/article/news/rel"]
        );
        assert!(matches!(&d.blocks[0], Block::Heading(2, s) if s[0].text == "UPDATE & MORE"));
        assert!(
            matches!(&d.blocks[2], Block::Image { src, full } if src == "https://cdn.x/a.png" && full == "https://dayz.com/b.png")
        );
        assert!(matches!(&d.blocks[3], Block::Item(None, 1, _)));
        assert!(matches!(&d.blocks[5], Block::Item(Some(1), 1, _)));
        assert!(matches!(d.blocks.last(), Some(Block::Rule)));
        assert_eq!(d.images().count(), 1);
        assert_eq!(d.words(), 10);
    }

    #[test]
    fn paragraphs_in_list_items_keep_the_item() {
        let d = HtmlDoc::parse(
            "<ul><li><p>one</p></li><li><p>two</p></li></ul><p>after</p><h2></h2><p>x</p>",
            BASE,
        );
        assert!(
            matches!(&d.blocks[0], Block::Item(None, 1, s) if s[0].text == "one"),
            "{d:?}"
        );
        assert!(
            matches!(&d.blocks[1], Block::Item(None, 1, s) if s[0].text == "two"),
            "{d:?}"
        );
        assert!(
            matches!(&d.blocks[2], Block::Para(s) if s[0].text == "after"),
            "{d:?}"
        );
        assert!(
            matches!(&d.blocks[3], Block::Para(s) if s[0].text == "x"),
            "{d:?}"
        );
    }

    #[test]
    fn a_youtube_frame_is_its_video() {
        let d = HtmlDoc::parse(
            r#"<p>a</p><iframe src="https://www.youtube-nocookie.com/embed/BnMQS-jrkwI?x=1" allowfullscreen="true"></iframe><iframe src="https://evil.example/embed/x"></iframe><p>b</p>"#,
            BASE,
        );
        assert_eq!(d.blocks.len(), 3, "{d:?}");
        assert_eq!(d.blocks[1], Block::Video("BnMQS-jrkwI".into()));
        assert_eq!(
            d.images().collect::<Vec<_>>(),
            ["https://i.ytimg.com/vi/BnMQS-jrkwI/hqdefault.jpg"]
        );
        assert_eq!(youtube_id("https://youtube.com/embed/"), None);
    }

    #[test]
    fn hostile_input_does_not_panic() {
        for s in [
            "<",
            "<a",
            "&#xFFFFFFFF;",
            "<p>&",
            "<!--",
            "<b><i>x</b>",
            "</p></p>",
            "<img src=>",
            "é<é>",
        ] {
            let _ = HtmlDoc::parse(s, BASE);
        }
        assert_eq!(resolve_url("mailto:a@b", BASE), None);
        assert_eq!(resolve_url("#top", BASE), None);
    }
}

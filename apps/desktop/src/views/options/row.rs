//! One launch option (= `OptionRow.svelte`): what it is, the flag it becomes, its value
//! when it takes one, and whether it is on. A value is written when the field is left or
//! Enter is pressed (Esc puts the saved one back), and writing one turns the option on.

use std::sync::Arc;

use dz_hub::features::profile::LaunchOptionDto;
use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, FontId, Galley, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui,
    UiBuilder, Vec2,
};

use super::Act;
use super::catalog::{self, ValueKind};
use crate::i18n::t;
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{FieldSize, IconButton, Input, InputKind, MiniSwitch, Size, TipExt, Variant};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What is typed in a value field, reset whenever the saved value changes.
#[derive(Default)]
pub struct Draft {
    pub text: String,
    base: Option<Option<String>>,
}

impl Draft {
    /// Follow the saved value when it changed under the field.
    fn sync(&mut self, saved: &Option<String>) {
        if self.base.as_ref() != Some(saved) {
            self.text = saved.clone().unwrap_or_default();
            self.base = Some(saved.clone());
        }
    }
}

const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 8.0;
const GAP: f32 = 12.0;
const ICON_COL: f32 = 20.0;
const SWITCH_W: f32 = 28.0;
const LABEL_H: f32 = 17.0;
const LINE_H: f32 = 16.0;
const CODE_H: f32 = 16.0;

/// The `-maxMem` / `-window` chip's galley.
fn mono(ui: &Ui, text: &str, px: f32) -> Arc<Galley> {
    paint::galley(ui, text, font(Face::Mono, px), Color32::PLACEHOLDER)
}

/// The recommendation's chip, or the tick when the value already matches.
enum Rec<'a> {
    None,
    Matches,
    Offer(&'a str, Arc<Galley>),
}

impl Rec<'_> {
    fn width(&self) -> f32 {
        match self {
            Rec::None => 0.0,
            Rec::Matches => size::ICON_SM,
            Rec::Offer(_, g) => 6.0 + 12.0 + 4.0 + g.size().x + 6.0 + 2.0,
        }
    }
}

/// Draw one option; what the player did goes into `acts`.
pub fn show(
    ui: &mut Ui,
    opt: &LaunchOptionDto,
    tone: Color32,
    recommended: Option<&str>,
    draft: &mut Draft,
    acts: &mut Vec<Act>,
) {
    let tk = ui.tokens();
    draft.sync(&opt.value);
    let meta = catalog::meta(&opt.key);
    let icon = meta.map_or(lucide::SLIDERS_HORIZONTAL, |m| m.icon);
    let kind = meta.map_or(
        if opt.value.is_some() {
            ValueKind::Text
        } else {
            ValueKind::None
        },
        |m| m.value,
    );
    let label = catalog::label(&opt.key);
    let desc = meta.map_or(opt.description.as_str(), |m| t("options", m.desc));
    let saved = opt.value.as_deref().unwrap_or("");
    let threads = if kind == ValueKind::Threads {
        catalog::threads_meaning(&draft.text)
    } else {
        String::new()
    };

    // The controls' width, right to left: [rec] [field] [browse] [clear].
    let rec = match (kind, recommended) {
        (ValueKind::None, _) | (_, None) => Rec::None,
        (_, Some(r)) if r == saved => Rec::Matches,
        (_, Some(r)) => Rec::Offer(r, mono(ui, r, size::TEXT_3XS)),
    };
    let has_value = !saved.is_empty();
    let mut ctrl: Vec<f32> = Vec::new();
    if kind != ValueKind::None {
        if !matches!(rec, Rec::None) {
            ctrl.push(rec.width());
        }
        ctrl.push(kind.width());
        if kind == ValueKind::Folder {
            ctrl.push(size::CONTROL_SM);
        }
        if has_value {
            ctrl.push(size::CONTROL_SM);
        }
    }
    let ctrl_w: f32 = ctrl.iter().sum::<f32>() + 4.0 * ctrl.len().saturating_sub(1) as f32;

    let w = ui.available_width();
    let text_w = (w - 2.0 * PAD_X - ICON_COL - ctrl_w - SWITCH_W - 3.0 * GAP).max(40.0);

    // The words: the name, then the flag and what it does (two lines at most).
    let label_g = paint::galley_trunc(
        ui,
        label,
        font(Face::SansMedium, size::TEXT_XS),
        Color32::PLACEHOLDER,
        text_w,
    );
    let flag_g = mono(ui, &catalog::flag_text(&opt.key, None), size::TEXT_3XS);
    let code_w = flag_g.size().x + 8.0;
    let desc_w = (text_w - code_w - 6.0).max(20.0);
    let desc_g = {
        let mut job = LayoutJob::default();
        let fmt = |c: Color32| TextFormat {
            font_id: FontId::new(size::TEXT_2XS, font(Face::Sans, size::TEXT_2XS).family),
            color: c,
            line_height: Some(LINE_H),
            ..Default::default()
        };
        job.append(desc, 0.0, fmt(tk.fg_faint));
        if !threads.is_empty() {
            job.append(&format!(" · {threads}"), 0.0, fmt(tk.fg_muted));
        }
        job.wrap.max_width = desc_w;
        job.wrap.max_rows = 2;
        job.wrap.break_anywhere = false;
        job.wrap.overflow_character = Some('…');
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let lines_h = desc_g.size().y.max(CODE_H);
    let content_h = LABEL_H + 2.0 + lines_h;
    let h = (content_h.max(size::CONTROL_SM) + 2.0 * PAD_Y).round();

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);
    let hover = paint::ease(ui, resp.id.with("hover"), hovered);
    if hover > 0.0 {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, alpha(tk.raised, 0.4 * hover));
    }
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .id_salt(("opt-row", &opt.key)),
    );
    if !opt.enabled {
        row.multiply_opacity(0.7);
    }
    let ui = &mut row;
    let cy = rect.center().y;
    let p = ui.painter().clone();

    // The icon.
    let x = rect.left() + PAD_X;
    icon.paint(
        &p,
        Rect::from_center_size(Pos2::new(x + 8.0, cy), Vec2::splat(size::ICON)),
        if opt.enabled { tone } else { tk.fg_faint },
    );

    // The words.
    let tx = x + ICON_COL + GAP;
    let top = cy - content_h / 2.0;
    p.galley(
        Pos2::new(tx, top + (LABEL_H - label_g.size().y) / 2.0),
        label_g,
        tk.fg,
    );
    let ly = top + LABEL_H + 2.0;
    let code = Rect::from_min_size(Pos2::new(tx, ly), Vec2::new(code_w, CODE_H));
    let (code_bg, code_ink) = if opt.enabled {
        (alpha(tk.accent, 0.12), tk.accent)
    } else {
        (tk.raised, tk.fg_faint)
    };
    p.rect_filled(code, CornerRadius::same(size::RADIUS_XS), code_bg);
    paint::text_at(&p, &flag_g, code.left() + 4.0, code.center().y, code_ink);
    let dx = code.right() + 6.0;
    let desc_rect = Rect::from_min_size(Pos2::new(dx, ly), desc_g.size());
    p.galley(desc_rect.min, desc_g, tk.fg_faint);
    ui.interact(
        Rect::from_min_max(code.min, desc_rect.max.max(code.max)),
        ui.id().with("desc"),
        Sense::hover(),
    )
    .tip(desc);

    // The switch, at the end.
    let sx = rect.right() - PAD_X - SWITCH_W;
    let mut on = opt.enabled;
    let sw = ui.put(
        Rect::from_min_size(Pos2::new(sx, cy - 7.0), Vec2::new(SWITCH_W, 14.0)),
        MiniSwitch::new(&mut on).label(label),
    );
    if sw.changed() {
        acts.push(Act::Toggle(opt.key.clone()));
    }

    if kind == ValueKind::None {
        return;
    }
    // The controls, left to right, ending a gap before the switch.
    let mut cx = sx - GAP - ctrl_w;
    let ctl = |x: f32, w: f32, hgt: f32| {
        Rect::from_min_size(Pos2::new(x, cy - hgt / 2.0), Vec2::new(w, hgt))
    };
    match &rec {
        Rec::None => {}
        Rec::Matches => {
            let r = ctl(cx, size::ICON_SM, size::ICON_SM);
            lucide::CHECK.paint(&p, r, tk.ok);
            ui.interact(r, ui.id().with("rec-ok"), Sense::hover())
                .tip(t("options", "matchesRecommended"));
            cx += rec.width() + 4.0;
        }
        Rec::Offer(value, g) => {
            let r = ctl(cx, rec.width(), size::CONTROL_SM);
            let b = ui
                .interact(r, ui.id().with("rec"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let hv = paint::ease(ui, b.id.with("hover"), b.hovered());
            p.rect(
                r,
                CornerRadius::same(size::RADIUS_SM),
                alpha(tk.accent, 0.1 * hv),
                Stroke::new(1.0, alpha(tk.accent, 0.3)),
                StrokeKind::Inside,
            );
            lucide::SPARKLES.paint(
                &p,
                Rect::from_min_size(Pos2::new(r.left() + 7.0, cy - 6.0), Vec2::splat(12.0)),
                tk.accent,
            );
            paint::text_at(&p, g, r.left() + 7.0 + 12.0 + 4.0, cy, tk.accent);
            if b.tip(t("options", "useRecommended")).clicked() {
                acts.push(Act::Set(opt.key.clone(), Some((*value).to_string())));
            }
            cx += rec.width() + 4.0;
        }
    }

    // The field.
    let fw = kind.width();
    let field = ctl(cx, fw, size::CONTROL_SM);
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let fr = ui.put(
        field,
        Input::new(&mut draft.text)
            .size(FieldSize::Xs)
            .width(fw)
            .kind(InputKind::Mono)
            .placeholder(t("options", "valuePlaceholder"))
            .char_limit(1024)
            .id_salt(("opt-value", &opt.key)),
    );
    if kind.numeric() && fr.changed() {
        // `type="number"`: figures only.
        draft.text.retain(|c| c.is_ascii_digit());
    }
    if fr.lost_focus() {
        if esc {
            draft.text = saved.to_string();
        } else {
            let v = draft.text.trim();
            if v != saved {
                acts.push(Act::Set(
                    opt.key.clone(),
                    (!v.is_empty()).then(|| v.to_string()),
                ));
            }
        }
    }
    if kind == ValueKind::Mb {
        let g = paint::galley(
            ui,
            t("options", "unitMb"),
            font(Face::Sans, size::TEXT_3XS),
            Color32::PLACEHOLDER,
        );
        p.galley(
            Pos2::new(field.right() - 20.0 - g.size().x, cy - g.size().y / 2.0),
            g,
            tk.fg_faint,
        );
    }
    cx += fw + 4.0;

    if kind == ValueKind::Folder {
        let b = ui.put(
            ctl(cx, size::CONTROL_SM, size::CONTROL_SM),
            IconButton::new(lucide::FOLDER_OPEN, t("options", "browseFolder"))
                .size(Size::IconXs)
                .variant(Variant::Ghost),
        );
        if b.clicked() {
            acts.push(Act::Browse(opt.key.clone()));
        }
        cx += size::CONTROL_SM + 4.0;
    }

    if has_value {
        // Shown while the row is under the pointer or the button has the focus.
        let id = ui.id().with("clear");
        let focused = ui.memory(|m| m.focused()).is_some_and(|f| f == id);
        let vis = paint::ease(ui, id.with("vis"), hovered || focused);
        let mut s = ui.new_child(
            UiBuilder::new()
                .max_rect(ctl(cx, size::CONTROL_SM, size::CONTROL_SM))
                .id_salt("clear-scope"),
        );
        s.multiply_opacity(vis);
        let b = s.add(
            IconButton::new(lucide::X, t("options", "clearValue"))
                .size(Size::IconXs)
                .variant(Variant::Ghost)
                .danger_hover(),
        );
        if b.clicked() {
            draft.text.clear();
            acts.push(Act::Set(opt.key.clone(), None));
        }
    }
}

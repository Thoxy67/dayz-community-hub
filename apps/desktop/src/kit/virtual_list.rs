//! `ui/virtual-list`: a long list of which only the rows on screen are laid out.
//!
//! Rows are one height (`size::ROW` by default), so which rows show is arithmetic, not
//! measurement; nothing is done per row that is not on screen (plus `overscan` each
//! side). The list takes the whole space on offer (a bounded scroller: never one that
//! grows with its content). A header stays at the top while the rows scroll under it;
//! `stick_to_end` follows the end as a log does, while the reader is at it.

use std::ops::Range;

use egui::{Id, Pos2, Rect, ScrollArea, Sense, Ui, UiBuilder, Vec2};

/// What the list showed this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualOutput {
    /// The rows on screen, without overscan (`onrange`).
    pub visible: Range<usize>,
    /// The scroll offset, in points.
    pub offset: f32,
    /// The rect the rows were drawn in (below the header).
    pub viewport: Rect,
}

/// `VirtualList::new("servers", rows.len()).header(28.0).show(ui, |ui| header(ui), |ui, i| row(ui, i))`.
#[must_use = "call show(…)"]
pub struct VirtualList {
    id: Id,
    count: usize,
    row_height: f32,
    overscan: usize,
    header: f32,
    stick_to_end: bool,
    scroll_to: Option<usize>,
    offset: Option<f32>,
}

impl VirtualList {
    pub fn new(id_salt: impl egui::AsId, count: usize) -> Self {
        Self {
            id: Id::new(id_salt),
            count,
            row_height: crate::theme::size::ROW,
            overscan: 6,
            header: 0.0,
            stick_to_end: false,
            scroll_to: None,
            offset: None,
        }
    }

    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = h.max(1.0);
        self
    }

    /// Rows laid out beyond each edge, so a flick shows no gap.
    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    /// A header of this height, drawn by `show`'s first closure, that stays at the top.
    pub fn header(mut self, h: f32) -> Self {
        self.header = h.max(0.0);
        self
    }

    pub fn stick_to_end(mut self) -> Self {
        self.stick_to_end = true;
        self
    }

    /// Brings a row into view (the selection, a row a controller walked to).
    pub fn scroll_to(mut self, index: Option<usize>) -> Self {
        self.scroll_to = index;
        self
    }

    /// Puts the scroll at `y` points (a new search starts at the top).
    pub fn offset(mut self, y: Option<f32>) -> Self {
        self.offset = y;
        self
    }

    /// Draws the header (when one was asked for) and the rows on screen. `row` gets a
    /// `Ui` exactly one row in size and the row's index. When `count` is 0, `row` is not
    /// called; draw the empty state in place of the list.
    pub fn show(
        self,
        ui: &mut Ui,
        header: impl FnOnce(&mut Ui),
        mut row: impl FnMut(&mut Ui, usize),
    ) -> VirtualOutput {
        let full = ui.available_rect_before_wrap();
        if self.header > 0.0 {
            let hr = Rect::from_min_size(full.min, Vec2::new(full.width(), self.header));
            let mut hui = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("header"))
                    .max_rect(hr),
            );
            hui.set_clip_rect(hr.intersect(ui.clip_rect()));
            header(&mut hui);
            ui.allocate_rect(hr, Sense::hover());
        }
        let h = self.row_height;
        let scroll_id = self.id.with("scroll");
        let mut area = ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .stick_to_bottom(self.stick_to_end);
        if let Some(i) = self.scroll_to {
            let last: Option<(f32, f32)> = ui.data(|d| d.get_temp(self.id.with("view")));
            if let Some((offset, height)) = last {
                let top = i as f32 * h;
                if top < offset {
                    area = area.vertical_scroll_offset(top);
                } else if top + h > offset + height {
                    area = area.vertical_scroll_offset(top + h - height);
                }
            }
        }
        if let Some(y) = self.offset {
            area = area.vertical_scroll_offset(y.max(0.0));
        }
        let total = self.count as f32 * h;
        let overscan = self.overscan;
        let count = self.count;
        let out = area.show_viewport(ui, |ui, view| {
            ui.set_height(total);
            ui.set_width(ui.available_width());
            let first = ((view.min.y / h).floor().max(0.0) as usize).min(count);
            let last = ((view.max.y / h).ceil().max(0.0) as usize).min(count);
            let from = first.saturating_sub(overscan);
            let to = last.saturating_add(overscan).min(count);
            let origin = ui.min_rect().min;
            let w = ui.available_width().max(ui.min_rect().width());
            for i in from..to {
                let r = Rect::from_min_size(
                    Pos2::new(origin.x, origin.y + i as f32 * h),
                    Vec2::new(w, h),
                );
                let mut rui = ui.new_child(UiBuilder::new().id_salt(("row", i)).max_rect(r));
                rui.set_clip_rect(r.intersect(ui.clip_rect()));
                row(&mut rui, i);
            }
            first..last
        });
        let offset = out.state.offset.y;
        ui.data_mut(|d| d.insert_temp(self.id.with("view"), (offset, out.inner_rect.height())));
        VirtualOutput {
            visible: out.inner,
            offset,
            viewport: out.inner_rect,
        }
    }
}

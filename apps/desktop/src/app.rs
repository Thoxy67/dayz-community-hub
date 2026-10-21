//! The window: the frame loop, the event pump, the view host.

use crate::theme::{self, TokensExt};

pub struct HubApp {}

impl HubApp {
    pub fn new(ctx: &egui::Context) -> Self {
        ctx.set_fonts(theme::fonts::definitions());
        theme::apply(ctx, theme::Tokens::default());
        Self {}
    }

    /// One frame of the whole window, into `ui` (the root, or `ui_shot`'s harness).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let t = ui.tokens();
        egui::Frame::new().fill(t.bg).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.label(egui::RichText::new("DAYZ COMMUNITY HUB").font(theme::font(theme::fonts::Face::DisplayBold, theme::size::TEXT_2XL)).color(t.accent));
        });
    }
}

impl eframe::App for HubApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        HubApp::ui(self, ui);
    }
}

//! Every place the window can show (= `apps/gui/src/shell/nav.ts`), and the views
//! themselves (= `src/features/<view>`). The rail, the keyboard shortcuts (Ctrl+1…) and
//! the view host all read [`PLACES`].
//!
//! Adding a view is one module (`views/<id>/mod.rs`, a type implementing [`View`]) and
//! one line in [`create`]. Until a view agent ports it, each id shows a [`Placeholder`].
//!
//! Views are made on first visit and then kept (as `App.svelte` kept them mounted), so a
//! list keeps its scroll, its filters and its selection when the player comes back.

pub mod about;
pub mod connect;
pub mod favorites;
pub mod friends;
pub mod history;
pub mod mods;
pub mod news;
pub mod offline;
pub mod options;
pub mod saved;
pub mod servers;
pub mod settings;
pub mod setup;
pub mod stats;

use dz_hub::HubEvent;

use crate::app::Ctx;
use crate::i18n::t;
use crate::icons::{Icon, game, lucide};
use crate::kit::EmptyState;
use crate::theme::TokensExt;

/// A place of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ViewId {
    #[default]
    Servers,
    Favorites,
    History,
    Friends,
    Connect,
    Offline,
    Mods,
    Options,
    News,
    Stats,
    Settings,
    About,
}

impl ViewId {
    /// In the rail's order (which is also Ctrl+PgUp/PgDn's).
    pub const ALL: [ViewId; 12] = [
        ViewId::Servers,
        ViewId::Favorites,
        ViewId::History,
        ViewId::Friends,
        ViewId::Connect,
        ViewId::Offline,
        ViewId::Mods,
        ViewId::Options,
        ViewId::News,
        ViewId::Stats,
        ViewId::Settings,
        ViewId::About,
    ];

    /// The id as the Svelte interface, the commands and the control channel write it.
    pub fn id(self) -> &'static str {
        match self {
            ViewId::Servers => "servers",
            ViewId::Favorites => "favorites",
            ViewId::History => "history",
            ViewId::Friends => "friends",
            ViewId::Connect => "connect",
            ViewId::Offline => "offline",
            ViewId::Mods => "mods",
            ViewId::Options => "options",
            ViewId::News => "news",
            ViewId::Stats => "stats",
            ViewId::Settings => "settings",
            ViewId::About => "about",
        }
    }

    pub fn from_id(id: &str) -> Option<ViewId> {
        ViewId::ALL.into_iter().find(|v| v.id() == id)
    }

    /// Its place in the navigation.
    pub fn place(self) -> &'static Place {
        let i = ViewId::ALL.iter().position(|v| *v == self).unwrap_or(0);
        PLACES.get(i).unwrap_or(&PLACES[0])
    }

    /// Its name, in the current language.
    pub fn label(self) -> &'static str {
        t("nav", self.place().label)
    }

    /// The view `delta` places away in the rail's order, wrapping.
    pub fn step(self, delta: i32) -> ViewId {
        let n = ViewId::ALL.len() as i32;
        let i = ViewId::ALL.iter().position(|v| *v == self).unwrap_or(0) as i32;
        let j = (i + delta).rem_euclid(n);
        ViewId::ALL.get(j as usize).copied().unwrap_or_default()
    }
}

/// The rail's groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Play,
    Gear,
    Intel,
    /// The foot of the rail (settings, about), without a heading.
    Foot,
}

/// One place: its group, its words key in the `nav` dictionary, its icon.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub id: ViewId,
    pub group: Group,
    pub label: &'static str,
    pub icon: Icon,
}

pub const PLACES: [Place; 12] = [
    Place {
        id: ViewId::Servers,
        group: Group::Play,
        label: "servers",
        icon: lucide::SERVER,
    },
    Place {
        id: ViewId::Favorites,
        group: Group::Play,
        label: "favorites",
        icon: lucide::STAR,
    },
    Place {
        id: ViewId::History,
        group: Group::Play,
        label: "history",
        icon: lucide::HISTORY,
    },
    Place {
        id: ViewId::Friends,
        group: Group::Play,
        label: "friends",
        icon: lucide::USERS,
    },
    Place {
        id: ViewId::Connect,
        group: Group::Play,
        label: "connect",
        icon: lucide::PLUG_ZAP,
    },
    Place {
        id: ViewId::Offline,
        group: Group::Play,
        label: "offline",
        icon: game::CAMPING_TENT,
    },
    Place {
        id: ViewId::Mods,
        group: Group::Gear,
        label: "mods",
        icon: lucide::PUZZLE,
    },
    Place {
        id: ViewId::Options,
        group: Group::Gear,
        label: "options",
        icon: lucide::SLIDERS_HORIZONTAL,
    },
    Place {
        id: ViewId::News,
        group: Group::Intel,
        label: "news",
        icon: lucide::NEWSPAPER,
    },
    Place {
        id: ViewId::Stats,
        group: Group::Intel,
        label: "stats",
        icon: lucide::CHART_NO_AXES_COLUMN,
    },
    Place {
        id: ViewId::Settings,
        group: Group::Foot,
        label: "settings",
        icon: lucide::SETTINGS,
    },
    Place {
        id: ViewId::About,
        group: Group::Foot,
        label: "about",
        icon: lucide::INFO,
    },
];

/// The rail's headed groups and their words keys (`nav` dictionary).
pub const GROUPS: [(Group, &str); 3] = [
    (Group::Play, "groupPlay"),
    (Group::Gear, "groupGear"),
    (Group::Intel, "groupIntel"),
];

/// A view: drawn into the main area while it is the current one, kept alive after.
pub trait View {
    /// Draw the view into `ui` (the main area, already sized and clipped).
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx);

    /// The view comes on screen (its first visit included). `cx.nav.focus` says which
    /// part was asked for (`settings#apis`), if any.
    fn on_show(&mut self, _cx: &mut Ctx) {}

    /// Every hub event, whether the view is on screen or not (after the stores saw it).
    fn on_event(&mut self, _ev: &HubEvent, _cx: &mut Ctx) {}
}

/// Draws the shared server detail (= `ServerDetail.svelte`) for `(ip, query port, name)`
/// into the `Ui` it is given.
pub type DetailHost = for<'a, 'b> fn(&'a mut egui::Ui, &'a mut Ctx<'b>, &'a str, i64, &'a str);

static DETAIL_HOST: std::sync::OnceLock<DetailHost> = std::sync::OnceLock::new();

/// The view that owns the server detail (servers) installs it once; the others (favourites,
/// history, Direct Connect) draw it with [`server_detail`].
pub fn set_server_detail(host: DetailHost) {
    let _ = DETAIL_HOST.set(host);
}

/// Draw the shared server detail; `false` (nothing drawn) while none is installed, so the
/// caller draws its own summary.
pub fn server_detail(ui: &mut egui::Ui, cx: &mut Ctx, ip: &str, port: i64, name: &str) -> bool {
    match DETAIL_HOST.get() {
        Some(host) => {
            host(ui, cx, ip, port, name);
            true
        }
        None => false,
    }
}

/// Once at start: what views draw outside their own area before they are first shown
/// (the mod operation's dialog, through the shell's hooks).
pub fn install() {
    mods::install();
    servers::install();
    setup::install();
}

/// The view for `id`: one line per view as they are ported
/// (`ViewId::Servers => Box::new(servers::ServersView::new()),`).
pub fn create(id: ViewId) -> Box<dyn View> {
    match id {
        ViewId::About => Box::new(about::AboutView::new()),
        ViewId::Connect => Box::new(connect::ConnectView::new()),
        ViewId::Favorites => Box::new(favorites::FavoritesView::new()),
        ViewId::Friends => Box::new(friends::FriendsView::new()),
        ViewId::History => Box::new(history::HistoryView::new()),
        ViewId::Mods => Box::new(mods::ModsView::new()),
        ViewId::News => Box::new(news::NewsView::new()),
        ViewId::Offline => Box::new(offline::OfflineView::new()),
        ViewId::Options => Box::new(options::OptionsView::new()),
        ViewId::Settings => Box::new(settings::SettingsView::new()),
        ViewId::Stats => Box::new(stats::StatsView::new()),
        ViewId::Servers => Box::new(servers::ServersView::new()),
    }
}

/// A view not ported yet: its name over the empty-state plate.
pub struct Placeholder(pub ViewId);

impl View for Placeholder {
    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Ctx) {
        let t = ui.tokens();
        let place = self.0.place();
        egui::Frame::new()
            .fill(t.panel)
            .inner_margin(egui::Margin::same(crate::theme::size::PAD as i8))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.label(
                    egui::RichText::new(self.0.label())
                        .font(crate::theme::font(
                            crate::theme::fonts::Face::Display,
                            crate::theme::size::TEXT_XL,
                        ))
                        .color(t.fg),
                );
                EmptyState::new()
                    .icon(place.icon)
                    .title(self.0.label())
                    .text(crate::i18n::t("common", "loading"))
                    .show(ui, |_| {});
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_follow_the_view_order() {
        for (i, v) in ViewId::ALL.iter().enumerate() {
            assert_eq!(PLACES[i].id, *v);
            assert_eq!(ViewId::from_id(v.id()), Some(*v));
            assert!(crate::i18n::has("nav", v.place().label), "{v:?}");
        }
        assert_eq!(ViewId::Servers.step(-1), ViewId::About);
        assert_eq!(ViewId::About.step(1), ViewId::Servers);
    }
}

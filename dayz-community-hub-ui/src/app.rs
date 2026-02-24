//! Application state, types, and all `App` methods.

use dayz_community_hub_core::{
    a2s_query, api, config,
    ctl::{DayzCtl, ModOpResult, ModOperation},
    mods, news,
    offline::OfflineMode,
    steamcmd::ModProgress,
};
use ratatui::style::Color;
use std::collections::HashMap;
use tokio::sync::mpsc;

// ─── Enums ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Servers,
    Favorites,
    History,
    Mods,
    News,
    DirectConnect,
    Options,
    Offline,
}

impl Tab {
    pub fn index(&self) -> usize {
        match self {
            Tab::Servers => 0,
            Tab::Favorites => 1,
            Tab::History => 2,
            Tab::Mods => 3,
            Tab::News => 4,
            Tab::DirectConnect => 5,
            Tab::Options => 6,
            Tab::Offline => 7,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Tab::Servers => "Servers",
            Tab::Favorites => "Favorites",
            Tab::History => "History",
            Tab::Mods => "Mods",
            Tab::News => "News",
            Tab::DirectConnect => "Connect",
            Tab::Options => "Options",
            Tab::Offline => "Offline",
        }
    }

    pub fn all() -> &'static [Tab] {
        &[
            Tab::Servers,
            Tab::Favorites,
            Tab::History,
            Tab::Mods,
            Tab::News,
            Tab::DirectConnect,
            Tab::Options,
            Tab::Offline,
        ]
    }

    pub fn next(&self) -> Tab {
        let all = Self::all();
        let idx = self.index();
        all[(idx + 1) % all.len()]
    }

    pub fn prev(&self) -> Tab {
        let all = Self::all();
        let idx = self.index();
        all[(idx + all.len() - 1) % all.len()]
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DirectConnectField {
    Address,
    Port,
    Password,
    ServerInfo,
    Connect,
}

#[derive(Clone)]
pub enum Popup {
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
    },
    Info {
        title: String,
        message: String,
    },
}

#[derive(Clone)]
pub enum ConfirmAction {
    DeleteMod(u64),
    CleanupMods,
    InstallMods(Vec<u64>),
    RemoveFavorite(String, u16),
    RemoveHistoryEntry(String, u16),
    ClearHistory,
    UpdateThenLaunch(dayz_community_hub_core::Server, Option<String>),
    LaunchDirect(dayz_community_hub_core::Server, Option<String>),
}

/// Tracks the live progress of a background mod operation.
#[derive(Clone)]
pub struct ProgressState {
    pub current: usize,
    pub total: usize,
    pub current_mod_name: String,
    pub current_mod_id: u64,
    pub phase: ProgressPhase,
    pub completed: Vec<(u64, String, bool)>, // (id, name, success)
}

#[derive(Clone, PartialEq)]
pub enum ProgressPhase {
    ShuttingDownSteam,
    Downloading,
    Finished {
        ok: usize,
        failed: usize,
        hint: Option<String>,
    },
}

/// Results sent back from non-blocking background tasks.
pub enum BackgroundResult {
    A2sDetails(Box<a2s_query::ServerDetails>, usize),
    A2sError(String),
    ServerListRefreshed(api::ServerList),
    ServerListError(String),
    SteamPlayerCount(u32),
    NewsRefreshed(Vec<news::Article>),
    NewsError(String),
    OfflineModeUpdated,
    OfflineModeError(String),
    LaunchDone(String),
    LaunchError(String),
}

/// Action to perform after a background mod operation completes.
#[derive(Clone)]
pub enum PendingAfterOp {
    LaunchServer(dayz_community_hub_core::Server, Option<String>),
    RefreshMods,
}

// ─── Application State ────────────────────────────────────────────────────

pub struct App {
    pub ctl: DayzCtl,
    pub servers: Vec<dayz_community_hub_core::Server>,
    pub selected_indices: [usize; 8],
    pub offsets: [usize; 8],
    pub tab: Tab,
    pub status_message: Option<String>,
    pub status_color: Color,
    // Direct connect
    pub direct_address: String,
    pub direct_port: String,
    pub direct_password: String,
    pub direct_cursor: DirectConnectField,
    pub direct_server_found: Option<dayz_community_hub_core::Server>,
    // Mods
    pub installed_mods: Option<Vec<mods::InstalledMod>>,
    // Server details
    pub current_a2s_details: Option<a2s_query::ServerDetails>,
    pub detailed_server_index: Option<usize>,
    pub show_server_details: bool,
    pub details_scroll_offset: u16,
    // Search
    pub search_query: String,
    pub search_active: bool,
    pub filtered_indices: Option<Vec<usize>>,
    // Option value editing
    pub option_edit_active: bool,
    pub option_edit_value: String,
    // Popup
    pub popup: Option<Popup>,
    // Terminal size
    pub term_height: u16,
    // Loading indicator
    pub loading: bool,
    // Background mod operation progress
    pub progress_rx: Option<mpsc::UnboundedReceiver<ModProgress>>,
    pub progress_handle: Option<tokio::task::JoinHandle<ModOpResult>>,
    pub progress_state: Option<ProgressState>,
    // What to do after a background operation finishes
    pub pending_after_op: Option<PendingAfterOp>,
    // Ping cache: "ip:query_port" -> ping_ms
    pub ping_cache: HashMap<String, u32>,
    // Background ping receiver
    pub ping_rx: Option<mpsc::UnboundedReceiver<(String, u32)>>,
    // Background result receiver (A2S, server refresh)
    pub bg_rx: Option<mpsc::UnboundedReceiver<BackgroundResult>>,
    // Persistent background channel for fire-and-forget tasks (stats, news)
    pub misc_tx: Option<mpsc::UnboundedSender<BackgroundResult>>,
    pub misc_rx: Option<mpsc::UnboundedReceiver<BackgroundResult>>,
    // Player counts shown in title bar
    pub steam_players: Option<u32>,
    // News tab
    pub news_articles: Option<Vec<news::Article>>,
    pub news_fetched_at: Option<std::time::Instant>,
    pub news_detail_scroll: u16,
    // Offline mode tab
    pub offline_missions: Option<Vec<String>>,
    pub offline_status: Option<String>,
    pub offline_status_color: Color,
    // Launch guard: true while a background launch task is in flight
    pub launching: bool,
}

impl App {
    pub fn new(ctl: DayzCtl, servers: Vec<dayz_community_hub_core::Server>) -> Self {
        Self {
            ctl,
            servers,
            selected_indices: [0; 8],
            offsets: [0; 8],
            tab: Tab::Servers,
            status_message: None,
            status_color: Color::Cyan,
            direct_address: String::new(),
            direct_port: "2302".to_string(),
            direct_password: String::new(),
            direct_cursor: DirectConnectField::Address,
            direct_server_found: None,
            installed_mods: None,
            current_a2s_details: None,
            detailed_server_index: None,
            show_server_details: false,
            details_scroll_offset: 0,
            search_query: String::new(),
            search_active: false,
            filtered_indices: None,
            option_edit_active: false,
            option_edit_value: String::new(),
            popup: None,
            term_height: 24,
            loading: false,
            progress_rx: None,
            progress_handle: None,
            progress_state: None,
            pending_after_op: None,
            ping_cache: HashMap::new(),
            ping_rx: None,
            bg_rx: None,
            misc_tx: None,
            misc_rx: None,
            steam_players: None,
            news_articles: None,
            news_fetched_at: None,
            news_detail_scroll: 0,
            offline_missions: None,
            offline_status: None,
            offline_status_color: Color::Cyan,
            launching: false,
        }
    }

    // ─── Status helpers ───

    pub fn set_status(&mut self, msg: impl Into<String>, color: Color) {
        self.status_message = Some(msg.into());
        self.status_color = color;
    }

    pub fn set_info(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Cyan);
    }

    pub fn set_success(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Green);
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Red);
    }

    pub fn set_warn(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Yellow);
    }

    // ─── Mods ───

    pub fn refresh_installed_mods(&mut self) {
        match self.ctl.get_installed_mods() {
            Ok(mods) => {
                let count = mods.len();
                self.installed_mods = Some(mods);
                self.set_info(format!("Loaded {} installed mods", count));
            }
            Err(e) => {
                self.set_error(format!("Failed to load mods: {}", e));
                self.installed_mods = None;
            }
        }
    }

    // ─── Navigation helpers ───

    pub fn selected_index(&self) -> usize {
        self.selected_indices[self.tab.index()]
    }

    pub fn selected_index_mut(&mut self) -> &mut usize {
        &mut self.selected_indices[self.tab.index()]
    }

    pub fn offset(&self) -> usize {
        self.offsets[self.tab.index()]
    }

    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offsets[self.tab.index()]
    }

    pub fn visible_items(&self) -> usize {
        // Overhead: tab bar (3) + title bar (1) + status bar (1) + list borders (2) = 7
        let overhead = 9u16;
        let available = self.term_height.saturating_sub(overhead);
        let lines_per_item: u16 = match self.tab {
            Tab::Mods | Tab::News | Tab::Offline => 1,
            _ => 2,
        };
        (available / lines_per_item).max(3) as usize
    }

    pub fn current_list_len(&self) -> usize {
        match self.tab {
            Tab::Servers => {
                if let Some(ref indices) = self.filtered_indices {
                    indices.len()
                } else {
                    self.servers.len()
                }
            }
            Tab::Favorites => self.ctl.profile().favorites.len(),
            Tab::History => self.ctl.profile().history.len(),
            Tab::Mods => self.installed_mods.as_ref().map(|v| v.len()).unwrap_or(0),
            Tab::Options => self.ctl.profile().options.all_options().len(),
            Tab::DirectConnect => 5,
            Tab::News => self.news_articles.as_ref().map(|a| a.len()).unwrap_or(0),
            Tab::Offline => self.offline_missions.as_ref().map(|m| m.len()).unwrap_or(0),
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.tab == Tab::DirectConnect {
            let current = self.direct_cursor as u8;
            let new = (current as i32 + delta).rem_euclid(5) as u8;
            self.direct_cursor = match new {
                0 => DirectConnectField::Address,
                1 => DirectConnectField::Port,
                2 => DirectConnectField::Password,
                3 => DirectConnectField::ServerInfo,
                4 => DirectConnectField::Connect,
                _ => DirectConnectField::Address,
            };
            return;
        }

        let len = self.current_list_len();
        if len == 0 {
            return;
        }

        let current = self.selected_index() as i32;
        let new_index = (current + delta).clamp(0, len as i32 - 1) as usize;
        *self.selected_index_mut() = new_index;

        let visible = self.visible_items();
        let offset = self.offset_mut();
        if new_index < *offset {
            *offset = new_index;
        } else if new_index >= *offset + visible {
            *offset = new_index - visible + 1;
        }
    }

    pub fn page_up(&mut self) {
        let visible = self.visible_items() as i32;
        self.move_selection(-visible);
    }

    pub fn page_down(&mut self) {
        let visible = self.visible_items() as i32;
        self.move_selection(visible);
    }

    // ─── Server helpers ───

    pub fn get_selected_server(&self) -> Option<dayz_community_hub_core::Server> {
        match self.tab {
            Tab::Servers => {
                let idx = self.selected_index();
                if let Some(ref indices) = self.filtered_indices {
                    indices.get(idx).and_then(|&i| self.servers.get(i)).cloned()
                } else {
                    self.servers.get(idx).cloned()
                }
            }
            Tab::Favorites => {
                let idx = self.selected_index();
                let fav = self.ctl.profile().favorites.get(idx)?;
                self.servers
                    .iter()
                    .find(|s| {
                        s.endpoint.ip == fav.ip
                            && (s.endpoint.port as u16 == fav.port
                                || s.game_port as u16 == fav.port)
                    })
                    .cloned()
            }
            Tab::History => {
                let idx = self.selected_index();
                let entry = self.ctl.profile().history.get(idx)?;
                self.servers
                    .iter()
                    .find(|s| {
                        s.endpoint.ip == entry.ip
                            && (s.endpoint.port as u16 == entry.port
                                || s.game_port as u16 == entry.port)
                    })
                    .cloned()
            }
            _ => None,
        }
    }

    pub fn get_ping(&self, server: &dayz_community_hub_core::Server) -> Option<u32> {
        let key = format!("{}:{}", server.endpoint.ip, server.endpoint.port);
        self.ping_cache.get(&key).copied()
    }

    // ─── Favorites ───

    pub fn add_favorite(&mut self) {
        if let Some(server) = self.get_selected_server() {
            let is_fav = self
                .ctl
                .profile()
                .is_favorite(&server.endpoint.ip, server.endpoint.port as u16);
            if is_fav {
                self.set_warn(format!("{} is already a favorite", server.name));
            } else {
                self.ctl.add_favorite(
                    server.name.clone(),
                    server.endpoint.ip.clone(),
                    server.endpoint.port as u16,
                );
                if let Err(e) = self.ctl.save_profile() {
                    self.set_error(format!("Error saving: {}", e));
                } else {
                    self.set_success(format!("Added {} to favorites", server.name));
                }
            }
        }
    }

    pub fn remove_favorite_at_index(&mut self) {
        if self.tab != Tab::Favorites {
            return;
        }
        let idx = self.selected_index();
        if let Some(fav) = self.ctl.profile().favorites.get(idx).cloned() {
            self.popup = Some(Popup::Confirm {
                title: "Remove Favorite".to_string(),
                message: format!(
                    "Remove '{}' ({}:{}) from favorites?",
                    fav.name, fav.ip, fav.port
                ),
                action: ConfirmAction::RemoveFavorite(fav.ip.clone(), fav.port),
            });
        }
    }

    // ─── History ───

    pub fn remove_history_entry_at_index(&mut self) {
        if self.tab != Tab::History {
            return;
        }
        let idx = self.selected_index();
        if let Some(entry) = self.ctl.profile().history.get(idx).cloned() {
            self.popup = Some(Popup::Confirm {
                title: "Remove History Entry".to_string(),
                message: format!(
                    "Remove '{}' ({}:{}) from history?",
                    entry.name, entry.ip, entry.port
                ),
                action: ConfirmAction::RemoveHistoryEntry(entry.ip.clone(), entry.port),
            });
        }
    }

    pub fn clear_history(&mut self) {
        if self.tab != Tab::History {
            return;
        }
        if self.ctl.profile().history.is_empty() {
            self.set_info("History is already empty");
            return;
        }
        self.popup = Some(Popup::Confirm {
            title: "Clear History".to_string(),
            message: format!(
                "Clear all {} history entries?",
                self.ctl.profile().history.len()
            ),
            action: ConfirmAction::ClearHistory,
        });
    }

    // ─── Connection / Launch ───

    pub fn connect_to_selected(&mut self) {
        if self.launching {
            return;
        }

        if self.tab == Tab::DirectConnect {
            self.handle_direct_connect();
            return;
        }

        let server = match self.get_selected_server() {
            Some(s) => s,
            None => {
                if self.tab == Tab::Favorites || self.tab == Tab::History {
                    self.set_error("Server is offline or not found in server list");
                }
                return;
            }
        };

        match self.ctl.get_missing_mod_ids(&server) {
            Ok(missing) => {
                if !missing.is_empty() {
                    let mod_names: Vec<String> = missing
                        .iter()
                        .map(|id| {
                            server
                                .mods
                                .iter()
                                .find(|m| m.steam_workshop_id as u64 == *id)
                                .map(|m| format!("{} ({})", m.name, id))
                                .unwrap_or_else(|| id.to_string())
                        })
                        .collect();

                    self.popup = Some(Popup::Confirm {
                        title: format!("Missing {} Mods", missing.len()),
                        message: format!("Install these mods?\n{}", mod_names.join("\n")),
                        action: ConfirmAction::InstallMods(missing),
                    });
                    self.direct_server_found = Some(server);
                } else if !server.mods.is_empty() {
                    self.popup = Some(Popup::Confirm {
                        title: "Update mods?".to_string(),
                        message: format!(
                            "Server: {}\n{} mods installed. Update before launching?",
                            server.name,
                            server.mods.len()
                        ),
                        action: ConfirmAction::UpdateThenLaunch(server, None),
                    });
                } else {
                    self.launch_server(&server, None);
                }
            }
            Err(e) => {
                self.set_warn(format!("Could not check mods: {}. Launching anyway.", e));
                self.launch_server(&server, None);
            }
        }
    }

    pub fn launch_server(
        &mut self,
        server: &dayz_community_hub_core::Server,
        password: Option<&str>,
    ) {
        if self.launching {
            return;
        }
        self.launching = true;

        let server = server.clone();
        let password = password.map(|p| p.to_string());
        let tx = self.misc_sender();

        self.set_info("Starting Steam and launching DayZ…");

        let mut ctl_clone = self.ctl.clone_for_launch();
        tokio::spawn(async move {
            match ctl_clone.launch_game(&server, password.as_deref()).await {
                Ok(_) => {
                    let _ = tx.send(BackgroundResult::LaunchDone(server.name.clone()));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::LaunchError(format!("{}", e)));
                }
            }
        });
    }

    pub fn install_mods_for_server(&mut self, _mod_ids: Vec<u64>) {
        let server = match &self.direct_server_found {
            Some(s) => s.clone(),
            None => {
                if let Some(s) = self.get_selected_server() {
                    s
                } else {
                    self.set_error("No server selected");
                    return;
                }
            }
        };

        self.set_info(format!("Installing mods for {}...", server.name));
        self.start_background_mod_op(
            ModOperation::InstallOnly { server },
            Some(PendingAfterOp::RefreshMods),
        );
    }

    // ─── Server list refresh ───

    pub fn refresh_server_list(&mut self) {
        self.set_info("Refreshing server list...");
        let client = self.ctl.http_client().clone();
        let cache_path = config::default_data_dir().join("server_list_cache.json");
        let (tx, rx) = mpsc::unbounded_channel();
        self.bg_rx = Some(rx);
        tokio::spawn(async move {
            match api::fetch_servers(&client).await {
                Ok(list) => {
                    api::save_server_list_cache(&cache_path, &list);
                    let _ = tx.send(BackgroundResult::ServerListRefreshed(list));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::ServerListError(format!("{}", e)));
                }
            }
        });
    }

    // ─── A2S query ───

    pub fn fetch_a2s_info(&mut self) {
        let server = match self.get_selected_server() {
            Some(s) => s,
            None => return,
        };
        let selected_idx = self.selected_index();
        self.set_info(format!("Querying {}...", server.name));

        let (tx, rx) = mpsc::unbounded_channel();
        self.bg_rx = Some(rx);
        tokio::spawn(async move {
            match a2s_query::get_server_details(&server).await {
                Ok(details) => {
                    let _ = tx.send(BackgroundResult::A2sDetails(
                        Box::new(details),
                        selected_idx,
                    ));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::A2sError(format!("{}", e)));
                }
            }
        });
    }

    // ─── Direct connect ───

    pub fn handle_direct_connect(&mut self) {
        let mut ip = self.direct_address.trim().to_string();
        let mut port_str = self.direct_port.trim().to_string();

        if let Some(colon_idx) = ip.find(':') {
            let after = &ip[colon_idx + 1..];
            if after.parse::<u16>().is_ok() {
                port_str = after.to_string();
                ip = ip[..colon_idx].to_string();
                self.direct_address = ip.clone();
                self.direct_port = port_str.clone();
            }
        }

        let port = port_str.parse::<u16>().unwrap_or(2302);
        let pw = if self.direct_password.is_empty() {
            None
        } else {
            Some(self.direct_password.clone())
        };

        let server = self
            .servers
            .iter()
            .find(|s| {
                s.endpoint.ip.trim() == ip.trim()
                    && (s.endpoint.port as u16 == port || s.game_port as u16 == port)
            })
            .cloned()
            .or_else(|| {
                self.servers
                    .iter()
                    .find(|s| s.endpoint.ip.trim() == ip.trim())
                    .cloned()
                    .map(|mut s| {
                        s.game_port = port as i64;
                        s
                    })
            });

        if let Some(server) = server {
            self.direct_server_found = Some(server.clone());
            self.set_info(format!(
                "Found: {} ({}:{})",
                server.name, server.endpoint.ip, server.game_port
            ));

            match self.ctl.get_missing_mod_ids(&server) {
                Ok(missing) if !missing.is_empty() => {
                    let mod_names: Vec<String> = missing
                        .iter()
                        .take(10)
                        .map(|id| {
                            server
                                .mods
                                .iter()
                                .find(|m| m.steam_workshop_id as u64 == *id)
                                .map(|m| format!("  {} ({})", m.name, id))
                                .unwrap_or_else(|| format!("  {}", id))
                        })
                        .collect();
                    let extra = if missing.len() > 10 {
                        format!("\n  ...and {} more", missing.len() - 10)
                    } else {
                        String::new()
                    };
                    self.popup = Some(Popup::Confirm {
                        title: format!("Missing {} Mods", missing.len()),
                        message: format!(
                            "Server: {}\nInstall missing mods?\n{}{}",
                            server.name,
                            mod_names.join("\n"),
                            extra,
                        ),
                        action: ConfirmAction::InstallMods(missing),
                    });
                }
                Ok(_) if !server.mods.is_empty() => {
                    self.popup = Some(Popup::Confirm {
                        title: "Update mods?".to_string(),
                        message: format!(
                            "Server: {}\n{} mods installed. Update before launching?",
                            server.name,
                            server.mods.len()
                        ),
                        action: ConfirmAction::UpdateThenLaunch(server, pw),
                    });
                }
                Ok(_) => {
                    self.popup = Some(Popup::Confirm {
                        title: "Launch".to_string(),
                        message: format!("Connect to {}?", server.name),
                        action: ConfirmAction::LaunchDirect(server, pw),
                    });
                }
                Err(e) => {
                    self.set_warn(format!("Could not check mods: {}", e));
                    self.popup = Some(Popup::Confirm {
                        title: "Launch anyway?".to_string(),
                        message: format!(
                            "Could not check mods.\nConnect to {} anyway?",
                            server.name
                        ),
                        action: ConfirmAction::LaunchDirect(server, pw),
                    });
                }
            }
        } else {
            let temp_server = dayz_community_hub_core::Server {
                endpoint: dayz_community_hub_core::Endpoint {
                    ip: ip.clone(),
                    port: port as i64,
                },
                name: format!("{}:{}", ip, port),
                game_port: port as i64,
                mods: vec![],
                ..Default::default()
            };
            self.set_warn(format!("Not found in server list: {}:{}", ip, port));
            self.popup = Some(Popup::Confirm {
                title: "Server not in list".to_string(),
                message: format!(
                    "{}:{} not found in server list.\nMod info unavailable.\nConnect anyway?",
                    ip, port
                ),
                action: ConfirmAction::LaunchDirect(temp_server, pw),
            });
        }
    }

    pub fn fetch_direct_server_info(&mut self) {
        let mut ip = self.direct_address.trim().to_string();
        let mut port_str = self.direct_port.trim().to_string();

        if let Some(colon_idx) = ip.find(':') {
            let after = &ip[colon_idx + 1..];
            if after.parse::<u16>().is_ok() {
                port_str = after.to_string();
                ip = ip[..colon_idx].to_string();
                self.direct_address = ip.clone();
                self.direct_port = port_str.clone();
            }
        }

        if ip.is_empty() {
            self.set_error("Enter a server address first");
            return;
        }

        let port = port_str.parse::<u16>().unwrap_or(2302);

        let server = dayz_community_hub_core::Server {
            name: format!("{}:{}", ip, port),
            endpoint: dayz_community_hub_core::Endpoint {
                ip: ip.clone(),
                port: port as i64,
            },
            game_port: port as i64,
            ..Default::default()
        };

        self.set_info(format!("Querying {}:{}...", ip, port));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.bg_rx = Some(rx);
        tokio::spawn(async move {
            match a2s_query::get_server_details(&server).await {
                Ok(details) => {
                    let _ = tx.send(BackgroundResult::A2sDetails(Box::new(details), usize::MAX));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::A2sError(format!("{}", e)));
                }
            }
        });
    }

    pub fn handle_direct_input(&mut self, key: char) {
        match self.direct_cursor {
            DirectConnectField::Address => self.direct_address.push(key),
            DirectConnectField::Port => {
                if key.is_ascii_digit() {
                    self.direct_port.push(key);
                }
            }
            DirectConnectField::Password => self.direct_password.push(key),
            DirectConnectField::ServerInfo | DirectConnectField::Connect => {}
        }
    }

    pub fn handle_direct_backspace(&mut self) {
        match self.direct_cursor {
            DirectConnectField::Address => {
                self.direct_address.pop();
            }
            DirectConnectField::Port => {
                self.direct_port.pop();
            }
            DirectConnectField::Password => {
                self.direct_password.pop();
            }
            DirectConnectField::ServerInfo | DirectConnectField::Connect => {}
        }
    }

    // ─── Mod actions ───

    pub fn update_selected_mod(&mut self) {
        if self.tab != Tab::Mods || self.is_busy() {
            return;
        }
        let mods = match &self.installed_mods {
            Some(m) => m,
            None => return,
        };
        let idx = self.selected_index();
        if let Some(mod_) = mods.get(idx) {
            let name = mod_.name.clone();
            let id = mod_.id;
            self.set_info(format!("Updating {}...", name));
            self.start_background_mod_op(
                ModOperation::UpdateOne { mod_id: id, name },
                Some(PendingAfterOp::RefreshMods),
            );
        }
    }

    pub fn update_all_installed_mods(&mut self) {
        if self.is_busy() {
            return;
        }
        self.set_info("Updating all mods...");
        self.start_background_mod_op(ModOperation::UpdateAll, Some(PendingAfterOp::RefreshMods));
    }

    pub fn delete_selected_mod(&mut self) {
        if self.tab != Tab::Mods {
            return;
        }
        let mods = match &self.installed_mods {
            Some(m) => m,
            None => return,
        };
        let idx = self.selected_index();
        if let Some(mod_) = mods.get(idx) {
            self.popup = Some(Popup::Confirm {
                title: "Delete Mod".to_string(),
                message: format!(
                    "Delete '{}' ({})?\nSize: {}",
                    mod_.name,
                    mod_.id,
                    mods::format_size(mod_.size)
                ),
                action: ConfirmAction::DeleteMod(mod_.id),
            });
        }
    }

    pub fn toggle_selected_mod_managed(&mut self) {
        if self.tab != Tab::Mods {
            return;
        }
        let mods = match &self.installed_mods {
            Some(m) => m,
            None => return,
        };
        let idx = self.selected_index();
        if let Some(mod_) = mods.get(idx) {
            match self.ctl.toggle_mod_managed(mod_.id) {
                Ok(managed) => {
                    let status = if managed { "managed" } else { "unmanaged" };
                    self.set_info(format!("{} marked as {}", mod_.name, status));
                    self.refresh_installed_mods();
                }
                Err(e) => self.set_error(format!("Toggle failed: {}", e)),
            }
        }
    }

    pub fn cleanup_mods(&mut self) {
        self.popup = Some(Popup::Confirm {
            title: "Cleanup Mods".to_string(),
            message: "Remove all managed mods and symlinks?".to_string(),
            action: ConfirmAction::CleanupMods,
        });
    }

    // ─── News ───

    pub fn open_selected_news_url(&mut self) {
        let articles = match self.news_articles.as_deref() {
            Some(a) => a,
            None => return,
        };
        let idx = self.selected_index();
        if let Some(article) = articles.get(idx) {
            let url = article.url();
            match std::process::Command::new("xdg-open")
                .arg(&url)
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => self.set_info(format!("Opening: {}", url)),
                Err(e) => self.set_error(format!("Failed to open browser: {}", e)),
            }
        }
    }

    // ─── Offline mode ───

    pub fn offline_mode(&self) -> Option<OfflineMode> {
        self.ctl
            .dayz_path()
            .ok()
            .map(|p| OfflineMode::new(p, self.ctl.http_client().clone()))
    }

    pub fn refresh_offline_missions(&mut self) {
        match self.offline_mode() {
            None => {
                self.offline_status = Some("SteamCMD / DayZ path not configured".to_string());
                self.offline_status_color = Color::Red;
                self.offline_missions = Some(vec![]);
            }
            Some(om) => match om.get_available_missions() {
                Ok(missions) => {
                    let count = missions.len();
                    self.offline_missions = Some(missions);
                    if count == 0 {
                        self.offline_status =
                            Some("No missions found. Press 'u' to install.".to_string());
                        self.offline_status_color = Color::Yellow;
                    } else {
                        self.offline_status = Some(format!("{} mission(s) available", count));
                        self.offline_status_color = Color::Green;
                    }
                }
                Err(e) => {
                    self.offline_status = Some(format!("Error reading missions: {}", e));
                    self.offline_status_color = Color::Red;
                    self.offline_missions = Some(vec![]);
                }
            },
        }
    }

    pub fn update_offline_mode(&mut self) {
        let om = match self.offline_mode() {
            Some(o) => o,
            None => {
                self.set_error("DayZ path not configured — cannot install offline mode");
                return;
            }
        };
        self.set_info("Downloading DayZCommunityOfflineMode…");
        let tx = self.misc_sender();
        tokio::spawn(async move {
            match om.update().await {
                Ok(()) => {
                    let _ = tx.send(BackgroundResult::OfflineModeUpdated);
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::OfflineModeError(format!("{}", e)));
                }
            }
        });
    }

    pub fn launch_offline_mission(&mut self) {
        if self.launching {
            return;
        }
        let missions = match self.offline_missions.as_ref() {
            Some(m) if !m.is_empty() => m,
            _ => {
                self.set_error("No missions available. Press 'u' to install offline mode.");
                return;
            }
        };
        let idx = self.selected_indices[Tab::Offline.index()];
        let mission = match missions.get(idx) {
            Some(m) => m.clone(),
            None => return,
        };

        let dayz_path = match self.ctl.dayz_path() {
            Ok(p) => p,
            Err(e) => {
                self.set_error(format!("DayZ path error: {}", e));
                return;
            }
        };

        let om = OfflineMode::new(dayz_path, self.ctl.http_client().clone());
        let dayz_args = om.build_launch_args(&mission, &[], false);

        let steam_args = dayz_community_hub_core::launch::build_steam_applaunch_args(
            dayz_community_hub_core::steamcmd::DAYZ_GAME_ID,
            &dayz_args,
            self.ctl.profile().player.as_deref(),
        );

        if let Err(e) = dayz_community_hub_core::steamcmd::SteamClient::start() {
            self.set_error(format!("Could not start Steam: {}", e));
            return;
        }

        match std::process::Command::new("steam")
            .args(&steam_args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                self.set_success(format!("Launching offline mission: {}", mission));
            }
            Err(e) => {
                self.set_error(format!("Launch failed: {}", e));
            }
        }
    }

    // ─── Options ───

    pub fn toggle_option_at_index(&mut self) {
        if self.tab != Tab::Options {
            return;
        }
        let idx = self.selected_index();
        let (name, new_state) = {
            let options = &mut self.ctl.profile_mut().options;
            let mut all = options.all_options_mut();
            if let Some((name, opt)) = all.get_mut(idx) {
                opt.enabled = !opt.enabled;
                (name.to_string(), opt.enabled)
            } else {
                return;
            }
        };
        let state = if new_state { "enabled" } else { "disabled" };
        self.set_info(format!("{}: {}", name, state));
        let _ = self.ctl.save_profile();
    }

    pub fn start_option_edit(&mut self) {
        if self.tab != Tab::Options {
            return;
        }
        let idx = self.selected_index();
        let current_value = {
            let options = &self.ctl.profile().options;
            let all = options.all_options();
            all.get(idx)
                .and_then(|(_, opt)| opt.value.clone())
                .unwrap_or_default()
        };
        self.option_edit_active = true;
        self.option_edit_value = current_value;
        self.set_info("Edit value: type new value, Enter to save, Esc to cancel");
    }

    pub fn apply_option_edit(&mut self) {
        let idx = self.selected_index();
        let (name, new_value) = {
            let value = if self.option_edit_value.trim().is_empty() {
                None
            } else {
                Some(self.option_edit_value.trim().to_string())
            };
            let options = &mut self.ctl.profile_mut().options;
            let mut all = options.all_options_mut();
            if let Some((name, opt)) = all.get_mut(idx) {
                let n = name.to_string();
                opt.value = value.clone();
                if value.is_some() {
                    opt.enabled = true;
                }
                (n, value)
            } else {
                return;
            }
        };
        self.option_edit_active = false;
        self.option_edit_value.clear();
        match new_value {
            Some(v) => self.set_success(format!("{} = {}", name, v)),
            None => self.set_info(format!("{}: value cleared", name)),
        }
        let _ = self.ctl.save_profile();
    }

    pub fn cancel_option_edit(&mut self) {
        self.option_edit_active = false;
        self.option_edit_value.clear();
        self.set_info("Edit cancelled");
    }

    // ─── Confirm actions ───

    pub fn execute_confirm_action(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DeleteMod(id) => match self.ctl.delete_mod(id, false) {
                Ok(_) => {
                    self.set_success(format!("Deleted mod {}", id));
                    self.refresh_installed_mods();
                }
                Err(e) => self.set_error(format!("Delete failed: {}", e)),
            },
            ConfirmAction::CleanupMods => match self.ctl.cleanup_mods() {
                Ok(stats) => {
                    self.set_success(format!(
                        "Cleanup: {} mods ({}) + {} symlinks removed",
                        stats.removed_count,
                        mods::format_size(stats.removed_size),
                        stats.symlinks_removed
                    ));
                    self.refresh_installed_mods();
                }
                Err(e) => self.set_error(format!("Cleanup failed: {}", e)),
            },
            ConfirmAction::InstallMods(mod_ids) => {
                self.install_mods_for_server(mod_ids);
            }
            ConfirmAction::RemoveFavorite(ip, port) => {
                self.ctl.remove_favorite(&ip, port);
                if let Err(e) = self.ctl.save_profile() {
                    self.set_error(format!("Save failed: {}", e));
                } else {
                    self.set_success("Removed from favorites");
                }
            }
            ConfirmAction::RemoveHistoryEntry(ip, port) => {
                self.ctl
                    .profile_mut()
                    .history
                    .retain(|h| h.ip != ip || h.port != port);
                if let Err(e) = self.ctl.save_profile() {
                    self.set_error(format!("Save failed: {}", e));
                } else {
                    self.set_success("Removed from history");
                    let len = self.ctl.profile().history.len();
                    if len > 0 && self.selected_index() >= len {
                        *self.selected_index_mut() = len - 1;
                    }
                }
            }
            ConfirmAction::ClearHistory => {
                self.ctl.profile_mut().history.clear();
                if let Err(e) = self.ctl.save_profile() {
                    self.set_error(format!("Save failed: {}", e));
                } else {
                    self.set_success("History cleared");
                    *self.selected_index_mut() = 0;
                    *self.offset_mut() = 0;
                }
            }
            ConfirmAction::UpdateThenLaunch(server, pw) => {
                self.set_info(format!(
                    "Updating {} mods for {}...",
                    server.mods.len(),
                    server.name
                ));
                self.start_background_mod_op(
                    ModOperation::UpdateThenLaunch {
                        server: server.clone(),
                        password: pw.clone(),
                    },
                    Some(PendingAfterOp::LaunchServer(server, pw)),
                );
            }
            ConfirmAction::LaunchDirect(server, pw) => {
                if let Err(e) = self.ctl.setup_mod_symlinks(&server) {
                    // non-fatal: log to status bar instead of swallowing silently
                    self.set_warn(format!("Symlink setup warning: {}", e));
                }
                self.launch_server(&server, pw.as_deref());
            }
        }
    }

    // ─── Search ───

    pub fn update_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = None;
            return;
        }
        let query = self.search_query.to_lowercase();
        let indices: Vec<usize> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.name.to_lowercase().contains(&query)
                    || s.endpoint.ip.contains(&query)
                    || s.map.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        self.filtered_indices = Some(indices);
        *self.selected_index_mut() = 0;
        *self.offset_mut() = 0;
    }

    // ─── Background mod operation ───

    pub fn start_background_mod_op(&mut self, op: ModOperation, after: Option<PendingAfterOp>) {
        match self.ctl.start_mod_operation(op) {
            Ok((rx, handle)) => {
                self.progress_rx = Some(rx);
                self.progress_handle = Some(handle);
                self.progress_state = Some(ProgressState {
                    current: 0,
                    total: 0,
                    current_mod_name: "Preparing...".to_string(),
                    current_mod_id: 0,
                    phase: ProgressPhase::Downloading,
                    completed: Vec::new(),
                });
                self.pending_after_op = after;
                self.loading = true;
            }
            Err(e) => {
                let msg = format!("{}", e);
                if msg.contains("login") || msg.contains("SteamCMD") {
                    if let Some(login) = self.ctl.steamcmd_login() {
                        self.set_error(format!("{}. Try: steamcmd +login {} +quit", msg, login));
                        return;
                    }
                }
                self.set_error(format!("Failed to start: {}", e));
            }
        }
    }

    pub fn poll_progress(&mut self) -> bool {
        let rx = match &mut self.progress_rx {
            Some(rx) => rx,
            None => return false,
        };

        loop {
            match rx.try_recv() {
                Ok(msg) => match msg {
                    ModProgress::ShuttingDownSteam => {
                        let state = self.progress_state.get_or_insert(ProgressState {
                            current: 0,
                            total: 0,
                            current_mod_name: String::new(),
                            current_mod_id: 0,
                            phase: ProgressPhase::ShuttingDownSteam,
                            completed: Vec::new(),
                        });
                        state.phase = ProgressPhase::ShuttingDownSteam;
                    }
                    ModProgress::Starting {
                        current,
                        total,
                        mod_id,
                        name,
                    } => {
                        let state = self.progress_state.get_or_insert(ProgressState {
                            current: 0,
                            total: 0,
                            current_mod_name: String::new(),
                            current_mod_id: 0,
                            phase: ProgressPhase::Downloading,
                            completed: Vec::new(),
                        });
                        state.current = current;
                        state.total = total;
                        state.current_mod_name = name;
                        state.current_mod_id = mod_id;
                        state.phase = ProgressPhase::Downloading;
                    }
                    ModProgress::Done {
                        current,
                        total,
                        mod_id,
                        name,
                    } => {
                        if let Some(state) = &mut self.progress_state {
                            state.current = current;
                            state.total = total;
                            state.completed.push((mod_id, name, true));
                        }
                    }
                    ModProgress::Failed {
                        current,
                        total,
                        mod_id,
                        name,
                        error: _,
                    } => {
                        if let Some(state) = &mut self.progress_state {
                            state.current = current;
                            state.total = total;
                            state.completed.push((mod_id, name, false));
                        }
                    }
                    ModProgress::Finished {
                        ok,
                        failed,
                        total,
                        hint,
                    } => {
                        if let Some(state) = &mut self.progress_state {
                            state.phase = ProgressPhase::Finished {
                                ok,
                                failed,
                                hint: hint.clone(),
                            };
                            state.total = total;
                        }
                        self.loading = false;
                        self.progress_rx = None;

                        if let Some(ref hint) = hint {
                            if hint.contains("Cached credentials not found") {
                                let cmd = hint
                                    .lines()
                                    .last()
                                    .map(|l| l.trim())
                                    .unwrap_or("steamcmd +login <user> +quit");
                                self.popup = Some(Popup::Info {
                                    title: "SteamCMD Login Required".to_string(),
                                    message: format!(
                                        "Your steamcmd credentials are missing or expired.\n\
                                             No mods were downloaded.\n\n\
                                             Run this command in a terminal, then try again:\n\n\
                                             {}\n\n\
                                             Press any key to dismiss.",
                                        cmd
                                    ),
                                });
                            }
                            self.set_error("steamcmd credentials expired — see popup");
                        } else if failed == 0 {
                            if total == 0 {
                                self.set_success("All mods already up to date");
                            } else {
                                self.set_success(format!("Completed: {} mods OK", ok));
                            }
                        } else {
                            self.set_warn(format!("Completed: {} OK, {} failed", ok, failed));
                        }

                        let after = self.pending_after_op.take();
                        self.refresh_installed_mods();

                        if hint.is_none() {
                            if let Some(pending) = after {
                                match pending {
                                    PendingAfterOp::LaunchServer(server, pw) => {
                                        if let Err(e) = self.ctl.setup_mod_symlinks(&server) {
                                            self.set_warn(format!("Symlink warning: {}", e));
                                        }
                                        self.launch_server(&server, pw.as_deref());
                                    }
                                    PendingAfterOp::RefreshMods => {}
                                }
                            }
                        }

                        self.progress_state = None;
                        return false;
                    }
                },
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.progress_rx = None;
                    self.progress_state = None;
                    self.set_error("Operation ended unexpectedly");
                    return false;
                }
            }
        }

        true
    }

    pub fn is_busy(&self) -> bool {
        self.progress_rx.is_some()
    }

    // ─── Ping ───

    pub fn start_background_ping(&mut self) {
        let priority_keys: std::collections::HashSet<String> = self
            .ctl
            .profile()
            .favorites
            .iter()
            .map(|f| format!("{}:{}", f.ip, f.port))
            .chain(
                self.ctl
                    .profile()
                    .history
                    .iter()
                    .map(|h| format!("{}:{}", h.ip, h.port)),
            )
            .collect();

        let (mut priority, rest): (Vec<_>, Vec<_>) = self.servers.iter().cloned().partition(|s| {
            let key = format!("{}:{}", s.endpoint.ip, s.endpoint.port);
            priority_keys.contains(&key)
        });

        priority.extend(rest);
        let ordered_servers = priority;

        let (tx, rx) = mpsc::unbounded_channel();
        self.ping_rx = Some(rx);

        tokio::spawn(async move {
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(50));
            let mut handles = Vec::new();

            for server in ordered_servers {
                let tx = tx.clone();
                let sem = semaphore.clone();
                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    let key = format!("{}:{}", server.endpoint.ip, server.endpoint.port);
                    if let Ok(ping_ms) = a2s_query::ping_server(&server).await {
                        let _ = tx.send((key, ping_ms));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    }

    pub fn poll_pings(&mut self) {
        let rx = match &mut self.ping_rx {
            Some(rx) => rx,
            None => return,
        };

        loop {
            match rx.try_recv() {
                Ok((key, ping_ms)) => {
                    self.ping_cache.insert(key, ping_ms);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.ping_rx = None;
                    break;
                }
            }
        }
    }

    // ─── Background results ───

    pub fn poll_background(&mut self) {
        let rx = match &mut self.bg_rx {
            Some(rx) => rx,
            None => return,
        };

        loop {
            match rx.try_recv() {
                Ok(result) => {
                    match result {
                        BackgroundResult::A2sDetails(details, idx) => {
                            self.current_a2s_details = Some(*details);
                            self.detailed_server_index = Some(idx);
                            self.show_server_details = true;
                            self.set_success("A2S info loaded");
                            self.bg_rx = None;
                        }
                        BackgroundResult::A2sError(e) => {
                            self.set_error(format!("A2S query failed: {}", e));
                            self.current_a2s_details = None;
                            self.show_server_details = false;
                            self.bg_rx = None;
                        }
                        BackgroundResult::ServerListRefreshed(list) => {
                            let count = list.result.len();
                            self.servers = list.result.clone();
                            if self.filtered_indices.is_some() {
                                self.update_search_filter();
                            }
                            self.ping_cache.clear();
                            self.start_background_ping();
                            self.set_success(format!("Refreshed: {} servers loaded", count));
                            self.bg_rx = None;
                        }
                        BackgroundResult::ServerListError(e) => {
                            self.set_error(format!("Refresh failed: {}", e));
                            self.bg_rx = None;
                        }
                        BackgroundResult::SteamPlayerCount(n) => {
                            self.steam_players = Some(n);
                            self.bg_rx = None;
                        }
                        BackgroundResult::NewsRefreshed(articles) => {
                            self.news_articles = Some(articles);
                            self.news_fetched_at = Some(std::time::Instant::now());
                            self.bg_rx = None;
                        }
                        BackgroundResult::NewsError(e) => {
                            self.set_error(format!("News fetch failed: {}", e));
                            self.bg_rx = None;
                        }
                        BackgroundResult::OfflineModeUpdated => {
                            self.set_success("Offline mode installed/updated successfully");
                            self.refresh_offline_missions();
                            self.bg_rx = None;
                        }
                        BackgroundResult::OfflineModeError(e) => {
                            self.set_error(format!("Offline mode update failed: {}", e));
                            self.bg_rx = None;
                        }
                        BackgroundResult::LaunchDone(name) => {
                            self.launching = false;
                            self.set_success(format!("Launched DayZ -> {}", name));
                            self.bg_rx = None;
                        }
                        BackgroundResult::LaunchError(e) => {
                            self.launching = false;
                            self.set_error(format!("Launch failed: {}", e));
                            self.bg_rx = None;
                        }
                    }
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.bg_rx = None;
                    break;
                }
            }
        }
    }

    pub fn misc_sender(&mut self) -> mpsc::UnboundedSender<BackgroundResult> {
        if self.misc_tx.is_none() {
            let (tx, rx) = mpsc::unbounded_channel();
            self.misc_tx = Some(tx);
            self.misc_rx = Some(rx);
        }
        self.misc_tx.as_ref().unwrap().clone()
    }

    pub fn poll_misc(&mut self) {
        loop {
            let result = match self.misc_rx.as_mut() {
                Some(rx) => match rx.try_recv() {
                    Ok(r) => r,
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.misc_rx = None;
                        self.misc_tx = None;
                        break;
                    }
                },
                None => break,
            };
            match result {
                BackgroundResult::SteamPlayerCount(n) => {
                    self.steam_players = Some(n);
                }
                BackgroundResult::NewsRefreshed(articles) => {
                    self.news_articles = Some(articles);
                    self.news_fetched_at = Some(std::time::Instant::now());
                }
                BackgroundResult::NewsError(e) => {
                    self.set_error(format!("News fetch failed: {}", e));
                }
                BackgroundResult::OfflineModeUpdated => {
                    self.set_success("Offline mode installed/updated successfully");
                    self.refresh_offline_missions();
                }
                BackgroundResult::OfflineModeError(e) => {
                    self.set_error(format!("Offline mode update failed: {}", e));
                }
                BackgroundResult::LaunchDone(name) => {
                    self.launching = false;
                    self.set_success(format!("Launched DayZ -> {}", name));
                }
                BackgroundResult::LaunchError(e) => {
                    self.launching = false;
                    self.set_error(format!("Launch failed: {}", e));
                }
                _ => {}
            }
        }
    }

    pub fn fetch_steam_players(&mut self) {
        let tx = self.misc_sender();
        let client = self.ctl.http_client().clone();
        tokio::spawn(async move {
            if let Ok(count) = api::fetch_steam_player_count(&client).await {
                let _ = tx.send(BackgroundResult::SteamPlayerCount(count));
            }
        });
    }

    pub fn fetch_news_if_needed(&mut self) {
        if let Some(fetched_at) = self.news_fetched_at {
            if fetched_at.elapsed().as_secs() < news::NEWS_CACHE_TTL_SECS {
                return;
            }
        }
        let tx = self.misc_sender();
        let client = self.ctl.http_client().clone();
        tokio::spawn(async move {
            match news::fetch_news(&client).await {
                Ok(articles) => {
                    let _ = tx.send(BackgroundResult::NewsRefreshed(articles));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundResult::NewsError(format!("{}", e)));
                }
            }
        });
    }
}

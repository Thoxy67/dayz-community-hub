use dayzsa_ml::{
    Result, a2s_query, config,
    ctl::{DayzCtl, ModOpResult, ModOperation},
    mods, steamcmd,
    steamcmd::ModProgress,
    system, utils,
};
use ratatui::{
    Frame, Terminal,
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};
use std::io;
use std::io::Write;
use termion::input::TermRead;
use termion::{event::Key, raw::IntoRawMode};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

// ─── Application State ────────────────────────────────────────────────────

struct App {
    ctl: DayzCtl,
    servers: Vec<dayzsa_ml::Server>,
    selected_indices: [usize; 7],
    offsets: [usize; 7],
    tab: Tab,
    status_message: Option<String>,
    status_color: Color,
    // Direct connect
    direct_address: String,
    direct_port: String,
    direct_password: String,
    direct_cursor: DirectConnectField,
    direct_server_found: Option<dayzsa_ml::Server>,
    // Mods
    installed_mods: Option<Vec<mods::InstalledMod>>,
    #[allow(dead_code)]
    pending_deletion: Option<u64>,
    #[allow(dead_code)]
    pending_cleanup: bool,
    // Server details
    current_a2s_details: Option<a2s_query::ServerDetails>,
    detailed_server_index: Option<usize>,
    show_server_details: bool,
    details_scroll_offset: u16,
    // Search
    search_query: String,
    search_active: bool,
    filtered_indices: Option<Vec<usize>>,
    // Option value editing
    option_edit_active: bool,
    option_edit_value: String,
    // Popup
    popup: Option<Popup>,
    // Terminal size
    term_height: u16,
    // Loading indicator
    loading: bool,
    // Background mod operation progress
    progress_rx: Option<mpsc::UnboundedReceiver<ModProgress>>,
    progress_handle: Option<tokio::task::JoinHandle<ModOpResult>>,
    progress_state: Option<ProgressState>,
    // What to do after a background operation finishes
    pending_after_op: Option<PendingAfterOp>,
}

/// Tracks the live progress of a background mod operation.
#[derive(Clone)]
struct ProgressState {
    current: usize,
    total: usize,
    current_mod_name: String,
    current_mod_id: u64,
    phase: ProgressPhase,
    completed: Vec<(u64, String, bool)>, // (id, name, success)
}

#[derive(Clone, PartialEq)]
enum ProgressPhase {
    Downloading,
    Finished {
        ok: usize,
        failed: usize,
        hint: Option<String>,
    },
}

/// Action to perform after a background mod operation completes.
#[derive(Clone)]
enum PendingAfterOp {
    LaunchServer(dayzsa_ml::Server, Option<String>),
    RefreshMods,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Servers,
    Favorites,
    History,
    Mods,
    DirectConnect,
    Options,
}

#[derive(Clone, Copy, PartialEq)]
enum DirectConnectField {
    Address,
    Port,
    Password,
    Connect,
}

#[derive(Clone)]
enum Popup {
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
    },
    #[allow(dead_code)]
    Info { title: String, message: String },
}

#[derive(Clone)]
enum ConfirmAction {
    DeleteMod(u64),
    CleanupMods,
    InstallMods(Vec<u64>),
    RemoveFavorite(String, u16),
    RemoveHistoryEntry(String, u16),
    ClearHistory,
    UpdateThenLaunch(dayzsa_ml::Server, Option<String>),
    LaunchDirect(dayzsa_ml::Server, Option<String>),
}

impl Tab {
    fn index(&self) -> usize {
        match self {
            Tab::Servers => 0,
            Tab::Favorites => 1,
            Tab::History => 2,
            Tab::Mods => 3,
            Tab::DirectConnect => 4,
            Tab::Options => 5,
        }
    }

    fn label(&self) -> &str {
        match self {
            Tab::Servers => "Servers",
            Tab::Favorites => "Favorites",
            Tab::History => "History",
            Tab::Mods => "Mods",
            Tab::DirectConnect => "Connect",
            Tab::Options => "Options",
        }
    }

    fn all() -> &'static [Tab] {
        &[
            Tab::Servers,
            Tab::Favorites,
            Tab::History,
            Tab::Mods,
            Tab::DirectConnect,
            Tab::Options,
        ]
    }

    fn next(&self) -> Tab {
        let all = Self::all();
        let idx = self.index();
        all[(idx + 1) % all.len()]
    }

    fn prev(&self) -> Tab {
        let all = Self::all();
        let idx = self.index();
        all[(idx + all.len() - 1) % all.len()]
    }
}

impl App {
    fn new(ctl: DayzCtl, servers: Vec<dayzsa_ml::Server>) -> Self {
        Self {
            ctl,
            servers,
            selected_indices: [0; 7],
            offsets: [0; 7],
            tab: Tab::Servers,
            status_message: None,
            status_color: Color::Cyan,
            direct_address: String::new(),
            direct_port: "2302".to_string(),
            direct_password: String::new(),
            direct_cursor: DirectConnectField::Address,
            direct_server_found: None,
            installed_mods: None,
            pending_deletion: None,
            pending_cleanup: false,
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
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, color: Color) {
        self.status_message = Some(msg.into());
        self.status_color = color;
    }

    fn set_info(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Cyan);
    }

    fn set_success(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Green);
    }

    fn set_error(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Red);
    }

    fn set_warn(&mut self, msg: impl Into<String>) {
        self.set_status(msg, Color::Yellow);
    }

    fn refresh_installed_mods(&mut self) {
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

    fn selected_index(&self) -> usize {
        self.selected_indices[self.tab.index()]
    }

    fn selected_index_mut(&mut self) -> &mut usize {
        &mut self.selected_indices[self.tab.index()]
    }

    fn offset(&self) -> usize {
        self.offsets[self.tab.index()]
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offsets[self.tab.index()]
    }

    fn visible_items(&self) -> usize {
        // Account for borders, tabs, title, status bar
        let overhead = 12u16;
        let available = self.term_height.saturating_sub(overhead);
        // Each server item takes ~2 lines, mods ~2 lines
        let lines_per_item = 2;
        (available / lines_per_item).max(3) as usize
    }

    fn current_list_len(&self) -> usize {
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
            Tab::DirectConnect => 4,
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.tab == Tab::DirectConnect {
            let current = self.direct_cursor as u8;
            let new = (current as i32 + delta).rem_euclid(4) as u8;
            self.direct_cursor = match new {
                0 => DirectConnectField::Address,
                1 => DirectConnectField::Port,
                2 => DirectConnectField::Password,
                3 => DirectConnectField::Connect,
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

        // Adjust scroll offset
        let visible = self.visible_items();
        let offset = self.offset_mut();
        if new_index < *offset {
            *offset = new_index;
        } else if new_index >= *offset + visible {
            *offset = new_index - visible + 1;
        }
    }

    fn page_up(&mut self) {
        let visible = self.visible_items() as i32;
        self.move_selection(-visible);
    }

    fn page_down(&mut self) {
        let visible = self.visible_items() as i32;
        self.move_selection(visible);
    }

    // ─── Server Actions ───

    fn get_selected_server(&self) -> Option<dayzsa_ml::Server> {
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

    fn add_favorite(&mut self) {
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

    fn remove_favorite_at_index(&mut self) {
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

    fn remove_history_entry_at_index(&mut self) {
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

    fn clear_history(&mut self) {
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

    fn connect_to_selected(&mut self) {
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

        // Check mods
        match self.ctl.get_missing_mod_ids(&server) {
            Ok(missing) => {
                if !missing.is_empty() {
                    // Show missing mods and offer to install
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
                    // All mods present -- ask to update before launch
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
                    // No mods needed, just launch
                    self.launch_server(&server, None);
                }
            }
            Err(e) => {
                self.set_warn(format!("Could not check mods: {}. Launching anyway.", e));
                self.launch_server(&server, None);
            }
        }
    }

    fn launch_server(&mut self, server: &dayzsa_ml::Server, password: Option<&str>) {
        match tokio::task::block_in_place(|| {
            Handle::current().block_on(self.ctl.launch_game(server, password))
        }) {
            Ok(_) => {
                self.set_success(format!("Launching DayZ -> {}", server.name));
            }
            Err(e) => {
                self.set_error(format!("Launch failed: {}", e));
            }
        }
    }

    fn install_mods_for_server(&mut self, _mod_ids: Vec<u64>) {
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

    // ─── Server List Refresh ───

    fn refresh_server_list(&mut self) {
        self.set_info("Refreshing server list...");
        match tokio::task::block_in_place(|| Handle::current().block_on(self.ctl.fetch_servers())) {
            Ok(list) => {
                let count = list.result.len();
                self.servers = list.result.clone();
                // Re-apply search filter if active
                if self.filtered_indices.is_some() {
                    self.update_search_filter();
                }
                self.set_success(format!("Refreshed: {} servers loaded", count));
            }
            Err(e) => {
                self.set_error(format!("Refresh failed: {}", e));
            }
        }
    }

    // ─── A2S Query ───

    fn fetch_a2s_info(&mut self) {
        let server = match self.get_selected_server() {
            Some(s) => s,
            None => return,
        };

        self.set_info(format!("Querying {}...", server.name));

        match tokio::task::block_in_place(|| {
            Handle::current().block_on(a2s_query::get_server_details(&server))
        }) {
            Ok(details) => {
                self.current_a2s_details = Some(details);
                self.detailed_server_index = Some(self.selected_index());
                self.show_server_details = true;
                self.set_success("A2S info loaded");
            }
            Err(e) => {
                self.set_error(format!("A2S query failed: {}", e));
                self.current_a2s_details = None;
                self.show_server_details = false;
            }
        }
    }

    // ─── Direct Connect ───

    fn handle_direct_connect(&mut self) {
        let mut ip = self.direct_address.trim().to_string();
        let mut port_str = self.direct_port.trim().to_string();

        // Handle IP:PORT format
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

        // Look up server in the server list (exact match first, then IP-only)
        let server = self
            .servers
            .iter()
            .find(|s| {
                s.endpoint.ip.trim() == ip.trim()
                    && (s.endpoint.port as u16 == port || s.game_port as u16 == port)
            })
            .cloned()
            .or_else(|| {
                // Fallback: IP-only match (take first server with this IP)
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
                    // All mods installed -- ask to update before launch
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
                    // No mods or can't check -- launch directly
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
            // Not found in server list
            let temp_server = dayzsa_ml::Server {
                endpoint: dayzsa_ml::Endpoint {
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

    fn handle_direct_input(&mut self, key: char) {
        match self.direct_cursor {
            DirectConnectField::Address => self.direct_address.push(key),
            DirectConnectField::Port => {
                if key.is_ascii_digit() {
                    self.direct_port.push(key);
                }
            }
            DirectConnectField::Password => self.direct_password.push(key),
            DirectConnectField::Connect => {}
        }
    }

    fn handle_direct_backspace(&mut self) {
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
            DirectConnectField::Connect => {}
        }
    }

    // ─── Mod Actions ───

    fn update_selected_mod(&mut self) {
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

    fn update_all_installed_mods(&mut self) {
        if self.is_busy() {
            return;
        }
        self.set_info("Updating all mods...");
        self.start_background_mod_op(ModOperation::UpdateAll, Some(PendingAfterOp::RefreshMods));
    }

    fn delete_selected_mod(&mut self) {
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

    fn toggle_selected_mod_managed(&mut self) {
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

    fn cleanup_mods(&mut self) {
        self.popup = Some(Popup::Confirm {
            title: "Cleanup Mods".to_string(),
            message: "Remove all managed mods and symlinks?".to_string(),
            action: ConfirmAction::CleanupMods,
        });
    }

    fn toggle_option_at_index(&mut self) {
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

    fn start_option_edit(&mut self) {
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

    fn apply_option_edit(&mut self) {
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
                // Auto-enable when setting a value
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

    fn cancel_option_edit(&mut self) {
        self.option_edit_active = false;
        self.option_edit_value.clear();
        self.set_info("Edit cancelled");
    }

    fn execute_confirm_action(&mut self, action: ConfirmAction) {
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
                    // Adjust selection if needed
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
                // Update all server mods in background, then launch when done
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
                    // non-fatal
                    let _ = e;
                }
                self.launch_server(&server, pw.as_deref());
            }
        }
    }

    // ─── Search ───

    /// Start a background mod operation (install/update) with progress tracking.
    fn start_background_mod_op(&mut self, op: ModOperation, after: Option<PendingAfterOp>) {
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
                // Show re-login hint for steamcmd errors
                let msg = format!("{}", e);
                if msg.contains("login") || msg.contains("SteamCMD") {
                    if let Some(login) = self.ctl.steamcmd_login() {
                        self.set_error(format!(
                            "{}. Try: steamcmd +login {} +quit",
                            msg, login
                        ));
                        return;
                    }
                }
                self.set_error(format!("Failed to start: {}", e));
            }
        }
    }

    /// Poll progress channel for updates. Returns true if operation is still running.
    fn poll_progress(&mut self) -> bool {
        let rx = match &mut self.progress_rx {
            Some(rx) => rx,
            None => return false,
        };

        // Drain all available messages
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    match msg {
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
                            // Operation is done
                            self.loading = false;
                            self.progress_rx = None;

                            if let Some(ref hint) = hint {
                                self.set_error(hint.clone());
                            } else if failed == 0 {
                                if total == 0 {
                                    self.set_success("All mods already up to date");
                                } else {
                                    self.set_success(format!("Completed: {} mods OK", ok));
                                }
                            } else {
                                self.set_warn(format!("Completed: {} OK, {} failed", ok, failed));
                            }

                            // Execute pending after-op action
                            let after = self.pending_after_op.take();
                            self.refresh_installed_mods();
                            if let Some(pending) = after {
                                match pending {
                                    PendingAfterOp::LaunchServer(server, pw) => {
                                        if let Err(e) = self.ctl.setup_mod_symlinks(&server) {
                                            let _ = e;
                                        }
                                        self.launch_server(&server, pw.as_deref());
                                    }
                                    PendingAfterOp::RefreshMods => {
                                        // Already refreshed above
                                    }
                                }
                            }

                            // Clear progress state after a moment (it will be cleared on next action)
                            self.progress_state = None;
                            return false;
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed unexpectedly
                    self.loading = false;
                    self.progress_rx = None;
                    self.progress_state = None;
                    self.set_error("Operation ended unexpectedly");
                    return false;
                }
            }
        }

        true // Still running
    }

    /// Returns true if a background operation is currently in progress.
    fn is_busy(&self) -> bool {
        self.progress_rx.is_some()
    }

    fn update_search_filter(&mut self) {
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
}

// ─── Setup ────────────────────────────────────────────────────────────────

fn run_setup_if_needed(profile_path: &std::path::Path) -> Result<()> {
    let mut profile = if profile_path.exists() {
        config::Profile::load(profile_path)?
    } else {
        let mut prof = config::Profile::default_with_version("0.1.0");
        prof.path = profile_path.to_path_buf();
        prof
    };

    let mut needs_save = false;

    // Auto-detect steam root
    if profile.steam_root.is_none() {
        if let Some(root) = steamcmd::find_steam_root() {
            println!("Auto-detected Steam root: {}", root.display());
            profile.steam_root = Some(root.to_string_lossy().to_string());
            needs_save = true;
        } else {
            println!("Steam root not found. Enter path to steamapps directory:");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let path = input.trim();
            if !path.is_empty() {
                profile.steam_root = Some(path.to_string());
                needs_save = true;
            }
        }
    }

    // Steam login
    if profile.steam_login.is_none() {
        println!("Steam username for workshop downloads (or 'anonymous' to skip):");
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let login = input.trim();
        if login.is_empty() {
            profile.steam_login = Some("anonymous".to_string());
        } else {
            profile.steam_login = Some(login.to_string());
        }
        needs_save = true;
    }

    // Player name
    if profile.player.is_none() {
        println!("Player name (in-game name):");
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let name = input.trim();
        if !name.is_empty() {
            profile.player = Some(name.to_string());
            needs_save = true;
        }
    }

    if needs_save {
        profile.save()?;
        println!("Configuration saved to: {}", profile_path.display());
    }

    // SteamCMD check
    if profile.steamcmd_enabled && steamcmd::find_steamcmd().is_none() {
        println!("Warning: steamcmd not found. Mod downloads will not work.");
        println!("Install with: sudo apt install steamcmd (or your package manager)");
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    // System check
    match system::check_max_map_count() {
        Ok(check) => {
            if !check.ok {
                println!("Warning: {}", check.recommendation());
                println!("Press Enter to continue...");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
            }
        }
        Err(_) => {} // Not on Linux or can't read sysctl
    }

    Ok(())
}

// ─── Main ─────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let profile_path = config::default_profile_path();

    // Setup wizard
    run_setup_if_needed(&profile_path)?;

    // Initialize
    println!("Fetching server list...");
    let mut ctl = DayzCtl::new(&profile_path).await?;
    let servers_list = ctl.fetch_servers().await?;
    let server_count = servers_list.result.len();
    let servers = servers_list.result.clone();
    println!("Loaded {} servers. Starting TUI...", server_count);

    // Terminal setup
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(ctl, servers);

    // Load installed mods in background
    app.refresh_installed_mods();

    // System check status
    match system::check_max_map_count() {
        Ok(check) if !check.ok => {
            app.set_warn(format!("vm.max_map_count too low ({})", check.current));
        }
        _ => {}
    }

    // Main loop -- non-blocking input so we can poll progress
    let mut running = true;
    let async_stdin = termion::async_stdin();
    let mut key_iter = async_stdin.keys();

    while running {
        // Update terminal size
        if let Ok((_, h)) = termion::terminal_size() {
            app.term_height = h;
        }

        // Poll background operation progress
        app.poll_progress();

        terminal.draw(|f| draw_ui(f, &app))?;

        // Process available input (non-blocking)
        while let Some(key_result) = key_iter.next() {
            let key = match key_result {
                Ok(k) => k,
                Err(_) => break,
            };

            // Popup handling takes priority
            if app.popup.is_some() {
                match key {
                    Key::Char('y') | Key::Char('Y') => {
                        if let Some(Popup::Confirm { action, .. }) = app.popup.take() {
                            app.execute_confirm_action(action);
                        }
                    }
                    Key::Char('n') | Key::Char('N') | Key::Esc => {
                        // For UpdateThenLaunch, "No" means skip update but still launch
                        if let Some(Popup::Confirm {
                            action: ConfirmAction::UpdateThenLaunch(server, pw),
                            ..
                        }) = app.popup.take()
                        {
                            app.set_info("Skipping update, launching...");
                            if let Err(e) = app.ctl.setup_mod_symlinks(&server) {
                                let _ = e;
                            }
                            app.launch_server(&server, pw.as_deref());
                        } else {
                            app.popup = None;
                            app.set_info("Cancelled");
                        }
                    }
                    _ => {}
                }
                break;
            }

            // Option value editing mode
            if app.option_edit_active {
                match key {
                    Key::Esc => {
                        app.cancel_option_edit();
                    }
                    Key::Char('\n') => {
                        app.apply_option_edit();
                    }
                    Key::Backspace => {
                        app.option_edit_value.pop();
                    }
                    Key::Char(c) => {
                        app.option_edit_value.push(c);
                    }
                    _ => {}
                }
                break;
            }

            // Search mode
            if app.search_active {
                match key {
                    Key::Esc => {
                        app.search_active = false;
                        app.search_query.clear();
                        app.filtered_indices = None;
                        app.set_info("Search cleared");
                    }
                    Key::Char('\n') => {
                        app.search_active = false;
                        let count = app
                            .filtered_indices
                            .as_ref()
                            .map(|i| i.len())
                            .unwrap_or(app.servers.len());
                        app.set_info(format!("Found {} servers", count));
                    }
                    Key::Backspace => {
                        app.search_query.pop();
                        app.update_search_filter();
                    }
                    Key::Char(c) => {
                        app.search_query.push(c);
                        app.update_search_filter();
                    }
                    _ => {}
                }
                break;
            }

            // DirectConnect text input mode
            if app.tab == Tab::DirectConnect {
                match key {
                    Key::Esc => {
                        app.tab = Tab::Servers;
                        app.status_message = None;
                    }
                    Key::Up => app.move_selection(-1),
                    Key::Down => app.move_selection(1),
                    // Tab/Shift-Tab switch tabs, same as everywhere else
                    Key::Char('\t') | Key::Ctrl('n') => {
                        let next = app.tab.next();
                        app.tab = next;
                        app.status_message = None;
                    }
                    Key::BackTab | Key::Ctrl('p') => {
                        let prev = app.tab.prev();
                        app.tab = prev;
                        app.status_message = None;
                    }
                    Key::Char('\n') => {
                        if app.direct_cursor == DirectConnectField::Connect {
                            app.handle_direct_connect();
                        } else {
                            app.move_selection(1);
                        }
                    }
                    Key::Backspace => {
                        app.handle_direct_backspace();
                    }
                    Key::Char(c) => {
                        app.handle_direct_input(c);
                    }
                    _ => {}
                }
                break;
            }

            // Normal mode
            match key {
                Key::Char('q') | Key::Ctrl('c') => running = false,
                Key::Esc => {
                    if app.show_server_details {
                        app.show_server_details = false;
                        app.current_a2s_details = None;
                        app.detailed_server_index = None;
                        app.details_scroll_offset = 0;
                    } else {
                        app.status_message = None;
                    }
                }
                // Details panel scrolling
                Key::Ctrl('d') if app.show_server_details => {
                    app.details_scroll_offset = app.details_scroll_offset.saturating_add(5);
                }
                Key::Ctrl('u') if app.show_server_details => {
                    app.details_scroll_offset = app.details_scroll_offset.saturating_sub(5);
                }
                Key::Char('j') | Key::Down => app.move_selection(1),
                Key::Char('k') | Key::Up => app.move_selection(-1),
                Key::Char('G') => {
                    let len = app.current_list_len();
                    if len > 0 {
                        *app.selected_index_mut() = len - 1;
                        let visible = app.visible_items();
                        if len > visible {
                            *app.offset_mut() = len - visible;
                        }
                    }
                }
                Key::Char('g') => {
                    *app.selected_index_mut() = 0;
                    *app.offset_mut() = 0;
                }
                Key::PageUp => app.page_up(),
                Key::PageDown => app.page_down(),

                // Tab switching with Tab / Shift-Tab
                Key::Char('\t') | Key::Ctrl('n') => {
                    let next = app.tab.next();
                    app.tab = next;
                    if app.tab == Tab::Mods && app.installed_mods.is_none() {
                        app.refresh_installed_mods();
                    }
                    app.status_message = None;
                }
                Key::BackTab | Key::Ctrl('p') => {
                    let prev = app.tab.prev();
                    app.tab = prev;
                    if app.tab == Tab::Mods && app.installed_mods.is_none() {
                        app.refresh_installed_mods();
                    }
                    app.status_message = None;
                }

                // Actions
                Key::Char('\n') => {
                    if app.tab == Tab::Options {
                        app.toggle_option_at_index();
                    } else {
                        app.connect_to_selected();
                    }
                }
                Key::Char('e') if app.tab == Tab::Options => {
                    app.start_option_edit();
                }
                Key::Char('f') if app.tab == Tab::Servers || app.tab == Tab::History => {
                    app.add_favorite();
                }
                Key::Char('x') if app.tab == Tab::Favorites => {
                    app.remove_favorite_at_index();
                }
                Key::Char('x') if app.tab == Tab::History => {
                    app.remove_history_entry_at_index();
                }
                Key::Char('c') if app.tab == Tab::History => {
                    app.clear_history();
                }
                Key::Char('i')
                    if app.tab == Tab::Servers
                        || app.tab == Tab::Favorites
                        || app.tab == Tab::History =>
                {
                    app.fetch_a2s_info();
                }
                Key::Char('m')
                    if app.tab == Tab::Servers
                        || app.tab == Tab::Favorites
                        || app.tab == Tab::History =>
                {
                    app.show_server_details = !app.show_server_details;
                    app.details_scroll_offset = 0;
                    if app.show_server_details {
                        app.detailed_server_index = Some(app.selected_index());
                    }
                }
                Key::Char('/') if app.tab == Tab::Servers => {
                    app.search_active = true;
                    app.search_query.clear();
                    app.set_info("Type to search, Enter to confirm, Esc to cancel");
                }
                Key::Char('r') if app.tab == Tab::Servers => {
                    app.refresh_server_list();
                }
                // Mod actions
                Key::Char('r') if app.tab == Tab::Mods => app.refresh_installed_mods(),
                Key::Char('u') if app.tab == Tab::Mods => app.update_selected_mod(),
                Key::Char('U') if app.tab == Tab::Mods => app.update_all_installed_mods(),
                Key::Char('d') if app.tab == Tab::Mods => app.delete_selected_mod(),
                Key::Char('m') if app.tab == Tab::Mods => app.toggle_selected_mod_managed(),
                Key::Char('c') if app.tab == Tab::Mods => app.cleanup_mods(),

                _ => {}
            }
            break; // Process one key per frame
        }

        // Small sleep to avoid busy-spinning (gives ~60fps refresh)
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    // Restore terminal
    terminal.clear()?;
    terminal.show_cursor()?;

    Ok(())
}

// ─── Drawing ──────────────────────────────────────────────────────────────

fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    draw_title_bar(f, app, chunks[0]);
    draw_tab_bar(f, app, chunks[1]);

    match app.tab {
        Tab::Servers => render_servers_tab(f, app, chunks[2]),
        Tab::Favorites => render_favorites_tab(f, app, chunks[2]),
        Tab::History => render_history_tab(f, app, chunks[2]),
        Tab::Mods => render_mods_tab(f, app, chunks[2]),
        Tab::DirectConnect => render_direct_connect_tab(f, app, chunks[2]),
        Tab::Options => render_options_tab(f, app, chunks[2]),
    }

    draw_status_bar(f, app, chunks[3]);

    // Draw progress overlay if active
    if let Some(ref progress) = app.progress_state {
        draw_progress_overlay(f, progress, size);
    }

    // Draw popup overlay if present (on top of everything)
    if let Some(ref popup) = app.popup {
        draw_popup(f, popup, size);
    }
}

fn draw_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let server_count = if let Some(ref indices) = app.filtered_indices {
        format!("{}/{}", indices.len(), app.servers.len())
    } else {
        format!("{}", app.servers.len())
    };

    let login_info = app.ctl.steamcmd_login().unwrap_or("no steamcmd");

    let player = app.ctl.profile().player.as_deref().unwrap_or("unnamed");

    let title = Line::from(vec![
        Span::styled(
            " DayZ-SA Multi Launcher ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("| {} servers ", server_count),
            Style::default().fg(Color::Green),
        ),
        Span::styled(format!("| {} ", player), Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("| {} ", login_info),
            Style::default().fg(Color::Gray),
        ),
    ]);

    f.render_widget(Paragraph::new(title), area);
}

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Span> = Tab::all()
        .iter()
        .enumerate()
        .flat_map(|(i, tab)| {
            let is_sel = *tab == app.tab;
            let label = tab.label();

            let extra = match tab {
                Tab::Favorites => format!("({})", app.ctl.profile().favorites.len()),
                Tab::History => format!("({})", app.ctl.profile().history.len()),
                Tab::Mods => {
                    let count = app.installed_mods.as_ref().map(|m| m.len()).unwrap_or(0);
                    format!("({})", count)
                }
                _ => String::new(),
            };

            let style = if is_sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let sep = if i < Tab::all().len() - 1 {
                Span::styled(" | ", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw(" ")
            };

            vec![Span::styled(format!(" {}{} ", label, extra), style), sep]
        })
        .collect();

    let search_info = if app.search_active {
        vec![
            Span::styled("  Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.search_query, Style::default().fg(Color::White)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]
    } else {
        vec![]
    };

    let mut all_spans = tabs;
    all_spans.extend(search_info);

    let line = Line::from(all_spans);
    let widget = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(widget, area);
}

fn keybinds_for_tab(tab: Tab) -> &'static str {
    match tab {
        Tab::Servers => {
            "j/k:Nav | Enter:Connect | f:Fav | i:A2S | m:Details | /:Search | r:Refresh | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::Favorites => {
            "j/k:Nav | Enter:Connect | i:A2S | x:Remove | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::History => {
            "j/k:Nav | Enter:Connect | i:A2S | f:Fav | x:Remove | c:Clear | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::Mods => {
            "j/k:Nav | u:Update | U:All | d:Del | m:Managed | c:Cleanup | r:Refresh | q:Quit"
        }
        Tab::DirectConnect => "Up/Down:Fields | Enter:Connect | Tab/S-Tab:Tabs | Esc:Back",
        Tab::Options => "j/k:Nav | Enter:Toggle | e:Edit Value | Tab/S-Tab:Tabs | q:Quit",
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let keybinds = keybinds_for_tab(app.tab);

    let line = if let Some(ref msg) = app.status_message {
        Line::from(vec![
            Span::styled(msg.as_str(), Style::default().fg(app.status_color)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(Span::styled(keybinds, Style::default().fg(Color::DarkGray)))
    };

    let widget = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(widget, area);
}

// ─── Shared Helpers ───────────────────────────────────────────────────────

/// Color for player count based on server fill.
fn player_color(players: i64, max_players: i64) -> Color {
    if players == 0 {
        Color::DarkGray
    } else if players >= max_players {
        Color::Red
    } else if players > max_players / 2 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Build a two-line ListItem for a server entry.
/// Used by Servers, Favorites, and History tabs for consistent layout.
fn make_server_list_item<'a>(
    index: usize,
    server: &'a dayzsa_ml::Server,
    is_selected: bool,
    is_favorite: bool,
    prefix: Option<Span<'a>>,
    suffix: Option<Span<'a>>,
) -> ListItem<'a> {
    let style = if is_selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let name_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let fav_marker = if is_favorite {
        Span::styled("* ", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("  ")
    };

    let pc = player_color(server.players, server.max_players);

    let env_tag = if server.environment == "w" {
        Span::styled("W", Style::default().fg(Color::Blue))
    } else {
        Span::styled("L", Style::default().fg(Color::Green))
    };
    let lock_tag = if server.password {
        Span::styled(" P", Style::default().fg(Color::Red))
    } else {
        Span::styled("  ", Style::default().fg(Color::DarkGray))
    };
    let fp_tag = if server.first_person_only {
        Span::styled(" 1P", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("   ", Style::default().fg(Color::DarkGray))
    };

    // Line 1: index + prefix (optional status) + fav + name + suffix (optional time)
    let mut line1 = vec![
        Span::styled(
            format!("{:>5}. ", index + 1),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if let Some(pfx) = prefix {
        line1.push(pfx);
    }
    line1.push(fav_marker);
    line1.push(Span::styled(server.name.as_str(), name_style));
    if let Some(sfx) = suffix {
        line1.push(Span::styled("  ", Style::default()));
        line1.push(sfx);
    }

    // Line 2: IP:port | players | map | mods | version | flags | time
    let line2 = vec![
        Span::styled("        ", Style::default()),
        Span::styled(
            format!("{:<22}", format!("{}:{}", server.endpoint.ip, server.game_port)),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!("{:>3}/{:<3}", server.players, server.max_players),
            Style::default().fg(pc),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:<14}", server.map),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            format!("{:>2} mods", server.mods.len()),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:<12}", server.version),
            Style::default().fg(Color::DarkGray),
        ),
        env_tag,
        lock_tag,
        fp_tag,
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(&server.time, Style::default().fg(Color::DarkGray)),
    ];

    ListItem::new(vec![Line::from(line1), Line::from(line2)]).style(style)
}

// ─── Tab Renderers ────────────────────────────────────────────────────────

fn render_servers_tab(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.offset();
    let selected = app.selected_index();
    let visible = app.visible_items();

    // Split for details panel
    let (list_area, details_area) = if app.show_server_details {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let server_iter: Box<dyn Iterator<Item = (usize, &dayzsa_ml::Server)>> =
        if let Some(ref indices) = app.filtered_indices {
            Box::new(
                indices
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(visible)
                    .map(|(display_i, &real_i)| (display_i, &app.servers[real_i])),
            )
        } else {
            Box::new(app.servers.iter().enumerate().skip(offset).take(visible))
        };

    let items: Vec<ListItem> = server_iter
        .map(|(display_idx, server)| {
            let is_sel = display_idx == selected;
            let is_fav = app
                .ctl
                .profile()
                .is_favorite(&server.endpoint.ip, server.endpoint.port as u16);
            make_server_list_item(display_idx, server, is_sel, is_fav, None, None)
        })
        .collect();

    let total = if let Some(ref indices) = app.filtered_indices {
        indices.len()
    } else {
        app.servers.len()
    };

    let title = if app.filtered_indices.is_some() {
        format!("Servers [{}/{}] (filtered)", selected + 1, total)
    } else {
        format!("Servers [{}/{}]", selected + 1, total)
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_symbol(">> ");
    f.render_widget(list, list_area);

    // Details panel
    if let Some(details_area) = details_area {
        let server = if let Some(ref indices) = app.filtered_indices {
            indices.get(selected).and_then(|&i| app.servers.get(i))
        } else {
            app.servers.get(selected)
        };
        if let Some(server) = server {
            render_server_details(f, app, server, details_area);
        }
    }
}

fn render_server_details(f: &mut Frame, app: &App, server: &dayzsa_ml::Server, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.name, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("IP: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(
                    "{}:{} (query:{})",
                    server.endpoint.ip, server.game_port, server.endpoint.port
                ),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Players: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}/{}", server.players, server.max_players),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  Map: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.map, Style::default().fg(Color::Cyan)),
            Span::styled("  Version: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.version, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Time: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.time, Style::default().fg(Color::White)),
            Span::styled("  1PP: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.first_person_only {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
            Span::styled("  Password: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.password { "Yes" } else { "No" },
                Style::default().fg(Color::White),
            ),
            Span::styled("  Platform: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.environment == "w" {
                    "Windows"
                } else {
                    "Linux"
                },
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    // Mods list
    if !server.mods.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Mods ({}):", server.mods.len()),
            Style::default().fg(Color::Magenta),
        )));

        // Check installed status
        let installed = app.ctl.get_installed_mods().unwrap_or_default();
        let installed_ids: Vec<u64> = installed.iter().map(|m| m.id).collect();

        for mod_ in &server.mods {
            let is_installed = installed_ids.contains(&(mod_.steam_workshop_id as u64));
            let marker = if is_installed { "+" } else { "-" };
            let color = if is_installed {
                Color::Green
            } else {
                Color::Red
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", marker), Style::default().fg(color)),
                Span::styled(&mod_.name, Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ({})", mod_.steam_workshop_id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // A2S details
    if let Some(ref details) = app.current_a2s_details {
        if app.detailed_server_index == Some(app.selected_index()) {
            lines.push(Line::from(Span::styled(
                "--- A2S Live Info ---",
                Style::default().fg(Color::Green),
            )));
            lines.push(Line::from(format!(
                "  Players: {}/{}  Game: {}",
                details.info.players, details.info.max_players, details.info.game
            )));
            if let Some(ref players) = details.players {
                for p in players.iter().take(20) {
                    if !p.name.is_empty() {
                        lines.push(Line::from(format!("    {} (score: {})", p.name, p.score)));
                    }
                }
            }
        }
    }

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize; // account for borders
    let scroll_indicator = if total_lines > visible_height {
        format!(
            " [{}-{}/{}] [Ctrl+d/Ctrl+u:Scroll]",
            app.details_scroll_offset + 1,
            (app.details_scroll_offset as usize + visible_height).min(total_lines),
            total_lines
        )
    } else {
        String::new()
    };

    let text = Text::from(lines);
    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .title(format!("Server Details [Esc to close]{}", scroll_indicator))
                .borders(Borders::ALL),
        )
        .scroll((app.details_scroll_offset, 0));
    f.render_widget(widget, area);
}

fn render_favorites_tab(f: &mut Frame, app: &App, area: Rect) {
    let favorites = &app.ctl.profile().favorites;
    let selected = app.selected_index();

    if favorites.is_empty() {
        let text = Paragraph::new("No favorites yet.\nPress 'f' on a server to add it.")
            .block(Block::default().title("Favorites").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    // Split for details panel
    let (list_area, details_area) = if app.show_server_details {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let items: Vec<ListItem> = favorites
        .iter()
        .enumerate()
        .map(|(i, fav)| {
            let is_sel = i == selected;
            let server = app.servers.iter().find(|s| {
                s.endpoint.ip == fav.ip
                    && (s.endpoint.port as u16 == fav.port || s.game_port as u16 == fav.port)
            });

            match server {
                Some(s) => {
                    let prefix =
                        Span::styled(" ON  ", Style::default().fg(Color::Green));
                    make_server_list_item(i, s, is_sel, true, Some(prefix), None)
                }
                None => {
                    // Offline: simple two-line display
                    let style = if is_sel {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let content = vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{:>5}. ", i + 1),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(" OFF ", Style::default().fg(Color::Red)),
                            Span::styled("* ", Style::default().fg(Color::Yellow)),
                            Span::styled(&fav.name, Style::default().fg(Color::DarkGray)),
                        ]),
                        Line::from(vec![
                            Span::styled("        ", Style::default()),
                            Span::styled(
                                format!("{}:{}", fav.ip, fav.port),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]),
                    ];
                    ListItem::new(content).style(style)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("Favorites [{}]", favorites.len()))
                .borders(Borders::ALL),
        )
        .highlight_symbol(">> ");
    f.render_widget(list, list_area);

    // Details panel
    if let Some(details_area) = details_area {
        if let Some(server) = app.get_selected_server() {
            render_server_details(f, app, &server, details_area);
        }
    }
}

fn render_history_tab(f: &mut Frame, app: &App, area: Rect) {
    let history = &app.ctl.profile().history;
    let selected = app.selected_index();

    if history.is_empty() {
        let text = Paragraph::new("No history yet.\nConnect to a server to see it here.")
            .block(Block::default().title("History").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    // Split for details panel
    let (list_area, details_area) = if app.show_server_details {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let items: Vec<ListItem> = history
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_sel = i == selected;

            // Look up live server info by IP match
            let server = app.servers.iter().find(|s| {
                s.endpoint.ip == entry.ip
                    && (s.endpoint.port as u16 == entry.port || s.game_port as u16 == entry.port)
            });

            let is_fav = app.ctl.profile().is_favorite(&entry.ip, entry.port);
            let time_suffix = Span::styled(
                entry.relative_time(),
                Style::default().fg(Color::DarkGray),
            );

            match server {
                Some(s) => {
                    let prefix =
                        Span::styled(" ON  ", Style::default().fg(Color::Green));
                    make_server_list_item(
                        i,
                        s,
                        is_sel,
                        is_fav,
                        Some(prefix),
                        Some(time_suffix),
                    )
                }
                None => {
                    // Offline: simple display with relative time
                    let style = if is_sel {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let fav_marker = if is_fav {
                        Span::styled("* ", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("  ")
                    };
                    let content = vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{:>5}. ", i + 1),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(" OFF ", Style::default().fg(Color::Red)),
                            fav_marker,
                            Span::styled(
                                entry.name.as_str(),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("        ", Style::default()),
                            Span::styled(
                                format!("{}:{}", entry.ip, entry.port),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                entry.relative_time(),
                                Style::default().fg(Color::Magenta),
                            ),
                        ]),
                    ];
                    ListItem::new(content).style(style)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("Recently Played [{}]", history.len()))
                .borders(Borders::ALL),
        )
        .highlight_symbol(">> ");
    f.render_widget(list, list_area);

    // Details panel
    if let Some(details_area) = details_area {
        if let Some(server) = app.get_selected_server() {
            render_server_details(f, app, &server, details_area);
        }
    }
}

fn render_mods_tab(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.offset();
    let selected = app.selected_index();
    let visible = app.visible_items();
    let mods = app.installed_mods.as_deref().unwrap_or(&[]);

    if mods.is_empty() {
        let text = Paragraph::new("No mods installed or failed to load.\nPress 'r' to refresh.")
            .block(
                Block::default()
                    .title("Installed Mods")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    let items: Vec<ListItem> = mods
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(i, m)| {
            let is_sel = i == selected;
            let style = if is_sel {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let managed_marker = if m.managed {
                Span::styled(" [M] ", Style::default().fg(Color::Cyan))
            } else {
                Span::styled("     ", Style::default())
            };

            let content = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:>4}. ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    managed_marker,
                    Span::styled(
                        &m.name,
                        if is_sel {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled("          ", Style::default()),
                    Span::styled(format!("ID: {}", m.id), Style::default().fg(Color::Cyan)),
                    Span::styled("  Size: ", Style::default().fg(Color::Gray)),
                    Span::styled(mods::format_size(m.size), Style::default().fg(Color::Green)),
                    Span::styled("  Installed: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if m.local_updated > 0 {
                            utils::format_relative_time(m.local_updated)
                        } else {
                            "unknown".to_string()
                        },
                        Style::default().fg(Color::Magenta),
                    ),
                    Span::styled("  Mod updated: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if m.timestamp > 0 {
                            utils::format_relative_time(m.timestamp)
                        } else {
                            "unknown".to_string()
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ];

            ListItem::new(content).style(style)
        })
        .collect();

    let total_size: u64 = mods.iter().map(|m| m.size).sum();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    "Installed Mods [{}/{}] ({})",
                    selected + 1,
                    mods.len(),
                    mods::format_size(total_size)
                ))
                .borders(Borders::ALL),
        )
        .highlight_symbol(">> ");
    f.render_widget(list, area);
}

fn render_direct_connect_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let field_style = |active: bool| {
        if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Address
    let cursor = if app.direct_cursor == DirectConnectField::Address {
        "|"
    } else {
        ""
    };
    let addr = Paragraph::new(format!("{}{}", app.direct_address, cursor))
        .style(field_style(
            app.direct_cursor == DirectConnectField::Address,
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Server Address (IP or IP:PORT)"),
        );
    f.render_widget(addr, chunks[0]);

    // Port
    let cursor = if app.direct_cursor == DirectConnectField::Port {
        "|"
    } else {
        ""
    };
    let port = Paragraph::new(format!("{}{}", app.direct_port, cursor))
        .style(field_style(app.direct_cursor == DirectConnectField::Port))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Port (default: 2302)"),
        );
    f.render_widget(port, chunks[1]);

    // Password
    let cursor = if app.direct_cursor == DirectConnectField::Password {
        "|"
    } else {
        ""
    };
    let pw_display = if app.direct_password.is_empty() {
        format!("(optional){}", cursor)
    } else {
        format!("{}{}", "*".repeat(app.direct_password.len()), cursor)
    };
    let pw = Paragraph::new(pw_display)
        .style(field_style(
            app.direct_cursor == DirectConnectField::Password,
        ))
        .block(Block::default().borders(Borders::ALL).title("Password"));
    f.render_widget(pw, chunks[2]);

    // Connect button
    let btn_style = if app.direct_cursor == DirectConnectField::Connect {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let btn = Paragraph::new("[ CONNECT ]")
        .style(btn_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(btn, chunks[3]);

    // Server info
    let info = if let Some(ref server) = app.direct_server_found {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Found: ", Style::default().fg(Color::Green)),
                Span::styled(&server.name, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(format!(
                "Players: {}/{}",
                server.players, server.max_players
            )),
        ];
        if !server.mods.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("Mods ({}):", server.mods.len()),
                Style::default().fg(Color::Magenta),
            )));
            for m in server.mods.iter().take(15) {
                lines.push(Line::from(format!(
                    "  - {} ({})",
                    m.name, m.steam_workshop_id
                )));
            }
            if server.mods.len() > 15 {
                lines.push(Line::from(format!(
                    "  ... and {} more",
                    server.mods.len() - 15
                )));
            }
        }
        Text::from(lines)
    } else {
        Text::from("Enter server address above. Use Up/Down to navigate fields.")
    };

    let info_widget = Paragraph::new(info)
        .block(Block::default().borders(Borders::ALL).title("Server Info"))
        .wrap(Wrap { trim: true });
    f.render_widget(info_widget, chunks[4]);
}

fn render_options_tab(f: &mut Frame, app: &App, area: Rect) {
    let options = app.ctl.profile().options.all_options();
    let selected = app.selected_index();

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (name, opt))| {
            let is_sel = i == selected;
            let is_editing = is_sel && app.option_edit_active;
            let style = if is_sel {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let check = if opt.enabled {
                Span::styled("[x] ", Style::default().fg(Color::Green))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let value_str = if is_editing {
                format!(" = {}|", app.option_edit_value)
            } else {
                opt.value
                    .as_ref()
                    .map(|v| format!(" = {}", v))
                    .unwrap_or_default()
            };

            let value_color = if is_editing {
                Color::Green
            } else if is_sel {
                Color::Yellow
            } else {
                Color::White
            };

            let content = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:>3}. ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    check,
                    Span::styled(format!("-{}", name), Style::default().fg(value_color)),
                    Span::styled(value_str, Style::default().fg(value_color)),
                ]),
                Line::from(vec![
                    Span::styled("         ", Style::default()),
                    Span::styled(&opt.description, Style::default().fg(Color::DarkGray)),
                ]),
            ];

            ListItem::new(content).style(style)
        })
        .collect();

    let args_preview = app.ctl.profile().options.to_args().join(" ");
    let title = format!(
        "Launch Options [{}]",
        if args_preview.is_empty() {
            "none"
        } else {
            &args_preview
        }
    );

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_symbol(">> ");
    f.render_widget(list, area);
}

fn draw_progress_overlay(f: &mut Frame, progress: &ProgressState, area: Rect) {
    let popup_width = (area.width as f32 * 0.7).min(70.0).max(40.0) as u16;

    // Check if there's a hint to display
    let hint = match &progress.phase {
        ProgressPhase::Finished { hint, .. } => hint.clone(),
        _ => None,
    };
    let hint_lines = hint
        .as_ref()
        .map(|h| h.lines().count() as u16 + 1) // +1 for spacing
        .unwrap_or(0);

    // Height: title(1) + border(2) + gauge(1) + status(1) + completed list (up to 8) + hint + padding
    let completed_lines = progress.completed.len().min(8) as u16;
    let popup_height =
        (6 + completed_lines + hint_lines).min(area.height.saturating_sub(4));

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let constraints = if hint.is_some() {
        vec![
            Constraint::Length(1),          // Status text
            Constraint::Length(1),          // Progress gauge
            Constraint::Length(hint_lines), // Hint text
            Constraint::Min(0),            // Completed list
        ]
    } else {
        vec![
            Constraint::Length(1), // Status text
            Constraint::Length(1), // Progress gauge
            Constraint::Min(0),    // Completed list
        ]
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .margin(1)
        .split(popup_area);

    // Title / border
    let title = if progress.total > 0 {
        format!(" Mod Operation [{}/{}] ", progress.current, progress.total)
    } else {
        " Mod Operation ".to_string()
    };

    let border_color = match &progress.phase {
        ProgressPhase::Finished { hint, .. } if hint.is_some() => Color::Red,
        _ => Color::Cyan,
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(border_color));
    f.render_widget(block, popup_area);

    // Status text: what's currently happening
    let status_text = match &progress.phase {
        ProgressPhase::Downloading => {
            format!(
                "Downloading: {} ({})",
                progress.current_mod_name, progress.current_mod_id
            )
        }
        ProgressPhase::Finished {
            ok, failed, hint, ..
        } => {
            if hint.is_some() {
                "Login failed or expired".to_string()
            } else if *failed == 0 {
                format!("Done! {} mods completed successfully", ok)
            } else {
                format!("Done: {} OK, {} failed", ok, failed)
            }
        }
    };
    let status_color = match &progress.phase {
        ProgressPhase::Downloading => Color::Yellow,
        ProgressPhase::Finished { hint, .. } if hint.is_some() => Color::Red,
        ProgressPhase::Finished { failed, .. } if *failed > 0 => Color::Red,
        _ => Color::Green,
    };
    f.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(status_color)),
        inner[0],
    );

    // Progress gauge
    let ratio = if progress.total > 0 {
        let done = progress.completed.len() as f64;
        done / progress.total as f64
    } else {
        0.0
    };
    let gauge_label = if progress.total > 0 {
        format!("{}/{}", progress.completed.len(), progress.total)
    } else {
        "...".to_string()
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio.min(1.0))
        .label(gauge_label);
    f.render_widget(gauge, inner[1]);

    // Hint text (shown between gauge and completed list)
    let list_area_idx = if hint.is_some() {
        if let Some(ref hint_text) = hint {
            let hint_lines: Vec<Line> = hint_text
                .lines()
                .map(|l| {
                    Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ))
                })
                .collect();
            f.render_widget(Paragraph::new(hint_lines), inner[2]);
        }
        3
    } else {
        2
    };

    // Completed list (show last N items)
    if !progress.completed.is_empty() && list_area_idx < inner.len() {
        let max_show = inner[list_area_idx].height as usize;
        let skip = progress.completed.len().saturating_sub(max_show);
        let lines: Vec<Line> = progress
            .completed
            .iter()
            .skip(skip)
            .map(|(id, name, success)| {
                let marker = if *success { "+" } else { "x" };
                let color = if *success { Color::Green } else { Color::Red };
                Line::from(vec![
                    Span::styled(format!(" {} ", marker), Style::default().fg(color)),
                    Span::styled(name.as_str(), Style::default().fg(Color::White)),
                    Span::styled(format!(" ({})", id), Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner[list_area_idx]);
    }
}

fn draw_popup(f: &mut Frame, popup: &Popup, area: Rect) {
    let popup_width = (area.width as f32 * 0.6).min(60.0) as u16;
    let popup_height = (area.height as f32 * 0.4).min(20.0) as u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    match popup {
        Popup::Confirm { title, message, .. } => {
            let mut lines: Vec<Line> = message.lines().map(|l| Line::from(l.to_string())).collect();
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "[y] Yes  ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[n] No",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));

            let widget = Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .title(title.as_str())
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(widget, popup_area);
        }
        Popup::Info { title, message } => {
            let widget = Paragraph::new(message.as_str())
                .block(
                    Block::default()
                        .title(title.as_str())
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(widget, popup_area);
        }
    }
}

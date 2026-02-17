use egui::{Context, CentralPanel, SidePanel, TopBottomPanel, ScrollArea, RichText, Key};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use crate::{pose::Pose, prompt::PromptGenerator,
    canvas3d::{draw_3d_canvas, Camera3D},
    json_loader::{OptionsLibrary, StylesLibrary, SettingsLibrary, GenericLibrary}};

fn get_app_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") { std::env::var("APPDATA").ok() }
        else if cfg!(target_os = "macos") { std::env::var("HOME").ok().map(|h| format!("{}/Library/Application Support", h)) }
        else                              { std::env::var("HOME").ok().map(|h| format!("{}/.config", h)) };
    let mut p = PathBuf::from(base.unwrap_or_else(|| ".".into()));
    p.push("PromptPuppet");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn saves_file() -> PathBuf { get_app_dir().join("promptpuppet_saves.json") }
fn theme_file() -> PathBuf { get_app_dir().join("promptpuppet_theme.json") }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OptionsData {
    #[serde(flatten)] pub values: HashMap<String, String>,
}
impl OptionsData {
    pub fn from_library(lib: &OptionsLibrary) -> Self {
        Self { values: lib.categories.iter().map(|c| (c.id.clone(), c.default.clone())).collect() }
    }
    pub fn get(&self, id: &str) -> &str { self.values.get(id).map(String::as_str).unwrap_or("") }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut String> { self.values.get_mut(id) }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(flatten)] pub values: HashMap<String, serde_json::Value>,
}
impl Settings {
    pub fn from_library(lib: &SettingsLibrary) -> Self {
        Self { values: lib.settings.iter().map(|s| (s.id.clone(), s.default.clone())).collect() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetItem {
    pub id: String, pub name: String, pub description: String,
    pub tags: Vec<String>,
    #[serde(skip)] pub pose_data: Option<Pose>,
    pub prompt: Option<String>,
    pub allow_custom: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectionState { pub selected: Vec<String>, pub sequence: Vec<String> }

#[derive(Clone, Debug)]
pub struct PresetMetadata {
    pub has_search: Option<bool>, pub multiple_selection: Option<String>,
    pub use_grid: Option<bool>,   pub allow_custom: Option<bool>,
    pub include_prompt: String,
}

impl PresetMetadata {
    pub fn allow_multi(&self, video: bool) -> bool {
        match self.multiple_selection.as_deref().unwrap_or("never") {
            "always" => true, "video" => video, "image" => !video, _ => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)] pub options:     HashMap<String, OptionsData>,
    #[serde(default)] pub settings:    HashMap<String, Settings>,
    pub pose: Pose,
    #[serde(default)] pub video_mode:  bool,
    #[serde(default)] pub selections:  HashMap<String, SelectionState>,
    #[serde(default)] pub custom_data: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedState { pub name: String, pub timestamp: String, pub state: AppState }

pub struct PromptPuppetApp {
    pub state:            AppState,
    pub libraries:        HashMap<String, OptionsLibrary>,
    pub settings_meta:    HashMap<String, SettingsLibrary>,
    pub preset_items:     HashMap<String, Vec<PresetItem>>,
    pub preset_metadata:  HashMap<String, PresetMetadata>,
    pub default_pose:     Pose,
    pub dragging_joint_3d: Option<String>,
    pub search:           HashMap<String, String>,
    pub popup_open:       HashMap<String, bool>,
    pub generated_prompt: String,
    pub status_message:   String,
    pub status_timer:     f32,
    pub ui_config:        crate::json_loader::UiConfig,
    state_hash:           u64,
    pub dark_mode:        bool,
    pub save_dialog:      Option<String>,
    pub load_dialog:      bool,
    pub saves:            Vec<SavedState>,
    pub camera_3d:        Camera3D,
}

#[derive(Serialize, Deserialize)]
struct ThemePref { dark_mode: bool }

fn load_or_warn<T: for<'de> serde::Deserialize<'de>>(name: &str) -> Option<T> {
    crate::json_loader::load(name).map_err(|e| eprintln!("Warning: {e}")).ok()
}

fn load_saves() -> Vec<SavedState> {
    std::fs::read_to_string(saves_file()).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_saves(saves: &[SavedState]) {
    if let Ok(j) = serde_json::to_string_pretty(saves) { let _ = std::fs::write(saves_file(), j); }
}

fn timestamp() -> String {
    // UTC epoch → "YYYY-MM-DD HH:MM" (no external dep)
    let Ok(dur) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else { return "—".into() };
    let s = dur.as_secs();
    let (min, hour) = ((s/60)%60, (s/3600)%24);
    let mut days = s/86400;
    let mut y = 1970u32;
    loop {
        let dy = if y%4==0 && (y%100!=0||y%400==0) { 366u64 } else { 365 };
        if days < dy { break; } days -= dy; y += 1;
    }
    let leap = y%4==0 && (y%100!=0||y%400==0);
    let ml = [31u64, if leap{29}else{28}, 31,30,31,30,31,31,30,31,30,31];
    let (mut mo, mut day) = (1u32, days+1);
    for &m in &ml { if day>m { day-=m; mo+=1; } else { break; } }
    format!("{y}-{mo:02}-{day:02}  {hour:02}:{min:02}")
}

fn load_preset_library(key: &str, path: &str, items: &mut HashMap<String, Vec<PresetItem>>,
    meta: &mut HashMap<String, PresetMetadata>, cx: f32, cy: f32,
    selections: &mut HashMap<String, SelectionState>)
{
    let Some(lib) = load_or_warn::<GenericLibrary>(path) else { return };
    let mut list: Vec<PresetItem> = lib.extract_items().into_iter().map(|gi| {
        let pose_data = gi.to_pose(cx, cy, 40.0);
        PresetItem {
            id: gi.id.clone(), name: if gi.name.is_empty() { gi.id.clone() } else { gi.name },
            description: gi.description, tags: gi.tags, pose_data,
            prompt: gi.prompt.or_else(|| gi.semantics.map(|s| s.prompt)),
            allow_custom: false,
        }
    }).collect();
    if key.contains("style") {
        if let Some(sl) = load_or_warn::<StylesLibrary>(path) {
            list = sl.styles.iter().map(|s| PresetItem {
                id: s.id.clone(), name: s.name.clone(),
                description: format!("{}\nNegative: {}", s.positive, s.negative),
                tags: vec![], pose_data: None, prompt: Some(s.positive.clone()), allow_custom: false,
            }).collect();
            list.push(PresetItem {
                id: "Custom".into(), name: "Custom".into(), description: String::new(),
                tags: vec![], pose_data: None, prompt: None, allow_custom: true,
            });
        }
    }
    if let Some(def) = lib.default {
        if list.iter().any(|p| p.id == def) {
            selections.insert(key.into(), SelectionState { selected: vec![def], sequence: vec![] });
        }
    }
    meta.insert(key.into(), PresetMetadata {
        has_search: lib.has_search, multiple_selection: lib.multiple_selection,
        use_grid: lib.use_grid, allow_custom: None, include_prompt: lib.include_prompt,
    });
    items.insert(key.into(), list);
}

impl Default for PromptPuppetApp {
    fn default() -> Self {
        let ui_config: crate::json_loader::UiConfig =
            load_or_warn("ui_config.json").unwrap_or(crate::json_loader::UiConfig { panels: vec![] });
        let (mut libraries, mut options, mut settings_meta, mut settings) =
            (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());

        for panel in &ui_config.panels {
            let key = panel.data_source.trim_end_matches(".json");
            if panel.components.is_empty() {
                match panel.panel_type.as_str() {
                    "options_grid" => if let Some(lib) = load_or_warn::<OptionsLibrary>(&panel.data_source) {
                        options.insert(key.into(), OptionsData::from_library(&lib));
                        libraries.insert(key.into(), lib);
                    },
                    "controls" => if let Some(lib) = load_or_warn::<SettingsLibrary>(&panel.data_source) {
                        settings.insert(key.into(), Settings::from_library(&lib));
                        settings_meta.insert(key.into(), lib);
                    },
                    _ => {}
                }
            } else {
                for comp in &panel.components {
                    let ckey = comp.data_source.trim_end_matches(".json");
                    if matches!(comp.component_type.as_str(), "dropdown"|"searchable_dropdown") {
                        if let Ok(lib) = crate::json_loader::load::<OptionsLibrary>(&comp.data_source) {
                            options.insert(ckey.into(), OptionsData::from_library(&lib));
                            libraries.insert(ckey.into(), lib);
                        }
                    }
                }
            }
        }

        let (mut preset_items, mut preset_metadata, mut selections) =
            (HashMap::new(), HashMap::new(), HashMap::new());
        const CX: f32 = 400.0; const CY: f32 = 539.0;
        for panel in &ui_config.panels {
            let key = panel.data_source.trim_end_matches(".json");
            if panel.panel_type == "preset_selector" {
                load_preset_library(key, &panel.data_source, &mut preset_items, &mut preset_metadata, CX, CY, &mut selections);
            }
            for comp in &panel.components {
                let ckey = comp.data_source.trim_end_matches(".json");
                if matches!(comp.component_type.as_str(), "dropdown"|"searchable_dropdown") {
                    load_preset_library(ckey, &comp.data_source, &mut preset_items, &mut preset_metadata, CX, CY, &mut selections);
                }
            }
        }

        let dark_mode = std::fs::read_to_string(theme_file()).ok()
            .and_then(|s| serde_json::from_str::<ThemePref>(&s).ok())
            .map(|t| t.dark_mode).unwrap_or(true);

        let default_pose = selections.iter()
            .find_map(|(k, sel)| {
                let id = sel.selected.first()?;
                preset_items.get(k)?.iter().find(|i| &i.id == id)?.pose_data.clone()
            })
            .expect("FATAL: No default pose in JSON. Check poses.json has a default with stick_figure data.");

        let state = AppState { options, settings, pose: default_pose.clone(),
            video_mode: false, selections, custom_data: HashMap::new() };
        Self {
            state, libraries, settings_meta, preset_items, preset_metadata, default_pose,
            dragging_joint_3d: None,
            search: HashMap::new(), popup_open: HashMap::new(),
            generated_prompt: String::new(), status_message: String::new(),
            status_timer: 0.0, ui_config, state_hash: 0, dark_mode,
            save_dialog: None, load_dialog: false, saves: load_saves(),
            camera_3d: Camera3D::default(),
        }
    }
}

impl PromptPuppetApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        cc.egui_ctx.set_theme(if app.dark_mode { egui::Theme::Dark } else { egui::Theme::Light });
        app.update_prompt();
        app
    }
    pub fn reset_pose_to_default(&mut self) {
        self.state.pose = self.default_pose.clone();
        self.set_status("✅ Reset to default pose", 2.0);
    }
    pub fn set_status(&mut self, msg: &str, dur: f32) {
        self.status_message = msg.to_string(); self.status_timer = dur;
    }
    pub fn update_prompt(&mut self) {
        self.generated_prompt = PromptGenerator::new(&self.state, &self.libraries,
            &self.settings_meta, &self.preset_items, &self.preset_metadata, &self.ui_config).generate();
    }
    fn do_save(&mut self, name: String) {
        self.saves.push(SavedState { name: name.clone(), timestamp: timestamp(), state: self.state.clone() });
        write_saves(&self.saves);
        self.set_status(&format!("✅ Saved \"{name}\""), 3.0);
    }
    fn do_load(&mut self, idx: usize) {
        if let Some(saved) = self.saves.get(idx) {
            let name = saved.name.clone();
            self.state = saved.state.clone();
            self.update_prompt();
            self.set_status(&format!("✅ Loaded \"{name}\""), 3.0);
        }
    }
    fn do_delete(&mut self, idx: usize) {
        if idx < self.saves.len() {
            let name = self.saves.remove(idx).name;
            write_saves(&self.saves);
            self.set_status(&format!("🗑 Deleted \"{name}\""), 2.0);
        }
    }
    fn clear_invalid_multiselections(&mut self) {
        let video = self.state.video_mode;
        let to_reset: Vec<_> = self.state.selections.iter()
            .filter(|(_, sel)| sel.selected.len() > 1)
            .filter(|(key, _)| self.preset_metadata.get(*key).map_or(false, |m| !m.allow_multi(video)))
            .map(|(k, _)| k.clone()).collect();
        for key in to_reset {
            if let Some(sel) = self.state.selections.get_mut(&key) {
                if let Some(first) = sel.selected.first().cloned() { sel.selected = vec![first]; }
            }
        }
    }
}

// ── Dialogs ───────────────────────────────────────────────────────────────────

fn dialog_frame(dark: bool) -> egui::Frame {
    egui::Frame::window(&egui::Style::default())
        .fill(if dark { egui::Color32::from_rgb(22,22,35) } else { egui::Color32::from_rgb(242,240,250) })
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(120,80,220)))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(20))
}
fn accent_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(egui::Color32::WHITE).size(13.0))
        .fill(egui::Color32::from_rgb(110,60,210)).corner_radius(egui::CornerRadius::same(6)))
}
fn ghost_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).size(13.0))
        .fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(6)))
}

enum DialogAction { Save(String), Load(usize), Delete(usize), Cancel }

fn show_save_dialog(ctx: &Context, dark: bool, buf: &mut String) -> Option<DialogAction> {
    let mut action = None;
    let muted = if dark { egui::Color32::from_gray(160) } else { egui::Color32::from_gray(90) };
    egui::Window::new("💾  Save State").collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).frame(dialog_frame(dark))
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.label(RichText::new("Name this state:").color(muted).size(13.0));
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(buf).desired_width(f32::INFINITY)
                .hint_text("e.g. Hero standing pose")).request_focus();
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                if (accent_btn(ui, "  Save  ").clicked() || enter) && !buf.trim().is_empty() {
                    action = Some(DialogAction::Save(buf.trim().to_string()));
                }
                ui.add_space(8.0);
                if ghost_btn(ui, "Cancel").clicked() { action = Some(DialogAction::Cancel); }
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) { action = Some(DialogAction::Cancel); }
        });
    action
}

fn show_load_dialog(ctx: &Context, dark: bool, saves: &[SavedState]) -> Option<DialogAction> {
    let mut action = None;
    let (pri, sec) = if dark { (egui::Color32::WHITE, egui::Color32::from_gray(140)) }
                     else    { (egui::Color32::from_gray(20), egui::Color32::from_gray(100)) };
    egui::Window::new("📂  Load State").collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).frame(dialog_frame(dark))
        .show(ctx, |ui| {
            ui.set_min_width(400.0);
            if saves.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("No saved states yet.").color(sec).size(13.0));
                ui.add_space(6.0);
            } else {
                ui.label(RichText::new("Select a state to load:").color(sec).size(12.0));
                ui.add_space(8.0);
                ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    for (i, save) in saves.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.add_space(3.0);
                                if ui.add(egui::Button::selectable(false,
                                    RichText::new(&save.name).strong().size(14.0).color(pri))).clicked() {
                                    action = Some(DialogAction::Load(i));
                                }
                                ui.label(RichText::new(&save.timestamp).size(11.0).color(sec));
                                ui.add_space(3.0);
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("🗑").on_hover_text("Delete").clicked() {
                                    action = Some(DialogAction::Delete(i));
                                }
                            });
                        });
                        ui.separator();
                    }
                });
            }
            ui.add_space(8.0);
            if ghost_btn(ui, "Close").clicked() { action = Some(DialogAction::Cancel); }
            if ui.input(|i| i.key_pressed(Key::Escape)) { action = Some(DialogAction::Cancel); }
        });
    action
}

// ── Window chrome ─────────────────────────────────────────────────────────────

fn render_custom_title_bar(ctx: &Context, dark_mode: bool) {
    use egui::{TopBottomPanel, Layout, Align};
    TopBottomPanel::top("title_bar").frame(egui::Frame {
        inner_margin: egui::Margin::symmetric(8, 4),
        fill: if dark_mode { egui::Color32::from_gray(25) } else { egui::Color32::from_gray(220) },
        ..Default::default()
    }).show(ctx, |ui| {
        ui.horizontal(|ui| {
            let resp = ui.interact(ui.available_rect_before_wrap(), ui.id().with("drag"), egui::Sense::click_and_drag());
            if resp.dragged() { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }
            ui.label(RichText::new("PromptPuppet").strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let sz = egui::vec2(32.0, 20.0);
                if ui.add_sized(sz, egui::Button::new("❌")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                if ui.add_sized(sz, egui::Button::new("🔲")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!ctx.input(|i| i.viewport().maximized.unwrap_or(false))));
                }
                if ui.add_sized(sz, egui::Button::new("➖")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); }
            });
        });
    });
}

fn handle_window_resize(ctx: &Context) {
    use egui::viewport::ResizeDirection as RD;
    let (m, r) = (8.0, ctx.input(|i| i.viewport_rect()));
    let dir = ctx.input(|i| i.pointer.hover_pos().and_then(|p| {
        let (l,ri,t,b) = (p.x<r.min.x+m, p.x>r.max.x-m, p.y<r.min.y+m, p.y>r.max.y-m);
        match (l,ri,t,b) {
            (true,_,true,_)  => Some(RD::NorthWest), (_,true,true,_)  => Some(RD::NorthEast),
            (true,_,_,true)  => Some(RD::SouthWest), (_,true,_,true)  => Some(RD::SouthEast),
            (true,_,_,_)     => Some(RD::West),       (_,true,_,_)     => Some(RD::East),
            (_,_,true,_)     => Some(RD::North),      (_,_,_,true)     => Some(RD::South),
            _                => None,
        }
    }));
    if let Some(d) = dir {
        ctx.set_cursor_icon(match d {
            RD::North|RD::South           => egui::CursorIcon::ResizeVertical,
            RD::East|RD::West             => egui::CursorIcon::ResizeHorizontal,
            RD::NorthEast|RD::SouthWest   => egui::CursorIcon::ResizeNeSw,
            RD::NorthWest|RD::SouthEast   => egui::CursorIcon::ResizeNwSe,
        });
        if ctx.input(|i| i.pointer.primary_pressed()) { ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d)); }
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

impl eframe::App for PromptPuppetApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.save_dialog.is_some() {
            let mut buf = self.save_dialog.take().unwrap();
            match show_save_dialog(ctx, self.dark_mode, &mut buf) {
                Some(DialogAction::Save(name)) => self.do_save(name),
                Some(_) => {}
                None    => self.save_dialog = Some(buf),
            }
        }
        if self.load_dialog {
            let snap = self.saves.clone();
            if let Some(action) = show_load_dialog(ctx, self.dark_mode, &snap) {
                match action {
                    DialogAction::Load(i)   => { self.do_load(i);   self.load_dialog = false; }
                    DialogAction::Delete(i) => self.do_delete(i),
                    DialogAction::Cancel    => self.load_dialog = false,
                    DialogAction::Save(_)   => {}
                }
            }
        }

        render_custom_title_bar(ctx, self.dark_mode);

        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.group(|ui| { ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui.button("💾 Save State").clicked() { self.save_dialog = Some(String::new()); }
                    if ui.button("📂 Load State").clicked() { self.load_dialog = true; }
                    if ui.button("🔄 Reset Pose").clicked() { self.reset_pose_to_default(); }
                }); });
                ui.add_space(12.0);
                if ui.checkbox(&mut self.state.video_mode, "🎬 Video Mode").changed() {
                    self.clear_invalid_multiselections();
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.button(if self.dark_mode { "☀ Light" } else { "🌙 Dark" }).clicked() {
                        self.dark_mode = !self.dark_mode;
                        ctx.set_theme(if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light });
                        let _ = std::fs::write(theme_file(),
                            serde_json::json!({"dark_mode": self.dark_mode}).to_string());
                    }
                });
            });
            ui.add_space(4.0);
        });

        SidePanel::left("controls").min_width(350.0).max_width(500.0).show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                if crate::ui_panels::render_ui_from_config(self, ui, &self.ui_config.clone()) {
                    self.update_prompt();
                }
            });
        });

        TopBottomPanel::bottom("prompt_output").min_height(200.0).max_height(200.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.heading("📝 Generated Prompt");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.add_sized([140.0,28.0],
                        egui::Button::new(RichText::new("📋 Copy to Clipboard").size(14.0))).clicked() {
                        ctx.copy_text(self.generated_prompt.clone());
                        self.set_status("✅ Copied to clipboard", 2.0);
                    }
                });
            });
            ui.add_space(4.0); ui.separator(); ui.add_space(2.0);
            ScrollArea::vertical().show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.generated_prompt.as_str())
                    .desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).interactive(false));
            });
            ui.add_space(4.0);
        });

        CentralPanel::default().show(ctx, |ui| {
            let sz = ui.available_size();
            draw_3d_canvas(ui, &mut self.state.pose, &mut self.camera_3d, sz, &mut self.dragging_joint_3d);
        });

        handle_window_resize(ctx);

        let h = { let mut h = DefaultHasher::new(); format!("{:?}", self.state).hash(&mut h); h.finish() };
        if h != self.state_hash { self.state_hash = h; self.update_prompt(); }

        if self.status_timer > 0.0 {
            self.status_timer -= ctx.input(|i| i.stable_dt);
            if self.status_timer <= 0.0 { self.status_message.clear(); }
            ctx.request_repaint();
        }
    }
}
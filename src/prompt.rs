// prompt.rs
use crate::app::{AppState, PresetItem};
use crate::json_loader::{OptionsLibrary, UiConfig};
use std::collections::HashMap;

pub struct PromptGenerator<'a> {
    state: &'a AppState,
    libraries: &'a HashMap<String, OptionsLibrary>,
    settings_meta: &'a HashMap<String, crate::json_loader::SettingsLibrary>,
    presets: &'a HashMap<String, Vec<PresetItem>>,
    preset_metadata: &'a HashMap<String, crate::app::PresetMetadata>,
    ui_config: &'a UiConfig,
    video_mode: bool,
}

impl<'a> PromptGenerator<'a> {
    pub fn new(
        state: &'a AppState,
        libraries: &'a HashMap<String, OptionsLibrary>,
        settings_meta: &'a HashMap<String, crate::json_loader::SettingsLibrary>,
        presets: &'a HashMap<String, Vec<PresetItem>>,
        preset_metadata: &'a HashMap<String, crate::app::PresetMetadata>,
        ui_config: &'a UiConfig,
    ) -> Self {
        Self { state, libraries, settings_meta, presets, preset_metadata, ui_config, video_mode: state.video_mode }
    }

    fn skip(val: &str) -> bool { val.is_empty() || val == "None" }

    fn should_include(&self, s: &str) -> bool {
        !matches!(s, "video" if !self.video_mode) && !matches!(s, "image" if self.video_mode)
    }

    fn get_selected_prompts(&self, key: &str) -> Vec<String> {
        let Some(selection) = self.state.selections.get(key) else { return Vec::new() };
        let Some(items) = self.presets.get(key) else { return Vec::new() };
        selection.selected.iter()
            .filter_map(|id| items.iter().find(|i| &i.id == id)?.prompt.clone())
            .collect()
    }

    pub fn generate(&self) -> String {
        let mut out = String::new();

        for panel in &self.ui_config.panels {
            let key = panel.data_source.trim_end_matches(".json");

            match panel.panel_type.as_str() {
                "options_grid" => {
                    let Some(lib) = self.libraries.get(key) else { continue };
                    if !self.should_include(&lib.include_prompt) { continue; }
                    let Some(data) = self.state.options.get(key) else { continue };

                    let mut groups: HashMap<Option<String>, Vec<String>> = HashMap::new();
                    for cat in &lib.categories {
                        let val = data.get(&cat.id);
                        if !Self::skip(val) {
                            groups.entry(cat.group.clone()).or_default().push(val.to_string());
                        }
                    }

                    const ORDER: &[&str] = &["Basic Info", "Physical Features", "Facial Features", "Body Details"];
                    let mut all: Vec<String> = groups.remove(&None).unwrap_or_default();
                    for g in ORDER {
                        if let Some(v) = groups.remove(&Some(g.to_string())) { all.extend(v); }
                    }
                    for (_, v) in groups { all.extend(v); }

                    if !all.is_empty() {
                        out.push_str(&all.join(", "));
                        out.push('\n');
                        out.push('\n');
                    }
                }
                "controls" => {
                    let Some(lib) = self.settings_meta.get(key) else { continue };
                    if !self.should_include(&lib.include_prompt) { continue; }
                    let Some(data) = self.state.settings.get(key) else { continue };
                    
                    if key == "global" || key == "motion" {
                        let mut pairs = Vec::new();
                        for setting in &lib.settings {
                            if let Some(v) = data.values.get(&setting.id) {
                                let display = if let Some(f) = v.as_f64() { format!("{:.1}", f) }
                                              else if let Some(s) = v.as_str() { s.to_string() }
                                              else { continue };
                                if !Self::skip(&display) {
                                    pairs.push(format!("{}: {}", setting.label, display));
                                }
                            }
                        }
                        if !pairs.is_empty() {
                            out.push_str(&pairs.join(", "));
                            out.push('\n');
                            out.push('\n');
                        }
                    } else {
                        for (_, v) in &data.values {
                            let display = if let Some(f) = v.as_f64() { format!("{:.1}", f) }
                                          else if let Some(s) = v.as_str() { s.to_string() }
                                          else { continue };
                            if !Self::skip(&display) { out.push_str(&display); out.push('\n'); }
                        }
                    }
                }
                "preset_selector" => {
                    let Some(meta) = self.preset_metadata.get(key) else { continue };
                    if !self.should_include(&meta.include_prompt) { continue; }
                    let prompts = self.get_selected_prompts(key);
                    if !prompts.is_empty() {
                        out.push_str(&prompts.join(", "));
                        out.push('\n');
                        out.push('\n');
                    }
                }
                "composite" => {
                    for comp in &panel.components {
                        let ckey = comp.data_source.trim_end_matches(".json");
                        if let Some(lib) = self.libraries.get(ckey) {
                            if !self.should_include(&lib.include_prompt) { continue; }
                        }
                        let prompts = self.get_selected_prompts(ckey);
                        if !prompts.is_empty() {
                            out.push_str(&prompts.join(", "));
                            out.push('\n');
                            out.push('\n');
                        }
                    }
                }
                _ => {}
            }
        }

        out
    }
}
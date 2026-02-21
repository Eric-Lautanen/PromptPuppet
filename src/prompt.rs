// prompt.rs
use crate::app::{AppState, PresetItem};
use crate::json_loader::{OptionsLibrary, UiConfig};
use std::collections::HashMap;

pub struct PromptGenerator<'a> {
    state:           &'a AppState,
    libraries:       &'a HashMap<String, OptionsLibrary>,
    settings_meta:   &'a HashMap<String, crate::json_loader::SettingsLibrary>,
    presets:         &'a HashMap<String, Vec<PresetItem>>,
    preset_metadata: &'a HashMap<String, crate::app::PresetMetadata>,
    ui_config:       &'a UiConfig,
    video_mode:      bool,
    pose_is_manual:  bool,
}

impl<'a> PromptGenerator<'a> {
    pub fn new(
        state: &'a AppState,
        libraries: &'a HashMap<String, OptionsLibrary>,
        settings_meta: &'a HashMap<String, crate::json_loader::SettingsLibrary>,
        presets: &'a HashMap<String, Vec<PresetItem>>,
        preset_metadata: &'a HashMap<String, crate::app::PresetMetadata>,
        ui_config: &'a UiConfig,
        pose_is_manual: bool,
    ) -> Self {
        Self { state, libraries, settings_meta, presets, preset_metadata, ui_config,
               video_mode: state.video_mode, pose_is_manual }
    }

    fn skip(v: &str) -> bool { v.is_empty() || v == "None" }

    fn include(&self, s: &str) -> bool {
        match s { "video" => self.video_mode, "image" => !self.video_mode, _ => true }
    }

    fn emit(out: &mut String, parts: &[String]) {
        if !parts.is_empty() { out.push_str(&parts.join(", ")); out.push_str("\n\n"); }
    }

    fn val_str(v: &serde_json::Value) -> Option<String> {
        if let Some(f) = v.as_f64() { return Some(format!("{:.1}", f)); }
        v.as_str().map(str::to_string)
    }

    fn selected_prompts(&self, key: &str) -> Vec<String> {
        // For the pose library specifically: if the user has manually moved a
        // joint, replace the preset JSON prompt with a live semantic description.
        if key == "poses" && self.pose_is_manual {
            let desc = crate::semantics::describe(&self.state.pose);
            return if desc.is_empty() { vec![] } else { vec![desc] };
        }

        let Some(sel)   = self.state.selections.get(key) else { return vec![] };
        let Some(items) = self.presets.get(key)          else { return vec![] };
        sel.selected.iter()
            .filter_map(|id| items.iter().find(|i| &i.id == id)?.prompt.clone())
            .collect()
    }

    pub fn generate(&self) -> String {
        let mut out = String::new();
        for panel in &self.ui_config.panels {
            let key = panel.data_source.trim_end_matches(".json");
            match panel.panel_type.as_str() {
                "options_grid" => {
                    let Some(lib)  = self.libraries.get(key)      else { continue };
                    if !self.include(&lib.include_prompt)           { continue }
                    let Some(data) = self.state.options.get(key)   else { continue };
                    let mut groups: HashMap<Option<String>, Vec<String>> = HashMap::new();
                    for cat in &lib.categories {
                        let v = data.get(&cat.id);
                        if !Self::skip(v) { groups.entry(cat.group.clone()).or_default().push(v.to_string()); }
                    }
                    const ORDER: &[&str] = &["Basic Info","Physical Features","Facial Features","Body Details"];
                    let mut all = groups.remove(&None).unwrap_or_default();
                    for g in ORDER { if let Some(v) = groups.remove(&Some(g.to_string())) { all.extend(v); } }
                    // Sort remaining groups by name for stable output order.
                    // HashMap iteration is non-deterministic; without this the prompt
                    // reshuffles every time update_prompt() is called (e.g. on joint drag).
                    let mut remaining: Vec<_> = groups.into_iter().collect();
                    remaining.sort_by_key(|(k, _)| k.clone());
                    for (_, v) in remaining { all.extend(v); }
                    Self::emit(&mut out, &all);
                }
                "controls" => {
                    let Some(lib)  = self.settings_meta.get(key)   else { continue };
                    if !self.include(&lib.include_prompt)           { continue }
                    let Some(data) = self.state.settings.get(key)  else { continue };
                    if matches!(key, "global"|"motion") {
                        let pairs: Vec<_> = lib.settings.iter().filter_map(|s| {
                            let disp = Self::val_str(data.values.get(&s.id)?)?;
                            (!Self::skip(&disp)).then(|| format!("{}: {}", s.label, disp))
                        }).collect();
                        Self::emit(&mut out, &pairs);
                    } else {
                        // Iterate by lib.settings (Vec) order, not data.values (HashMap),
                        // so the output is stable and won't reshuffle on each update_prompt().
                        for s in &lib.settings {
                            if let Some(v) = data.values.get(&s.id) {
                                if let Some(d) = Self::val_str(v) {
                                    if !Self::skip(&d) { out.push_str(&d); out.push('\n'); }
                                }
                            }
                        }
                    }
                }
                "preset_selector" => {
                    let Some(meta) = self.preset_metadata.get(key) else { continue };
                    if !self.include(&meta.include_prompt)          { continue }
                    Self::emit(&mut out, &self.selected_prompts(key));
                }
                "composite" => {
                    for comp in &panel.components {
                        let ckey = comp.data_source.trim_end_matches(".json");
                        if self.libraries.get(ckey).map_or(true, |l| self.include(&l.include_prompt)) {
                            Self::emit(&mut out, &self.selected_prompts(ckey));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}
// json_loader.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct OptionsLibrary {
    pub categories: Vec<OptionCategory>,
    #[serde(default = "default_include_prompt")]
    pub include_prompt: String,
}

fn default_include_prompt() -> String { "always".to_string() }

#[derive(Debug, Deserialize, Clone)]
pub struct OptionCategory {
    pub id: String,
    pub label: String,
    #[serde(default)] pub options: Vec<OptionValue>,
    #[serde(default)] pub allow_custom: bool,
    pub default: String,
    #[serde(default)] pub is_text_field: bool,
    #[serde(default)] pub group: Option<String>,
    #[serde(default)] pub has_search: Option<bool>,
    #[serde(default)] pub visibility: Option<VisibilityRule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VisibilityRule {
    pub condition: String,
    pub field: String,
    #[serde(default)] pub value: Option<String>,
    #[serde(default)] pub values: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OptionValue { pub value: String, pub display: String }

#[derive(Debug, Deserialize, Clone)]
pub struct StylesLibrary {
    pub styles: Vec<StyleEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StyleEntry { pub id: String, pub name: String, pub positive: String, pub negative: String }

#[derive(Debug, Deserialize, Clone)]
pub struct SettingsLibrary {
    pub settings: Vec<SettingEntry>,
    #[serde(default = "default_include_prompt")]
    pub include_prompt: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SettingEntry {
    pub id: String, pub label: String,
    #[serde(rename = "type")] pub setting_type: String,
    #[serde(default)] pub min: Option<f32>,
    #[serde(default)] pub max: Option<f32>,
    #[serde(default)] pub options: Vec<OptionValue>,
    pub default: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenericLibrary {
    #[serde(default)] pub has_search: Option<bool>,
    #[serde(default)] pub multiple_selection: Option<String>,
    #[serde(default)] pub use_grid: Option<bool>,
    #[serde(default = "default_include_prompt")]
    pub include_prompt: String,
    #[serde(default)] pub default: Option<String>,
    #[serde(flatten)] pub data: serde_json::Value,
}

impl GenericLibrary {
    pub fn extract_items(&self) -> Vec<GenericItem> {
        let mut items = Vec::new();
        let Some(obj) = self.data.as_object() else { return items };
        for (_, value) in obj {
            if let Some(arr) = value.as_array() {
                for v in arr {
                    if let Ok(item) = serde_json::from_value(v.clone()) { items.push(item); }
                }
            } else if let Some(nested) = value.as_object() {
                if let Some(cats) = nested.get("categories").and_then(|v| v.as_array()) {
                    for cat in cats {
                        if let Some(cat_obj) = cat.as_object() {
                            // Check for 'poses' or 'expressions' arrays within each category
                            for key in &["poses", "expressions", "items"] {
                                if let Some(arr) = cat_obj.get(*key).and_then(|v| v.as_array()) {
                                    for v in arr {
                                        if let Ok(item) = serde_json::from_value(v.clone()) { items.push(item); }
                                    }
                                }
                            }
                            // Fallback: check all arrays in category object
                            for (_, arr) in cat_obj {
                                if let Some(arr) = arr.as_array() {
                                    for v in arr {
                                        if let Ok(item) = serde_json::from_value(v.clone()) { items.push(item); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        items
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenericItem {
    #[serde(alias = "term")]
    pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub description: String,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub prompt: Option<String>,
    #[serde(default)] pub stick_figure: Option<StickFigure>,
    #[serde(default)] pub semantics: Option<Semantics>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StickFigure { pub points: HashMap<String, [f32; 2]> }

#[derive(Debug, Deserialize, Clone)]
pub struct Semantics { pub prompt: String }

impl GenericItem {
    pub fn to_pose(&self, cx: f32, cy: f32, scale: f32) -> Option<crate::pose::Pose> {
        let sf = self.stick_figure.as_ref()?;
        let pt = |name: &str| -> (f32, f32) {
            sf.points.get(name).map(|p| (cx + p[0] * scale, cy - p[1] * scale)).unwrap_or((cx, cy))
        };
        let j = |name: &str| { let (x, y) = pt(name); crate::pose::Joint::new(x, y) };
        let blend = |a: &str, b: &str, wa: f32, wb: f32| {
            let (ax, ay) = pt(a); let (bx, by) = pt(b);
            crate::pose::Joint::new(ax * wa + bx * wb, ay * wa + by * wb)
        };
        let ankle = |foot: &str| { let (x, y) = pt(foot); crate::pose::Joint::new(x, y - 10.0) };

        Some(crate::pose::Pose {
            head:           j("head"),
            left_shoulder:  j("left_shoulder"),  right_shoulder: j("right_shoulder"),
            left_elbow:     j("left_elbow"),      right_elbow:    j("right_elbow"),
            left_wrist:     blend("left_hand",  "left_elbow",  0.7, 0.3),
            right_wrist:    blend("right_hand", "right_elbow", 0.7, 0.3),
            left_hand:      j("left_hand"),       right_hand:     j("right_hand"),
            left_fingers:   crate::pose::FingerSet::default(),
            right_fingers:  crate::pose::FingerSet::default(),
            hips:           j("pelvis"),
            torso_lean: 0.0, torso_sway: 0.0,
            left_knee:      j("left_knee"),       right_knee:     j("right_knee"),
            left_ankle:     ankle("left_foot"),   right_ankle:    ankle("right_foot"),
            left_foot:      j("left_foot"),        right_foot:     j("right_foot"),
            head_tilt: 0.0, head_nod: 0.0, head_yaw: 0.0,
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct UiConfig { pub panels: Vec<PanelConfig> }

#[derive(Debug, Deserialize, Clone)]
pub struct PanelConfig {
    pub title: String,
    #[serde(rename = "type")] pub panel_type: String,
    #[serde(default)] pub data_source: String,
    pub default_open: bool,
    #[serde(default)] pub components: Vec<ComponentConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ComponentConfig {
    pub label: String,
    #[serde(rename = "type")] pub component_type: String,
    pub data_source: String,
}

// include_str! requires compile-time paths; all assets must be listed here.
fn asset(name: &str) -> Result<&'static str, String> {
    match name {
        "ui_config.json"             => Ok(include_str!("../assets/ui_config.json")),
        "character_attributes.json"  => Ok(include_str!("../assets/character_attributes.json")),
        "clothing.json"              => Ok(include_str!("../assets/clothing.json")),
        "styles.json"                => Ok(include_str!("../assets/styles.json")),
        "motion.json"                => Ok(include_str!("../assets/motion.json")),
        "global.json"                => Ok(include_str!("../assets/global.json")),
        "poses.json"                 => Ok(include_str!("../assets/poses.json")),
        "expressions.json"           => Ok(include_str!("../assets/expressions.json")),
        "environments.json"          => Ok(include_str!("../assets/environments.json")),
        _ => Err(format!("Asset '{name}' not embedded. Add it to json_loader.rs asset() to embed at compile time.")),
    }
}

pub fn load<T: for<'de> Deserialize<'de>>(name: &str) -> Result<T, String> {
    serde_json::from_str(asset(name)?).map_err(|e| format!("Parse error in {name}: {e}"))
}

impl OptionCategory {
    pub fn get_display_text(&self, value: &str) -> String {
        self.options.iter().find(|o| o.value == value)
            .map(|o| o.display.clone()).unwrap_or_else(|| value.to_string())
    }

    pub fn should_show(&self, data: &crate::app::OptionsData) -> bool {
        let Some(vis) = &self.visibility else { return true };
        let fv = data.get(&vis.field);
        match vis.condition.as_str() {
            "field_equals"     => vis.value.as_ref().map_or(true, |v| fv == v),
            "field_in"         => vis.values.contains(&fv.to_string()),
            "field_not_equals" => vis.value.as_ref().map_or(true, |v| fv != v),
            _                  => true,
        }
    }
}
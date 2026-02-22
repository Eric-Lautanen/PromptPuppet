// json_loader.rs
// 
// This module handles loading and parsing of JSON assets including:
// - Pose libraries with 3D coordinates [x, y, z]
// - Expression libraries
// - Style libraries
// - Options and settings
//
// 3D Coordinate Support:
// The StickFigure struct now uses Vec<f32> to support both legacy 2D poses [x, y]
// and new 3D poses [x, y, z]. The to_pose() method automatically handles both formats.

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
pub struct StyleEntry {
    pub id: String, pub name: String,
    pub positive: String,
    #[serde(skip)] pub negative: String,  // never used in prompt generation; skip deserialization
}

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
        let parse  = |v: &serde_json::Value| serde_json::from_value::<GenericItem>(v.clone()).ok();
        let from_arr = |a: &Vec<serde_json::Value>| a.iter().filter_map(parse).collect::<Vec<_>>();
        let Some(obj) = self.data.as_object() else { return vec![] };
        obj.values().flat_map(|value| {
            if let Some(arr) = value.as_array() {
                from_arr(arr)
            } else if let Some(cats) = value.as_object()
                .and_then(|o| o.get("categories")).and_then(|c| c.as_array())
            {
                cats.iter().filter_map(|c| c.as_object())
                    .flat_map(|cat| cat.values().filter_map(|v| v.as_array()).flat_map(from_arr))
                    .collect()
            } else { vec![] }
        }).collect()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenericItem {
    #[serde(alias = "term")]
    pub id: String,
    #[serde(default)] pub name: String,
    #[serde(skip)]    pub description: String,   // never used after load; skip deserialization
    #[serde(skip)]    pub tags: Vec<String>,      // never used after load; skip deserialization
    #[serde(default)] pub prompt: Option<String>,
    #[serde(default)] pub stick_figure: Option<StickFigure>,
    #[serde(default)] pub semantics: Option<Semantics>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StickFigure { 
    pub points: HashMap<String, Vec<f32>>
}

#[derive(Debug, Deserialize, Clone)]
pub struct Semantics { pub prompt: String }

impl GenericItem {
    pub fn to_pose(&self, cx: f32, cy: f32, scale: f32) -> Option<crate::pose::Pose> {
        let sf = self.stick_figure.as_ref()?;
        let sk = crate::skeleton::get();

        // Helper to get point with smart Z defaults based on anatomy
        let pt = |name: &str| -> (f32, f32, f32) {
            sf.points.get(name).map(|p| {
                let z = if p.len() >= 3 { 
                    p[2] * scale 
                } else {
                    // Smart depth defaults based on anatomy when Z is missing
                    match name {
                        "left_elbow" | "right_elbow" => -4.0 * scale,  // Arms slightly forward
                        "left_wrist" | "right_wrist" => -4.0 * scale,
                        "left_knee" | "right_knee" => 6.0 * scale,     // Legs slightly back
                        "left_ankle" | "right_ankle" => 6.0 * scale,
                        "pelvis" => 4.0 * scale,                        // Pelvis slightly back
                        _ => 0.0,                                       // Head, neck, shoulders at center
                    }
                };
                (cx + p[0] * scale, cy - p[1] * scale, z)
            }).unwrap_or((cx, cy, 0.0))
        };
        let j = |name: &str| { let (x, y, z) = pt(name); crate::pose::Joint::new_3d(x, y, z) };

        let wrist = |elbow: &str| {
            let (ex, ey, ez) = pt(elbow);
            crate::pose::Joint::new_3d(ex, ey + sk.seg("forearm"), ez)
        };
        let ankle = |knee: &str| {
            let (kx, ky, kz) = pt(knee);
            crate::pose::Joint::new_3d(kx, ky + sk.seg("shin"), kz)
        };
        let (ls, rs) = (pt("left_shoulder"), pt("right_shoulder"));
        // Use the JSON neck point directly; only fall back to shoulder midpoint if neck is absent.
        let smid = if sf.points.contains_key("neck") {
            j("neck")
        } else {
            crate::pose::Joint::new_3d((ls.0+rs.0)/2.0, (ls.1+rs.1)/2.0, (ls.2+rs.2)/2.0)
        };

        let mut pose = crate::pose::Pose {
            head:           j("head"),
            neck:           smid,
            left_shoulder:  j("left_shoulder"),  right_shoulder: j("right_shoulder"),
            left_elbow:     j("left_elbow"),      right_elbow:    j("right_elbow"),
            left_wrist:     wrist("left_elbow"),  right_wrist:    wrist("right_elbow"),
            left_fingers:   crate::pose::FingerSet::default(),
            right_fingers:  crate::pose::FingerSet::default(),
            waist:          crate::pose::Joint::new_3d(smid.x, smid.y + sk.seg("torso_upper"), smid.z),
            crotch:         j("pelvis"),
            torso_lean: 0.0, torso_sway: 0.0,
            left_knee:      j("left_knee"),       right_knee:     j("right_knee"),
            left_ankle:     ankle("left_knee"),   right_ankle:    ankle("right_knee"),
            head_tilt: 0.0, head_nod: 0.0, head_yaw: 0.0,
        };
        
        // FORCE all segments to match skeleton.json - fixes bad JSON proportions
        let constrain_dist = |from: (f32,f32,f32), to: (f32,f32,f32), len: f32| -> (f32,f32,f32) {
            let (dx, dy, dz) = (to.0-from.0, to.1-from.1, to.2-from.2);
            let d = (dx*dx + dy*dy + dz*dz).sqrt();
            if d < 0.001 { return (from.0+len, from.1, from.2); }
            let s = len / d;
            (from.0+dx*s, from.1+dy*s, from.2+dz*s)
        };
        
        // Fix shoulder width
        let ls_pos = pose.left_shoulder.xyz();
        let rs_pos = pose.right_shoulder.xyz();
        let sh_mid = ((ls_pos.0+rs_pos.0)/2.0, (ls_pos.1+rs_pos.1)/2.0, (ls_pos.2+rs_pos.2)/2.0);
        let ls_dir = (ls_pos.0-sh_mid.0, ls_pos.1-sh_mid.1, ls_pos.2-sh_mid.2);
        let d = (ls_dir.0*ls_dir.0 + ls_dir.1*ls_dir.1 + ls_dir.2*ls_dir.2).sqrt();
        if d > 0.001 {
            let half_width = sk.seg("shoulder_width") / 2.0;
            let s = half_width / d;
            pose.left_shoulder.set_xyz((sh_mid.0+ls_dir.0*s, sh_mid.1+ls_dir.1*s, sh_mid.2+ls_dir.2*s));
            pose.right_shoulder.set_xyz((sh_mid.0-ls_dir.0*s, sh_mid.1-ls_dir.1*s, sh_mid.2-ls_dir.2*s));
        }
        
        // CRITICAL: In the Pose model, `neck` IS the shoulder midpoint (the collar
        // joint). Both move_shoulder() and ragdoll_from_neck() enforce this invariant
        // at runtime, so the loaded pose must match. JSON files often author "neck"
        // as the anatomical mid-neck (above the shoulders), which detaches the
        // shoulder bar from the spine on load.
        //
        // Fix: snap neck to the true midpoint of the (now-constrained) shoulders,
        // then translate head by the same delta so the neck-segment bone stays intact.
        {
            let ls_c = pose.left_shoulder.xyz();
            let rs_c = pose.right_shoulder.xyz();
            let true_neck = (
                (ls_c.0 + rs_c.0) / 2.0,
                (ls_c.1 + rs_c.1) / 2.0,
                (ls_c.2 + rs_c.2) / 2.0,
            );
            let old_neck = pose.neck.xyz();
            let nd = (true_neck.0 - old_neck.0, true_neck.1 - old_neck.1, true_neck.2 - old_neck.2);
            pose.neck.set_xyz(true_neck);
            pose.head.translate(nd.0, nd.1, nd.2);
        }

        // Fix left arm
        let lsh = pose.left_shoulder.xyz();
        let lel = pose.left_elbow.xyz();
        pose.left_elbow.set_xyz(constrain_dist(lsh, lel, sk.seg("arm")));
        let lel = pose.left_elbow.xyz();
        let lwr = pose.left_wrist.xyz();
        pose.left_wrist.set_xyz(constrain_dist(lel, lwr, sk.seg("forearm")));
        
        // Fix right arm
        let rsh = pose.right_shoulder.xyz();
        let rel = pose.right_elbow.xyz();
        pose.right_elbow.set_xyz(constrain_dist(rsh, rel, sk.seg("arm")));
        let rel = pose.right_elbow.xyz();
        let rwr = pose.right_wrist.xyz();
        pose.right_wrist.set_xyz(constrain_dist(rel, rwr, sk.seg("forearm")));
        
        // Fix spine
        let neck = pose.neck.xyz();
        let waist = pose.waist.xyz();
        pose.waist.set_xyz(constrain_dist(neck, waist, sk.seg("torso_upper")));
        let waist = pose.waist.xyz();
        let crotch = pose.crotch.xyz();
        pose.crotch.set_xyz(constrain_dist(waist, crotch, sk.seg("torso_lower")));
        
        // Fix left leg
        let crotch = pose.crotch.xyz();
        let lkn = pose.left_knee.xyz();
        pose.left_knee.set_xyz(constrain_dist(crotch, lkn, sk.seg("thigh")));
        let lkn = pose.left_knee.xyz();
        let lank = pose.left_ankle.xyz();
        pose.left_ankle.set_xyz(constrain_dist(lkn, lank, sk.seg("shin")));
        
        // Fix right leg
        let rkn = pose.right_knee.xyz();
        pose.right_knee.set_xyz(constrain_dist(crotch, rkn, sk.seg("thigh")));
        let rkn = pose.right_knee.xyz();
        let rank = pose.right_ankle.xyz();
        pose.right_ankle.set_xyz(constrain_dist(rkn, rank, sk.seg("shin")));
        
        // ── Derive head orientation from the neck→head direction vector ──────────────
        // Coordinate space: X = right, Y = up, Z = into screen (away from viewer).
        //
        //   head_nod  > 0  chin down  (head tips toward camera / looking forward-down)
        //             < 0  chin up    (head tips away  / looking up)
        //   head_yaw  > 0  turned right (character's own right)
        //             < 0  turned left
        //   head_tilt = 0  roll — cannot be inferred from a 2-point vector alone
        {
            let (nx, ny, nz) = pose.neck.xyz();
            let (hx, hy, hz) = pose.head.xyz();
            let (dx, dy, dz) = (hx - nx, hy - ny, hz - nz);
            let len = (dx*dx + dy*dy + dz*dz).sqrt();
            if len > 0.001 {
                // Nod: angle of the neck→head vector in the YZ plane relative to straight up.
                // Negative Z (toward viewer) → chin drops forward → positive nod.
                pose.head_nod = (-dz / len).asin().to_degrees();

                // Yaw: lateral deviation of the neck→head vector in the XZ plane.
                // Positive X (character's right) → positive yaw.
                // We use the full len so poses with simultaneous nod+yaw decode correctly.
                pose.head_yaw = (dx / len).asin().to_degrees();

                // Tilt (roll around the neck→head axis) cannot be resolved from
                // two points — leave it neutral. A future pass could read a
                // "head_right" hint from the JSON if you add one.
                pose.head_tilt = 0.0;
            }
        }

        Some(pose)
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
        "skeleton.json"              => Ok(include_str!("../assets/skeleton.json")),
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
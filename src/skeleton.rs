// skeleton.rs — loaded once via OnceLock; shared by ui_canvas and canvas3d.
use std::sync::OnceLock;
use serde::Deserialize;
use egui::Color32;

#[derive(Debug, Clone, Deserialize)]
pub struct BoneDef  { pub a: String, pub b: String, pub color: [u8; 3] }

#[derive(Debug, Clone, Deserialize)]
pub struct JointDef { pub name: String, pub radius: f32, pub color: [u8; 3] }

#[derive(Debug, Clone, Deserialize)]
pub struct Segments {
    pub arm: f32, pub forearm: f32, pub thigh: f32, pub shin: f32,
    pub neck: f32, pub torso_upper: f32, pub torso_lower: f32,
    pub shoulder_width: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AngleRange { pub min: f32, pub max: f32 }

#[derive(Debug, Clone, Deserialize)]
pub struct Constraints {
    #[serde(default = "default_elbow")]
    pub elbow: AngleRange,
    #[serde(default = "default_knee")]
    pub knee: AngleRange,
}

// Angle at the joint between upper and lower bone:
//   180° = fully straight (extended)  |  ~30° = maximum anatomical flexion
// OLD values (min:0 max:155) were BACKWARDS: max:155 blocked straightening,
// and min:0 allowed impossible hyperextension past the bone bulk.
fn default_elbow() -> AngleRange { AngleRange { min: 30.0, max: 180.0 } }
fn default_knee()  -> AngleRange { AngleRange { min: 30.0, max: 180.0 } }

#[derive(Debug, Clone, Deserialize)]
pub struct Skeleton {
    pub head_size: f32,
    pub segments:  Segments,
    pub bones:     Vec<BoneDef>,
    pub joints:    Vec<JointDef>,
    pub constraints: Constraints,
}

impl Skeleton {
    pub fn seg(&self, name: &str) -> f32 {
        let s = &self.segments;
        self.head_size * match name {
            "arm"            => s.arm,   "forearm"     => s.forearm,
            "thigh"          => s.thigh, "shin"        => s.shin,
            "neck"           => s.neck,  "torso_upper" => s.torso_upper,
            "torso_lower"    => s.torso_lower,
            "shoulder_width" => s.shoulder_width,
            _                => return 0.0,
        }
    }
}

pub fn color32(rgb: [u8; 3]) -> Color32 { Color32::from_rgb(rgb[0], rgb[1], rgb[2]) }

static SK: OnceLock<Skeleton> = OnceLock::new();

pub fn get() -> &'static Skeleton {
    SK.get_or_init(|| crate::json_loader::load("skeleton.json")
        .expect("skeleton.json missing or malformed"))
}
// pose.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Joint { 
    pub x: f32, 
    pub y: f32, 
    #[serde(default)]
    pub z: f32,
    pub angle: f32 
}

impl Joint {
    pub fn new(x: f32, y: f32) -> Self { Self { x, y, z: 0.0, angle: 0.0 } }
    pub fn distance_to(&self, px: f32, py: f32) -> f32 {
        ((self.x - px).powi(2) + (self.y - py).powi(2)).sqrt()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerSet {
    pub thumb: f32, pub index: f32, pub middle: f32,
    pub ring: f32,  pub pinky: f32, pub spread: f32,
}

impl Default for FingerSet {
    fn default() -> Self { Self { thumb: 0.0, index: 0.0, middle: 0.0, ring: 0.0, pinky: 0.0, spread: 20.0 } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub left_shoulder: Joint, pub right_shoulder: Joint,
    pub left_elbow: Joint,    pub right_elbow: Joint,
    pub left_wrist: Joint,    pub right_wrist: Joint,
    pub left_hand: Joint,     pub right_hand: Joint,
    pub left_fingers: FingerSet, pub right_fingers: FingerSet,
    pub hips: Joint,
    pub torso_lean: f32, pub torso_sway: f32,
    pub left_knee: Joint,  pub right_knee: Joint,
    pub left_ankle: Joint, pub right_ankle: Joint,
    pub left_foot: Joint,  pub right_foot: Joint,
    pub head: Joint,
    pub head_tilt: f32, pub head_nod: f32, pub head_yaw: f32,
}

impl Pose {
    pub fn new_anatomical(cx: f32, cy: f32) -> Self {
        Self {
            head:           Joint::new(cx,           cy - 156.0),
            left_shoulder:  Joint::new(cx -  38.0,   cy - 108.0),
            right_shoulder: Joint::new(cx +  38.0,   cy - 108.0),
            left_elbow:     Joint::new(cx -  38.0,   cy -  33.0),
            right_elbow:    Joint::new(cx +  38.0,   cy -  33.0),
            left_wrist:     Joint::new(cx -  38.0,   cy +  26.0),
            right_wrist:    Joint::new(cx +  38.0,   cy +  26.0),
            left_hand:      Joint::new(cx -  38.0,   cy +  46.0),
            right_hand:     Joint::new(cx +  38.0,   cy +  46.0),
            left_fingers: FingerSet::default(), right_fingers: FingerSet::default(),
            hips:           Joint::new(cx,           cy +  38.0),
            torso_lean: 0.0, torso_sway: 0.0,
            left_knee:      Joint::new(cx -  19.0,   cy + 113.0),
            right_knee:     Joint::new(cx +  19.0,   cy + 113.0),
            left_ankle:     Joint::new(cx -  19.0,   cy + 169.0),
            right_ankle:    Joint::new(cx +  19.0,   cy + 169.0),
            left_foot:      Joint::new(cx -   9.0,   cy + 189.0),
            right_foot:     Joint::new(cx +  29.0,   cy + 189.0),
            head_tilt: 0.0, head_nod: 0.0, head_yaw: 0.0,
        }
    }

    pub fn update_joint_angle(&mut self, joint_name: &str, origin_x: f32, origin_y: f32) {
        let joint = match joint_name {
            "left_shoulder"  => &mut self.left_shoulder,
            "right_shoulder" => &mut self.right_shoulder,
            "left_elbow"     => &mut self.left_elbow,
            "right_elbow"    => &mut self.right_elbow,
            "left_wrist"     => &mut self.left_wrist,
            "right_wrist"    => &mut self.right_wrist,
            "left_hand"      => &mut self.left_hand,
            "right_hand"     => &mut self.right_hand,
            "left_knee"      => &mut self.left_knee,
            "right_knee"     => &mut self.right_knee,
            "left_ankle"     => &mut self.left_ankle,
            "right_ankle"    => &mut self.right_ankle,
            "left_foot"      => &mut self.left_foot,
            "right_foot"     => &mut self.right_foot,
            "head"           => &mut self.head,
            "hips"           => &mut self.hips,
            _ => return,
        };
        joint.angle = (joint.y - origin_y).atan2(joint.x - origin_x).to_degrees();
    }

    pub fn clamp_angles(&mut self) {
        self.left_elbow.angle  = self.left_elbow.angle.clamp(-90.0,  90.0);
        self.right_elbow.angle = self.right_elbow.angle.clamp(-90.0, 90.0);
        self.left_knee.angle   = self.left_knee.angle.clamp(-5.0,  140.0);
        self.right_knee.angle  = self.right_knee.angle.clamp(-5.0, 140.0);
        self.head_tilt  = self.head_tilt.clamp(-15.0,  15.0);
        self.head_nod   = self.head_nod.clamp(-10.0,   10.0);
        self.head_yaw   = self.head_yaw.clamp(-30.0,   30.0);
        self.torso_lean = self.torso_lean.clamp(-30.0, 30.0);
        self.torso_sway = self.torso_sway.clamp(-20.0, 20.0);
    }
}
// pose.rs — 3D: X left→right, Y bottom→top, Z viewer→scene
use serde::{Deserialize, Serialize};

// ========== Vec3 helpers for cleaner FABRIK ==========
#[derive(Copy, Clone)]
struct Vec3 { x: f32, y: f32, z: f32 }

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    fn from_tuple(t: (f32, f32, f32)) -> Self { Self::new(t.0, t.1, t.2) }
    fn to_tuple(self) -> (f32, f32, f32) { (self.x, self.y, self.z) }
    
    fn dot(self, o: Self) -> f32 { self.x*o.x + self.y*o.y + self.z*o.z }
    fn len(self) -> f32 { self.dot(self).sqrt() }
    fn norm(self) -> Self {
        let l = self.len();
        if l < 0.001 { Self::new(1.0, 0.0, 0.0) } else { self.scale(1.0/l) }
    }
    fn scale(self, s: f32) -> Self { Self::new(self.x*s, self.y*s, self.z*s) }
    fn add(self, o: Self) -> Self { Self::new(self.x+o.x, self.y+o.y, self.z+o.z) }
    fn sub(self, o: Self) -> Self { Self::new(self.x-o.x, self.y-o.y, self.z-o.z) }
    fn distance(self, o: Self) -> f32 { self.sub(o).len() }
    
    fn cross(self, o: Self) -> Self {
        Self::new(
            self.y*o.z - self.z*o.y,
            self.z*o.x - self.x*o.z,
            self.x*o.y - self.y*o.x,
        )
    }
    
    fn rotate_around_axis(self, axis: Self, angle: f32) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        self.scale(cos).add(axis.cross(self).scale(sin)).add(axis.scale(axis.dot(self) * (1.0 - cos)))
    }
    
    /// Soft clamp - gradually resists instead of hard snap
    /// softness: 0.0 = hard clamp, 1.0 = no correction
    fn soft_clamp(current: f32, min: f32, max: f32, softness: f32) -> f32 {
        if current >= min && current <= max { return current; }
        
        let clamped = current.clamp(min, max);
        let correction_strength = 1.0 - softness.clamp(0.0, 1.0);
        
        // Lerp between current and clamped based on correction strength
        current + (clamped - current) * correction_strength
    }
}
// ========== End Vec3 helpers ==========

// ========== Constraint System ==========
// Flexible constraint architecture supporting multiple joint types:
//
// - Hinge: Single-axis rotation (elbows, knees)
//   Example: elbow with 0-155° bend
//
// - Cone: Spherical motion within cone (shoulders, hips)
//   Example: shoulder with 90° max deviation from parent bone
//
// - EllipticalCone: Asymmetric cone (neck, wrist)
//   Example: neck with different pitch (-45 to +45) and yaw (-60 to +60) limits
//
// - Twist: Axial rotation (future: forearm pronation/supination)
//
// Current usage: Simple hinge constraints for elbow/knee via skeleton.json
// Future: Per-joint constraint definitions with parent relationships

#[derive(Clone)]
pub enum ConstraintType {
    Hinge,           // Elbow, knee - single axis rotation
    Cone,            // Shoulder, hip - cone of motion
    Twist,           // Forearm rotation
    EllipticalCone,  // Neck - asymmetric cone
}

#[derive(Clone)]
pub struct ConstraintDef {
    pub ctype: ConstraintType,
    // Hinge params
    pub axis: Option<Vec3>,
    pub min_deg: f32,
    pub max_deg: f32,
    // Cone params
    pub cone_angle: Option<f32>,
    // Elliptical params
    pub pitch_min: Option<f32>,
    pub pitch_max: Option<f32>,
    pub yaw_min: Option<f32>,
    pub yaw_max: Option<f32>,
    // Soft constraint params
    pub softness: f32,  // 0.0 = hard snap, 1.0 = very soft/gradual
}

impl ConstraintDef {
    pub fn hinge(min_deg: f32, max_deg: f32) -> Self {
        Self {
            ctype: ConstraintType::Hinge,
            axis: None,
            min_deg,
            max_deg,
            cone_angle: None,
            pitch_min: None,
            pitch_max: None,
            yaw_min: None,
            yaw_max: None,
            softness: 0.7,  // Default to fairly soft constraints
        }
    }
}
// ========== End Constraint System ==========


#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Joint {
    pub x: f32, pub y: f32,
    #[serde(default)] pub z: f32,
    pub angle: f32,
}

impl Joint {
    pub fn new_3d(x: f32, y: f32, z: f32) -> Self { Self { x, y, z, angle: 0.0 } }
    
    pub fn set_xyz(&mut self, (x, y, z): (f32, f32, f32))  { self.x = x; self.y = y; self.z = z; }
    pub fn xyz(&self) -> (f32, f32, f32)   { (self.x, self.y, self.z) }
    
    /// Apply a delta position to this joint
    pub fn translate(&mut self, dx: f32, dy: f32, dz: f32) {
        self.x += dx; self.y += dy; self.z += dz;
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
    pub head: Joint, pub neck: Joint,
    pub left_shoulder: Joint,  pub right_shoulder: Joint,
    pub left_elbow: Joint,     pub right_elbow: Joint,
    pub left_wrist: Joint,     pub right_wrist: Joint,
    pub left_fingers: FingerSet, pub right_fingers: FingerSet,
    pub waist: Joint, pub crotch: Joint,
    pub torso_lean: f32, pub torso_sway: f32,
    pub left_knee: Joint,  pub right_knee: Joint,
    pub left_ankle: Joint, pub right_ankle: Joint,
    pub head_tilt: f32, pub head_nod: f32, pub head_yaw: f32,
}

impl Pose {
    /// Move a joint using FABRIK algorithm - GUARANTEES fixed bone lengths
    /// handles both IK (reaching) and FK (dragging) depending on which joint is grabbed.
    pub fn move_joint_constrained(&mut self, name: &str, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton) {
        match name {
            "neck" => {
                let delta = (target.0 - self.neck.x, target.1 - self.neck.y, target.2 - self.neck.z);
                let old_crotch = self.crotch.xyz();
                
                // Translate entire upper body rigidly
                self.head.translate(delta.0, delta.1, delta.2);
                self.left_shoulder.translate(delta.0, delta.1, delta.2);
                self.right_shoulder.translate(delta.0, delta.1, delta.2);
                self.neck.set_xyz(target);
                
                // Drag arms with shoulders
                self.drag_arm("left", delta.0, delta.1, delta.2);
                self.drag_arm("right", delta.0, delta.1, delta.2);

                // BACKWARD PASS: constrain spine segments
                self.waist.set_xyz(Self::constrain_distance(target, self.waist.xyz(), sk.seg("torso_upper")));
                self.crotch.set_xyz(Self::constrain_distance(self.waist.xyz(), self.crotch.xyz(), sk.seg("torso_lower")));
                
                // Legs follow actual crotch movement
                let new_crotch = self.crotch.xyz();
                let crotch_delta = (new_crotch.0 - old_crotch.0, new_crotch.1 - old_crotch.1, new_crotch.2 - old_crotch.2);
                self.drag_leg("left", crotch_delta.0, crotch_delta.1, crotch_delta.2);
                self.drag_leg("right", crotch_delta.0, crotch_delta.1, crotch_delta.2);
            }
            "head" => {
                let neck = self.neck.xyz();
                let neck_len = sk.seg("neck");
                
                // Head must stay above neck (y <= neck.y since Y increases downward)
                let clamped_target = if target.1 > neck.1 {
                    // Trying to move head below neck - clamp to neck level
                    let min_y = neck.1 - neck_len;
                    (target.0, min_y, target.2)
                } else {
                    target
                };
                
                self.head.set_xyz(Self::constrain_distance(neck, clamped_target, neck_len));
            }
            "left_shoulder" => {
                self.move_shoulder("left", target, sk);
            }
            "right_shoulder" => {
                self.move_shoulder("right", target, sk);
            }
            "left_elbow" => {
                self.fabrik_left_arm(target, sk, 1); // 1 = elbow (mid)
            }
            "left_wrist" => {
                self.fabrik_left_arm(target, sk, 2); // 2 = wrist (end)
            }
            "right_elbow" => {
                self.fabrik_right_arm(target, sk, 1);
            }
            "right_wrist" => {
                self.fabrik_right_arm(target, sk, 2);
            }
            "waist" => {
                // Waist movement might pull the crotch. Check delta.
                let old_crotch = self.crotch.xyz();
                self.fabrik_torso(target, sk, 1);
                let new_crotch = self.crotch.xyz();
                let (dx, dy, dz) = (new_crotch.0 - old_crotch.0, new_crotch.1 - old_crotch.1, new_crotch.2 - old_crotch.2);
                self.drag_leg("left", dx, dy, dz);
                self.drag_leg("right", dx, dy, dz);
            }
            "crotch" => {
                // Crotch movement DEFINITELY pulls the legs.
                let old_crotch = self.crotch.xyz();
                self.fabrik_torso(target, sk, 2);
                let new_crotch = self.crotch.xyz();
                let (dx, dy, dz) = (new_crotch.0 - old_crotch.0, new_crotch.1 - old_crotch.1, new_crotch.2 - old_crotch.2);
                self.drag_leg("left", dx, dy, dz);
                self.drag_leg("right", dx, dy, dz);
            }
            "left_knee" => {
                self.fabrik_left_leg(target, sk, 1);
            }
            "left_ankle" => {
                self.fabrik_left_leg(target, sk, 2);
            }
            "right_knee" => {
                self.fabrik_right_leg(target, sk, 1);
            }
            "right_ankle" => {
                self.fabrik_right_leg(target, sk, 2);
            }
            _ => {}
        }
    }

    // --- Helper Logic ---

    /// specialized handler for shoulders to maintain width and connectivity
    fn move_shoulder(&mut self, side: &str, target: (f32,f32,f32), sk: &crate::skeleton::Skeleton) {
        let is_left = side == "left";
        let (active_shoulder, other_shoulder) = if is_left {
            (&mut self.left_shoulder, &mut self.right_shoulder)
        } else {
            (&mut self.right_shoulder, &mut self.left_shoulder)
        };

        let old_active_pos = active_shoulder.xyz();
        let old_other_pos = other_shoulder.xyz();
        let old_neck_pos = self.neck.xyz(); // Capture neck BEFORE it moves

        // 1. Set active shoulder to target
        active_shoulder.set_xyz(target);
        let new_active_pos = active_shoulder.xyz();

        // 2. Pull the OTHER shoulder to maintain shoulder_width
        let (ox, oy, oz) = old_other_pos;
        let dist_vec = (ox - new_active_pos.0, oy - new_active_pos.1, oz - new_active_pos.2);
        let current_dist = (dist_vec.0.powi(2) + dist_vec.1.powi(2) + dist_vec.2.powi(2)).sqrt();
        
        let width = sk.seg("shoulder_width");
        let new_other_pos = if current_dist > 0.001 {
            let r = width / current_dist;
            (new_active_pos.0 + dist_vec.0 * r, new_active_pos.1 + dist_vec.1 * r, new_active_pos.2 + dist_vec.2 * r)
        } else {
            (new_active_pos.0 + width, new_active_pos.1, new_active_pos.2)
        };
        other_shoulder.set_xyz(new_other_pos);

        // 3. Center Neck between new shoulders
        self.neck.set_xyz((
            (new_active_pos.0 + new_other_pos.0) / 2.0,
            (new_active_pos.1 + new_other_pos.1) / 2.0,
            (new_active_pos.2 + new_other_pos.2) / 2.0
        ));
        
        // --- FIX: DRAG HEAD WITH NECK ---
        let new_neck_pos = self.neck.xyz();
        let (ndx, ndy, ndz) = (new_neck_pos.0 - old_neck_pos.0, new_neck_pos.1 - old_neck_pos.1, new_neck_pos.2 - old_neck_pos.2);
        self.head.translate(ndx, ndy, ndz);
        // --------------------------------

        // 4. Drag the arms! 
        let active_delta = (new_active_pos.0 - old_active_pos.0, new_active_pos.1 - old_active_pos.1, new_active_pos.2 - old_active_pos.2);
        let other_delta = (new_other_pos.0 - old_other_pos.0, new_other_pos.1 - old_other_pos.1, new_other_pos.2 - old_other_pos.2);
        
        self.drag_arm(side, active_delta.0, active_delta.1, active_delta.2);
        self.drag_arm(if is_left { "right" } else { "left" }, other_delta.0, other_delta.1, other_delta.2);

        // 5. Reconcile Spine & Legs
        // If the neck moved, we must pull the waist (to keep upper torso length)
        // and pull the crotch (to keep lower torso length) and drag legs.
        let old_waist = self.waist.xyz();
        self.waist.set_xyz(Self::constrain_distance(self.neck.xyz(), old_waist, sk.seg("torso_upper")));
        
        let old_crotch = self.crotch.xyz();
        self.crotch.set_xyz(Self::constrain_distance(self.waist.xyz(), old_crotch, sk.seg("torso_lower")));

        let new_crotch = self.crotch.xyz();
        let (cdx, cdy, cdz) = (new_crotch.0 - old_crotch.0, new_crotch.1 - old_crotch.1, new_crotch.2 - old_crotch.2);
        
        self.drag_leg("left", cdx, cdy, cdz);
        self.drag_leg("right", cdx, cdy, cdz);
    }

    fn drag_arm(&mut self, side: &str, dx: f32, dy: f32, dz: f32) {
        if side == "left" {
            self.left_elbow.translate(dx, dy, dz);
            self.left_wrist.translate(dx, dy, dz);
        } else {
            self.right_elbow.translate(dx, dy, dz);
            self.right_wrist.translate(dx, dy, dz);
        }
    }

    fn drag_leg(&mut self, side: &str, dx: f32, dy: f32, dz: f32) {
        if side == "left" {
            self.left_knee.translate(dx, dy, dz);
            self.left_ankle.translate(dx, dy, dz);
        } else {
            self.right_knee.translate(dx, dy, dz);
            self.right_ankle.translate(dx, dy, dz);
        }
    }
    
    /// Solve leg IK with FABRIK - keeps foot planted, bends knee naturally
    // --- FABRIK Implementations ---

    fn fabrik_left_arm(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, target_idx: usize) {
        let mut chain = [self.left_shoulder.xyz(), self.left_elbow.xyz(), self.left_wrist.xyz()];
        let lengths = [sk.seg("arm"), sk.seg("forearm")];
        let pole_l = Vec3::new(-0.3, 0.0, 1.0).norm(); // faces forward, slight outward bias
        Self::fabrik_solve_constrained(&mut chain, &lengths, target, target_idx, |c| {
            Self::constrain_elbow(c, &sk.constraints.elbow, pole_l);
        });
        // chain[0] (shoulder) is the fixed root — do not write back to avoid drift
        self.left_elbow.set_xyz(chain[1]);
        self.left_wrist.set_xyz(chain[2]);
    }

    fn fabrik_right_arm(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, target_idx: usize) {
        let mut chain = [self.right_shoulder.xyz(), self.right_elbow.xyz(), self.right_wrist.xyz()];
        let lengths = [sk.seg("arm"), sk.seg("forearm")];
        let pole_r = Vec3::new( 0.3, 0.0, 1.0).norm(); // mirrored
        Self::fabrik_solve_constrained(&mut chain, &lengths, target, target_idx, |c| {
            Self::constrain_elbow(c, &sk.constraints.elbow, pole_r);
        });
        // chain[0] (shoulder) is the fixed root — do not write back to avoid drift
        self.right_elbow.set_xyz(chain[1]);
        self.right_wrist.set_xyz(chain[2]);
    }

    fn fabrik_torso(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, target_idx: usize) {
        let neck_locked = self.neck.xyz();
        let mut chain = [neck_locked, self.waist.xyz(), self.crotch.xyz()];
        let lengths = [sk.seg("torso_upper"), sk.seg("torso_lower")];
        
        // Pre-check: If target would violate ordering, clamp it to valid region
        let validated_target = match target_idx {
            1 => { // waist
                // Waist must be at least torso_upper distance below neck
                let min_y = neck_locked.1 + lengths[0];
                if target.1 < min_y {
                    (target.0, min_y, target.2)
                } else {
                    target
                }
            }
            2 => { // crotch  
                // Crotch must be at least torso_lower distance below waist
                let min_y = chain[1].1 + lengths[1];
                if target.1 < min_y {
                    (target.0, min_y, target.2)
                } else {
                    target
                }
            }
            _ => target
        };
        
        // Spine has no angle constraints — pass a no-op
        Self::fabrik_solve_constrained(&mut chain, &lengths, validated_target, target_idx, |_| {});
        
        // chain[0] (neck) is the fixed root — do not write back
        self.waist.set_xyz(chain[1]);
        self.crotch.set_xyz(chain[2]);
    }

    fn fabrik_left_leg(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, target_idx: usize) {
        let mut chain = [self.crotch.xyz(), self.left_knee.xyz(), self.left_ankle.xyz()];
        let lengths = [sk.seg("thigh"), sk.seg("shin")];
        let pole_fwd = Vec3::new(0.0, 0.0, 1.0);
        Self::fabrik_solve_constrained(&mut chain, &lengths, target, target_idx, |c| {
            Self::constrain_knee(c, &sk.constraints.knee, pole_fwd);
        });
        self.crotch.set_xyz(chain[0]);
        self.left_knee.set_xyz(chain[1]);
        self.left_ankle.set_xyz(chain[2]);
    }

    fn fabrik_right_leg(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, target_idx: usize) {
        let mut chain = [self.crotch.xyz(), self.right_knee.xyz(), self.right_ankle.xyz()];
        let lengths = [sk.seg("thigh"), sk.seg("shin")];
        let pole_fwd = Vec3::new(0.0, 0.0, 1.0);
        Self::fabrik_solve_constrained(&mut chain, &lengths, target, target_idx, |c| {
            Self::constrain_knee(c, &sk.constraints.knee, pole_fwd);
        });
        self.crotch.set_xyz(chain[0]);
        self.right_knee.set_xyz(chain[1]);
        self.right_ankle.set_xyz(chain[2]);
    }

    /// FABRIK with anatomical constraints enforced during solving
    fn fabrik_solve_constrained<F>(chain: &mut [(f32,f32,f32)], lengths: &[f32], target: (f32,f32,f32), target_idx: usize, constrain: F)
    where F: Fn(&mut [(f32,f32,f32)]) {
        if target_idx == 0 {
            chain[0] = target;
            for i in 0..chain.len()-1 {
                chain[i+1] = Self::constrain_distance(chain[i], chain[i+1], lengths[i]);
            }
            constrain(chain);
            return;
        }

        let root_fixed = chain[0];
        let target_v = Vec3::from_tuple(target);
        
        for _ in 0..6 {
            // Forward pass
            chain[target_idx] = target;
            for i in (1..=target_idx).rev() {
                chain[i-1] = Self::constrain_distance(chain[i], chain[i-1], lengths[i-1]);
            }
            constrain(chain);
            
            // Backward pass
            chain[0] = root_fixed;
            for i in 0..target_idx {
                chain[i+1] = Self::constrain_distance(chain[i], chain[i+1], lengths[i]);
            }
            constrain(chain);
            
            // Convergence check - stop early if close enough
            if Vec3::from_tuple(chain[target_idx]).distance(target_v) < 0.001 {
                break;
            }
        }

        // Drag tail
        for i in target_idx..chain.len()-1 {
            chain[i+1] = Self::constrain_distance(chain[i], chain[i+1], lengths[i]);
        }
        constrain(chain);
    }

    /// Hinge constraint with plane enforcement and hyperextension blocking.
    ///
    /// `pole` is the world-space direction the joint "faces" — the valid flexion side.
    /// For knees: (0,0,1) forward. For elbows: outward + slightly forward.
    ///
    /// Three steps per call (runs inside FABRIK loop, so 6× per drag frame):
    ///   1. Plane steering  — remove 40% of the out-of-plane component each iteration
    ///   2. Hyperextension  — hard-block: if bc opposes pole, zero that component out
    ///   3. Angle clamp     — soft-clamp to [min_deg=30, max_deg=180]
    ///                        180° = straight, 30° = max anatomical flexion
    fn constrain_hinge(chain: &mut [(f32,f32,f32)], constraint: &ConstraintDef, pole: Vec3) {
        if chain.len() < 3 { return; }

        let a = Vec3::from_tuple(chain[0]); // root  (shoulder / hip)
        let b = Vec3::from_tuple(chain[1]); // joint (elbow / knee)
        let c = Vec3::from_tuple(chain[2]); // end   (wrist / ankle)

        let lower_len = b.distance(c);
        if lower_len < 0.001 { return; }

        let ab     = a.sub(b).norm(); // joint→root direction
        let bc_cur = c.sub(b).norm(); // joint→end  direction

        // ── Step 1: Plane steering ───────────────────────────────────────────
        // Remove 40% of the component that takes bc out of the ab+pole plane.
        // Converges to ~88% in-plane over 6 FABRIK iterations — firm, not snappy.
        let bc_steered = {
            let plane_n = ab.cross(pole);
            if plane_n.len() > 0.01 {
                let n   = plane_n.norm();
                let out = n.scale(n.dot(bc_cur) * 0.40);
                let s   = bc_cur.sub(out);
                if s.len() > 0.01 { s.norm() } else { bc_cur }
            } else {
                bc_cur
            }
        };

        // ── Step 2: Block hyperextension ────────────────────────────────────
        // `pole` is the flexion direction. A negative dot product means bc is
        // pointing away from the flexion side — that's anatomically impossible.
        // Remove the negative component: projects back to "just straight" at worst.
        //
        // Concrete example (knee, pole = +Z forward):
        //   bc·pole > 0: shin forward  = valid flexion        → pass through
        //   bc·pole < 0: shin backward = hyperextension       → zero it out
        let flex_dot = bc_steered.dot(pole);
        let bc_blocked = if flex_dot < 0.0 {
            // Remove the hyperextension component entirely
            let projected = bc_steered.sub(pole.scale(flex_dot));
            if projected.len() > 0.01 { projected.norm() } else { ab.scale(-1.0) }
        } else {
            bc_steered
        };

        // ── Step 3: Angle clamp ──────────────────────────────────────────────
        let dot        = ab.dot(bc_blocked).clamp(-1.0, 1.0);
        let angle_deg  = dot.acos().to_degrees();
        let target_deg = Vec3::soft_clamp(angle_deg, constraint.min_deg, constraint.max_deg, constraint.softness);
        let target_rad = target_deg.to_radians();

        let cross  = ab.cross(bc_blocked);
        let new_bc = if cross.len() < 0.001 {
            let fb = ab.cross(pole);
            if fb.len() > 0.01 { ab.rotate_around_axis(fb.norm(), target_rad) } else { bc_blocked }
        } else if (target_deg - angle_deg).abs() > 0.01 {
            ab.rotate_around_axis(cross.norm(), target_rad)
        } else {
            bc_blocked
        };

        chain[2] = b.add(new_bc.scale(lower_len)).to_tuple();
    }


    pub fn constrain_elbow(chain: &mut [(f32,f32,f32)], limits: &crate::skeleton::AngleRange, pole: Vec3) {
        Self::constrain_hinge(chain, &ConstraintDef::hinge(limits.min, limits.max), pole);
    }

    pub fn constrain_knee(chain: &mut [(f32,f32,f32)], limits: &crate::skeleton::AngleRange, pole: Vec3) {
        Self::constrain_hinge(chain, &ConstraintDef::hinge(limits.min, limits.max), pole);
    }
    
    /// Cone constraint - for shoulder/hip with spherical motion limit
    #[allow(dead_code)]
    fn constrain_cone(chain: &mut [(f32,f32,f32)], constraint: &ConstraintDef) {
        if chain.len() != 3 { return; }
        
        let parent = Vec3::from_tuple(chain[0]).sub(Vec3::from_tuple(chain[1])).norm();
        let child = Vec3::from_tuple(chain[2]).sub(Vec3::from_tuple(chain[1])).norm();
        
        let max_deg = constraint.cone_angle.unwrap_or(90.0);
        let max_rad = max_deg.to_radians();
        let dot = parent.dot(child).clamp(-1.0, 1.0);
        let current_angle = dot.acos();
        let current_deg = current_angle.to_degrees();
        
        // Soft clamp the angle
        let target_deg = Vec3::soft_clamp(current_deg, 0.0, max_deg, constraint.softness);
        
        if (target_deg - current_deg).abs() < 0.01 { return; }
        
        let target_rad = target_deg.to_radians();
        let axis = parent.cross(child).norm();
        if axis.len() < 0.001 { return; }
        
        let new_dir = parent.rotate_around_axis(axis, target_rad);
        let len = Vec3::from_tuple(chain[2]).sub(Vec3::from_tuple(chain[1])).len();
        chain[2] = Vec3::from_tuple(chain[1]).add(new_dir.scale(len)).to_tuple();
    }
    
    /// Elliptical cone - for neck with asymmetric pitch/yaw limits
    #[allow(dead_code)]
    fn constrain_elliptical(chain: &mut [(f32,f32,f32)], constraint: &ConstraintDef) {
        if chain.len() != 3 { return; }
        
        let joint = Vec3::from_tuple(chain[1]);
        let end = Vec3::from_tuple(chain[2]);
        let dir = end.sub(joint).norm();
        
        let pitch = dir.y.asin().to_degrees();
        let yaw = dir.x.atan2(dir.z).to_degrees();
        
        // Soft clamp both pitch and yaw
        let cpitch = Vec3::soft_clamp(
            pitch,
            constraint.pitch_min.unwrap_or(-45.0),
            constraint.pitch_max.unwrap_or(45.0),
            constraint.softness
        );
        let cyaw = Vec3::soft_clamp(
            yaw,
            constraint.yaw_min.unwrap_or(-60.0),
            constraint.yaw_max.unwrap_or(60.0),
            constraint.softness
        );
        
        // Only apply if we actually changed something
        if (cpitch - pitch).abs() < 0.01 && (cyaw - yaw).abs() < 0.01 { return; }
        
        let new_dir = Vec3::new(
            cyaw.to_radians().sin(),
            cpitch.to_radians().sin(),
            cyaw.to_radians().cos()
        ).norm();
        
        let len = end.sub(joint).len();
        chain[2] = joint.add(new_dir.scale(len)).to_tuple();
    }

    /// Helper to place point `to` at distance `len` from point `from` (3D)
    fn constrain_distance(from: (f32,f32,f32), to: (f32,f32,f32), len: f32) -> (f32,f32,f32) {
        let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
        let d = (dx*dx + dy*dy + dz*dz).sqrt();
        if d < 0.001 {
            // Joints collapsed to the same point — return `to` unchanged.
            // The opposite FABRIK pass will pull it to the correct distance without
            // snapping to an arbitrary +X direction (which caused visible pops).
            return to;
        }
        let s = len / d;
        (from.0 + dx*s, from.1 + dy*s, from.2 + dz*s)
    }

    
    /// Calculate 3D distance between two points
    fn distance_3d(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
        let (dx, dy, dz) = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
    
    /// Normalize a 3D vector
    fn normalize_3d(v: (f32, f32, f32)) -> (f32, f32, f32) {
        let len = (v.0*v.0 + v.1*v.1 + v.2*v.2).sqrt();
        if len < 0.001 {
            (1.0, 0.0, 0.0)
        } else {
            (v.0 / len, v.1 / len, v.2 / len)
        }
    }

    /// Apply anatomical constraints to entire pose (call on drag release)
    pub fn apply_anatomical_constraints(&mut self, sk: &crate::skeleton::Skeleton) {
        // Enforce spine ordering: head must be above neck, waist below neck, crotch below waist
        // Y increases downward, so "above" means smaller Y value
        
        // Head above neck
        if self.head.y > self.neck.y {
            let dx = self.head.x - self.neck.x;
            let dz = self.head.z - self.neck.z;
            let horiz = (dx*dx + dz*dz).sqrt();
            let neck_len = sk.seg("neck");
            let max_drop = (neck_len*neck_len - horiz*horiz).max(0.0).sqrt();
            self.head.y = (self.neck.y - max_drop).min(self.head.y);
        }
        
        // Waist below neck
        if self.waist.y < self.neck.y + sk.seg("torso_upper") {
            self.waist.y = self.neck.y + sk.seg("torso_upper");
        }
        
        // Crotch below waist
        if self.crotch.y < self.waist.y + sk.seg("torso_lower") {
            self.crotch.y = self.waist.y + sk.seg("torso_lower");
        }
        
        // Enforce spine chain 3D distances while maintaining Y ordering
        let head_to_neck_dist = Self::distance_3d(self.head.xyz(), self.neck.xyz());
        if (head_to_neck_dist - sk.seg("neck")).abs() > 0.1 {
            let corrected = Self::constrain_distance(self.neck.xyz(), self.head.xyz(), sk.seg("neck"));
            // Ensure head stays above neck after distance correction
            if corrected.1 > self.neck.y {
                // Head went below neck - clamp it
                let dx = corrected.0 - self.neck.x;
                let dz = corrected.2 - self.neck.z;
                let horiz = (dx*dx + dz*dz).sqrt();
                let neck_len = sk.seg("neck");
                let vert = (neck_len*neck_len - horiz*horiz).max(0.0).sqrt();
                self.head.set_xyz((corrected.0, self.neck.y - vert, corrected.2));
            } else {
                self.head.set_xyz(corrected);
            }
        }
        
        let neck_to_waist_dist = Self::distance_3d(self.neck.xyz(), self.waist.xyz());
        if (neck_to_waist_dist - sk.seg("torso_upper")).abs() > 0.1 {
            let corrected = Self::constrain_distance(self.neck.xyz(), self.waist.xyz(), sk.seg("torso_upper"));
            // Ensure waist stays below neck after distance correction
            if corrected.1 < self.neck.y + sk.seg("torso_upper") {
                self.waist.y = self.neck.y + sk.seg("torso_upper");
            } else {
                self.waist.set_xyz(corrected);
            }
        }
        
        let waist_to_crotch_dist = Self::distance_3d(self.waist.xyz(), self.crotch.xyz());
        if (waist_to_crotch_dist - sk.seg("torso_lower")).abs() > 0.1 {
            let corrected = Self::constrain_distance(self.waist.xyz(), self.crotch.xyz(), sk.seg("torso_lower"));
            // Ensure crotch stays below waist after distance correction
            if corrected.1 < self.waist.y + sk.seg("torso_lower") {
                self.crotch.y = self.waist.y + sk.seg("torso_lower");
            } else {
                self.crotch.set_xyz(corrected);
            }
        }
        
        let pole_l   = Vec3::new(-0.3, 0.0, 1.0).norm();
        let pole_r   = Vec3::new( 0.3, 0.0, 1.0).norm();
        let pole_fwd = Vec3::new( 0.0, 0.0, 1.0);

        // Left arm chain
        let mut chain = [self.left_shoulder.xyz(), self.left_elbow.xyz(), self.left_wrist.xyz()];
        Self::constrain_elbow(&mut chain, &sk.constraints.elbow, pole_l);
        self.left_elbow.set_xyz(chain[1]);
        self.left_wrist.set_xyz(chain[2]);

        // Right arm chain
        let mut chain = [self.right_shoulder.xyz(), self.right_elbow.xyz(), self.right_wrist.xyz()];
        Self::constrain_elbow(&mut chain, &sk.constraints.elbow, pole_r);
        self.right_elbow.set_xyz(chain[1]);
        self.right_wrist.set_xyz(chain[2]);

        // Left leg chain
        let mut chain = [self.crotch.xyz(), self.left_knee.xyz(), self.left_ankle.xyz()];
        Self::constrain_knee(&mut chain, &sk.constraints.knee, pole_fwd);
        self.left_knee.set_xyz(chain[1]);
        self.left_ankle.set_xyz(chain[2]);

        // Right leg chain
        let mut chain = [self.crotch.xyz(), self.right_knee.xyz(), self.right_ankle.xyz()];
        Self::constrain_knee(&mut chain, &sk.constraints.knee, pole_fwd);
        self.right_knee.set_xyz(chain[1]);
        self.right_ankle.set_xyz(chain[2]);
    }

    /// Simple debug - print all joint positions
    pub fn debug_all_joints(&self, label: &str) {
        println!("\n═══ {} ═══", label);
        for (name, j) in [
            ("head", &self.head), ("neck", &self.neck),
            ("L_shoulder", &self.left_shoulder), ("R_shoulder", &self.right_shoulder),
            ("L_elbow", &self.left_elbow), ("R_elbow", &self.right_elbow),
            ("L_wrist", &self.left_wrist), ("R_wrist", &self.right_wrist),
            ("waist", &self.waist), ("crotch", &self.crotch),
            ("L_knee", &self.left_knee), ("R_knee", &self.right_knee),
            ("L_ankle", &self.left_ankle), ("R_ankle", &self.right_ankle),
        ] {
            println!("  {:12} ({:7.1}, {:7.1}, {:7.1})", name, j.x, j.y, j.z);
        }
    }

    pub fn joint_mut(&mut self, name: &str) -> Option<&mut Joint> {
        Some(match name {
            "head"           => &mut self.head,
            "neck"           => &mut self.neck,
            "left_shoulder"  => &mut self.left_shoulder,
            "right_shoulder" => &mut self.right_shoulder,
            "left_elbow"     => &mut self.left_elbow,
            "right_elbow"    => &mut self.right_elbow,
            "left_wrist"     => &mut self.left_wrist,
            "right_wrist"    => &mut self.right_wrist,
            "waist"          => &mut self.waist,
            "crotch"         => &mut self.crotch,
            "left_knee"      => &mut self.left_knee,
            "right_knee"     => &mut self.right_knee,
            "left_ankle"     => &mut self.left_ankle,
            "right_ankle"    => &mut self.right_ankle,
            _                => return None,
        })
    }
}
// pose.rs — 3D: X left→right, Y bottom→top, Z viewer→scene
use serde::{Deserialize, Serialize};

// ========== Vec3 helpers for FABRIK ==========
#[derive(Copy, Clone)]
struct Vec3 { x: f32, y: f32, z: f32 }

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    fn from_tuple(t: (f32, f32, f32)) -> Self { Self::new(t.0, t.1, t.2) }

    fn dot(self, o: Self) -> f32 { self.x*o.x + self.y*o.y + self.z*o.z }
    fn len(self) -> f32 { self.dot(self).sqrt() }
    fn sub(self, o: Self) -> Self { Self::new(self.x-o.x, self.y-o.y, self.z-o.z) }
    fn distance(self, o: Self) -> f32 { self.sub(o).len() }
}
// ========== End Vec3 helpers ==========


#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Joint {
    pub x: f32, pub y: f32,
    #[serde(default)] pub z: f32,
    pub angle: f32,
}

impl std::hash::Hash for Joint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.z.to_bits().hash(state);
        self.angle.to_bits().hash(state);
    }
}

impl Joint {
    pub fn new_3d(x: f32, y: f32, z: f32) -> Self { Self { x, y, z, angle: 0.0 } }

    pub fn set_xyz(&mut self, (x, y, z): (f32, f32, f32)) { self.x = x; self.y = y; self.z = z; }
    pub fn xyz(&self) -> (f32, f32, f32) { (self.x, self.y, self.z) }

    pub fn translate(&mut self, dx: f32, dy: f32, dz: f32) {
        self.x += dx; self.y += dy; self.z += dz;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerSet {
    pub thumb: f32, pub index: f32, pub middle: f32,
    pub ring: f32,  pub pinky: f32, pub spread: f32,
}

impl std::hash::Hash for FingerSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.thumb.to_bits().hash(state);  self.index.to_bits().hash(state);
        self.middle.to_bits().hash(state); self.ring.to_bits().hash(state);
        self.pinky.to_bits().hash(state);  self.spread.to_bits().hash(state);
    }
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

impl std::hash::Hash for Pose {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.head.hash(state);           self.neck.hash(state);
        self.left_shoulder.hash(state);  self.right_shoulder.hash(state);
        self.left_elbow.hash(state);     self.right_elbow.hash(state);
        self.left_wrist.hash(state);     self.right_wrist.hash(state);
        self.left_fingers.hash(state);   self.right_fingers.hash(state);
        self.waist.hash(state);          self.crotch.hash(state);
        self.torso_lean.to_bits().hash(state);
        self.torso_sway.to_bits().hash(state);
        self.left_knee.hash(state);      self.right_knee.hash(state);
        self.left_ankle.hash(state);     self.right_ankle.hash(state);
        self.head_tilt.to_bits().hash(state);
        self.head_nod.to_bits().hash(state);
        self.head_yaw.to_bits().hash(state);
    }
}

impl Pose {
    /// Move a joint, maintaining bone lengths via FABRIK.
    /// No angle constraints — pose freely; semantics handles interpretation.
    pub fn move_joint(&mut self, name: &str, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton) {
        match name {
            "neck" => {
                self.ragdoll_from_neck(target, sk);
            }
            "head" => {
                self.head.set_xyz(Self::fix_dist(self.neck.xyz(), target, sk.seg("neck")));
            }
            "left_shoulder"  => self.move_shoulder("left",  target, sk),
            "right_shoulder" => self.move_shoulder("right", target, sk),
            "left_elbow"     => self.fabrik_left_arm(target,  sk, 1),
            "left_wrist"     => self.fabrik_left_arm(target,  sk, 2),
            "right_elbow"    => self.fabrik_right_arm(target, sk, 1),
            "right_wrist"    => self.fabrik_right_arm(target, sk, 2),
            "waist" => {
                let old_crotch = self.crotch.xyz();
                self.fabrik_torso(target, sk, 1);
                let nc = self.crotch.xyz();
                let cd = (nc.0-old_crotch.0, nc.1-old_crotch.1, nc.2-old_crotch.2);
                self.drag_leg("left",  cd.0, cd.1, cd.2);
                self.drag_leg("right", cd.0, cd.1, cd.2);
            }
            "crotch" => {
                let old_crotch = self.crotch.xyz();
                self.fabrik_torso(target, sk, 2);
                let nc = self.crotch.xyz();
                let cd = (nc.0-old_crotch.0, nc.1-old_crotch.1, nc.2-old_crotch.2);
                self.drag_leg("left",  cd.0, cd.1, cd.2);
                self.drag_leg("right", cd.0, cd.1, cd.2);
            }
            "left_knee"   => self.fabrik_left_leg(target,  sk, 1),
            "left_ankle"  => self.fabrik_left_leg(target,  sk, 2),
            "right_knee"  => self.fabrik_right_leg(target, sk, 1),
            "right_ankle" => self.fabrik_right_leg(target, sk, 2),
            _ => {}
        }
        self.clamp_to_floor();
    }

    /// Clamp every joint so nothing sinks below the ankle plane.
    /// Y increases downward in Pose space, so "below floor" means y > floor_y.
    /// The ankles define the floor and are never clamped themselves.
    fn clamp_to_floor(&mut self) {
        let floor_y = self.left_ankle.y.max(self.right_ankle.y);
        for j in [
            &mut self.head, &mut self.neck,
            &mut self.left_shoulder,  &mut self.right_shoulder,
            &mut self.left_elbow,     &mut self.right_elbow,
            &mut self.left_wrist,     &mut self.right_wrist,
            &mut self.waist,          &mut self.crotch,
            &mut self.left_knee,      &mut self.right_knee,
        ] {
            if j.y > floor_y { j.y = floor_y; }
        }
    }

    // ── Shoulder ─────────────────────────────────────────────────────────────

    fn move_shoulder(&mut self, side: &str, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton) {
        let is_left    = side == "left";
        let old_active = if is_left { self.left_shoulder.xyz()  } else { self.right_shoulder.xyz() };
        let old_other  = if is_left { self.right_shoulder.xyz() } else { self.left_shoulder.xyz()  };
        let old_neck   = self.neck.xyz();

        if is_left { self.left_shoulder.set_xyz(target); } else { self.right_shoulder.set_xyz(target); }

        // Pull other shoulder to maintain width
        let width = sk.seg("shoulder_width");
        let diff  = (old_other.0-target.0, old_other.1-target.1, old_other.2-target.2);
        let d     = (diff.0*diff.0 + diff.1*diff.1 + diff.2*diff.2).sqrt();
        let new_other = if d > 0.001 {
            let r = width / d;
            (target.0+diff.0*r, target.1+diff.1*r, target.2+diff.2*r)
        } else {
            (target.0 + width, target.1, target.2)
        };
        if is_left { self.right_shoulder.set_xyz(new_other); } else { self.left_shoulder.set_xyz(new_other); }

        // Center neck between shoulders and drag head with it
        let new_neck = ((target.0+new_other.0)/2.0, (target.1+new_other.1)/2.0, (target.2+new_other.2)/2.0);
        self.neck.set_xyz(new_neck);
        let nd = (new_neck.0-old_neck.0, new_neck.1-old_neck.1, new_neck.2-old_neck.2);
        self.head.translate(nd.0, nd.1, nd.2);

        // Drag arms
        let ad = (target.0-old_active.0,  target.1-old_active.1,  target.2-old_active.2);
        let od = (new_other.0-old_other.0, new_other.1-old_other.1, new_other.2-old_other.2);
        self.drag_arm(side,                                      ad.0, ad.1, ad.2);
        self.drag_arm(if is_left { "right" } else { "left" },   od.0, od.1, od.2);

        // Pull spine and legs
        let old_crotch = self.crotch.xyz();
        self.waist.set_xyz(Self::fix_dist(new_neck, self.waist.xyz(), sk.seg("torso_upper")));
        self.crotch.set_xyz(Self::fix_dist(self.waist.xyz(), self.crotch.xyz(), sk.seg("torso_lower")));
        let nc = self.crotch.xyz();
        let cd = (nc.0-old_crotch.0, nc.1-old_crotch.1, nc.2-old_crotch.2);
        self.drag_leg("left",  cd.0, cd.1, cd.2);
        self.drag_leg("right", cd.0, cd.1, cd.2);
    }

    // ── Drag helpers ─────────────────────────────────────────────────────────

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

    // ── FABRIK chains ─────────────────────────────────────────────────────────

    fn fabrik_left_arm(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, idx: usize) {
        let mut chain = [self.left_shoulder.xyz(), self.left_elbow.xyz(), self.left_wrist.xyz()];
        Self::fabrik_solve(&mut chain, &[sk.seg("arm"), sk.seg("forearm")], target, idx);
        self.left_elbow.set_xyz(chain[1]);
        self.left_wrist.set_xyz(chain[2]);
    }

    fn fabrik_right_arm(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, idx: usize) {
        let mut chain = [self.right_shoulder.xyz(), self.right_elbow.xyz(), self.right_wrist.xyz()];
        Self::fabrik_solve(&mut chain, &[sk.seg("arm"), sk.seg("forearm")], target, idx);
        self.right_elbow.set_xyz(chain[1]);
        self.right_wrist.set_xyz(chain[2]);
    }

    fn fabrik_torso(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, idx: usize) {
        let mut chain = [self.neck.xyz(), self.waist.xyz(), self.crotch.xyz()];
        Self::fabrik_solve(&mut chain, &[sk.seg("torso_upper"), sk.seg("torso_lower")], target, idx);
        // chain[0] (neck) is fixed root — don't write back
        self.waist.set_xyz(chain[1]);
        self.crotch.set_xyz(chain[2]);
    }

    fn fabrik_left_leg(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, idx: usize) {
        let mut chain = [self.crotch.xyz(), self.left_knee.xyz(), self.left_ankle.xyz()];
        Self::fabrik_solve(&mut chain, &[sk.seg("thigh"), sk.seg("shin")], target, idx);
        self.crotch.set_xyz(chain[0]);
        self.left_knee.set_xyz(chain[1]);
        self.left_ankle.set_xyz(chain[2]);
    }

    fn fabrik_right_leg(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton, idx: usize) {
        let mut chain = [self.crotch.xyz(), self.right_knee.xyz(), self.right_ankle.xyz()];
        Self::fabrik_solve(&mut chain, &[sk.seg("thigh"), sk.seg("shin")], target, idx);
        self.crotch.set_xyz(chain[0]);
        self.right_knee.set_xyz(chain[1]);
        self.right_ankle.set_xyz(chain[2]);
    }

    /// Pure FABRIK — bone lengths only, no angle constraints.
    fn fabrik_solve(chain: &mut [(f32,f32,f32)], lengths: &[f32], target: (f32,f32,f32), target_idx: usize) {
        if target_idx == 0 {
            chain[0] = target;
            for i in 0..chain.len()-1 {
                chain[i+1] = Self::fix_dist(chain[i], chain[i+1], lengths[i]);
            }
            return;
        }

        let root     = chain[0];
        let target_v = Vec3::from_tuple(target);

        for _ in 0..6 {
            // Forward: pull toward target
            chain[target_idx] = target;
            for i in (1..=target_idx).rev() {
                chain[i-1] = Self::fix_dist(chain[i], chain[i-1], lengths[i-1]);
            }
            // Backward: re-anchor root
            chain[0] = root;
            for i in 0..target_idx {
                chain[i+1] = Self::fix_dist(chain[i], chain[i+1], lengths[i]);
            }
            if Vec3::from_tuple(chain[target_idx]).distance(target_v) < 0.001 { break; }
        }

        // Extend tail past solved joints
        for i in target_idx..chain.len()-1 {
            chain[i+1] = Self::fix_dist(chain[i], chain[i+1], lengths[i]);
        }
    }


    // ── Ragdoll from neck ─────────────────────────────────────────────────────
    //
    // The neck is the root anchor. Every joint in the skeleton gets a fraction
    // of the neck's delta based on its "chain depth" from the neck — joints
    // close to the neck follow nearly 1:1, joints at the extremities barely
    // follow at all and instead sag downward under simulated gravity.
    //
    // Chain weights (how much of the neck delta each joint inherits):
    //
    //   neck            1.00  ← the anchor, moves exactly to target
    //   head            0.95  ← almost locked to neck
    //   shoulders       0.88  ← close, slight lag
    //   elbows          0.55  ← mid-arm, noticeable sag
    //   wrists          0.25  ← hangs loosely
    //   waist           0.75  ← spine follows well
    //   crotch          0.55  ← lower spine lags more
    //   knees           0.30  ← legs swing freely
    //   ankles          0.10  ← feet barely care
    //
    // After soft-following, every bone length is re-enforced with fix_dist so
    // the skeleton never stretches. Limbs also get a minimum spread so they
    // don't fully collapse and become impossible to grab.
    fn ragdoll_from_neck(&mut self, target: (f32, f32, f32), sk: &crate::skeleton::Skeleton) {
        let (dx, dy, dz) = (
            target.0 - self.neck.x,
            target.1 - self.neck.y,
            target.2 - self.neck.z,
        );

        // Gravity sag: extra downward pull scaled by horizontal distance moved.
        // Makes limbs droop naturally when the neck swings sideways.
        let horiz_dist  = (dx*dx + dz*dz).sqrt();
        let gravity_sag = horiz_dist * 0.18; // tune: bigger = floppier

        // Helper: apply weighted delta + gravity to a joint
        let soft = |pos: (f32,f32,f32), w: f32, grav: f32| -> (f32,f32,f32) {
            (pos.0 + dx*w, pos.1 + dy*w + grav, pos.2 + dz*w)
        };

        // ── Move neck ────────────────────────────────────────────────────────
        self.neck.set_xyz(target);

        // ── Head (tight) ─────────────────────────────────────────────────────
        self.head.set_xyz(soft(self.head.xyz(), 0.95, 0.0));

        // ── Shoulders (close follow) ─────────────────────────────────────────
        let ls = soft(self.left_shoulder.xyz(),  0.88, gravity_sag * 0.1);
        let rs = soft(self.right_shoulder.xyz(), 0.88, gravity_sag * 0.1);
        self.left_shoulder.set_xyz(ls);
        self.right_shoulder.set_xyz(rs);

        // ── Arms (progressively looser) ──────────────────────────────────────
        let le = soft(self.left_elbow.xyz(),   0.55, gravity_sag * 0.5);
        let re = soft(self.right_elbow.xyz(),  0.55, gravity_sag * 0.5);
        let lw = soft(self.left_wrist.xyz(),   0.25, gravity_sag * 0.9);
        let rw = soft(self.right_wrist.xyz(),  0.25, gravity_sag * 0.9);
        self.left_elbow.set_xyz(le);
        self.right_elbow.set_xyz(re);
        self.left_wrist.set_xyz(lw);
        self.right_wrist.set_xyz(rw);

        // ── Spine (pulls down from neck) ─────────────────────────────────────
        let wa = soft(self.waist.xyz(),  0.75, gravity_sag * 0.2);
        let cr = soft(self.crotch.xyz(), 0.55, gravity_sag * 0.35);
        self.waist.set_xyz(wa);
        self.crotch.set_xyz(cr);

        // ── Legs (swing freely) ──────────────────────────────────────────────
        let lk = soft(self.left_knee.xyz(),   0.30, gravity_sag * 0.7);
        let rk = soft(self.right_knee.xyz(),  0.30, gravity_sag * 0.7);
        let la = soft(self.left_ankle.xyz(),  0.10, gravity_sag * 1.0);
        let ra = soft(self.right_ankle.xyz(), 0.10, gravity_sag * 1.0);
        self.left_knee.set_xyz(lk);
        self.right_knee.set_xyz(rk);
        self.left_ankle.set_xyz(la);
        self.right_ankle.set_xyz(ra);

        // ── Re-enforce all bone lengths (skeleton never stretches) ───────────
        let neck = self.neck.xyz();

        // Head
        self.head.set_xyz(Self::fix_dist(neck, self.head.xyz(), sk.seg("neck")));

        // Shoulders: the shoulder bar is always centred on the neck.
        // Take the current shoulder direction (from the soft-moved positions)
        // to preserve the tilt/angle the user posed them at, but anchor the
        // midpoint exactly at neck so the clavicle never detaches.
        let ls = self.left_shoulder.xyz();
        let rs = self.right_shoulder.xyz();
        let ld = (ls.0-rs.0, ls.1-rs.1, ls.2-rs.2); // left→right direction
        let d  = (ld.0*ld.0 + ld.1*ld.1 + ld.2*ld.2).sqrt();
        let half_w = sk.seg("shoulder_width") / 2.0;
        // neck IS the shoulder midpoint — spread left and right from it
        if d > 0.001 {
            let s = half_w / d;
            self.left_shoulder.set_xyz( (neck.0 + ld.0*s, neck.1 + ld.1*s, neck.2 + ld.2*s));
            self.right_shoulder.set_xyz((neck.0 - ld.0*s, neck.1 - ld.1*s, neck.2 - ld.2*s));
        } else {
            // Shoulders collapsed — spread them horizontally from neck
            self.left_shoulder.set_xyz( (neck.0 - half_w, neck.1, neck.2));
            self.right_shoulder.set_xyz((neck.0 + half_w, neck.1, neck.2));
        }

        // Arms
        let ls = self.left_shoulder.xyz();
        self.left_elbow.set_xyz(Self::fix_dist(ls, self.left_elbow.xyz(), sk.seg("arm")));
        let le = self.left_elbow.xyz();
        self.left_wrist.set_xyz(Self::spread_fix(le, self.left_wrist.xyz(), sk.seg("forearm")));

        let rs = self.right_shoulder.xyz();
        self.right_elbow.set_xyz(Self::fix_dist(rs, self.right_elbow.xyz(), sk.seg("arm")));
        let re = self.right_elbow.xyz();
        self.right_wrist.set_xyz(Self::spread_fix(re, self.right_wrist.xyz(), sk.seg("forearm")));

        // Spine
        self.waist.set_xyz(Self::fix_dist(neck, self.waist.xyz(), sk.seg("torso_upper")));
        let wa = self.waist.xyz();
        self.crotch.set_xyz(Self::fix_dist(wa, self.crotch.xyz(), sk.seg("torso_lower")));

        // Legs
        let cr = self.crotch.xyz();
        self.left_knee.set_xyz(Self::fix_dist(cr, self.left_knee.xyz(), sk.seg("thigh")));
        let lk = self.left_knee.xyz();
        self.left_ankle.set_xyz(Self::spread_fix(lk, self.left_ankle.xyz(), sk.seg("shin")));

        self.right_knee.set_xyz(Self::fix_dist(cr, self.right_knee.xyz(), sk.seg("thigh")));
        let rk = self.right_knee.xyz();
        self.right_ankle.set_xyz(Self::spread_fix(rk, self.right_ankle.xyz(), sk.seg("shin")));
    }

    /// Place `to` at exactly `len` from `from`, preserving direction.
    fn fix_dist(from: (f32,f32,f32), to: (f32,f32,f32), len: f32) -> (f32,f32,f32) {
        let (dx, dy, dz) = (to.0-from.0, to.1-from.1, to.2-from.2);
        let d = (dx*dx + dy*dy + dz*dz).sqrt();
        if d < 0.001 { return (from.0, from.1 + len, from.2); }
        let s = len / d;
        (from.0+dx*s, from.1+dy*s, from.2+dz*s)
    }

    /// Like fix_dist but also enforces a minimum spread (35% of bone length)
    /// so ragdolled limbs never collapse to a point and become ungrabbable.
    fn spread_fix(from: (f32,f32,f32), to: (f32,f32,f32), len: f32) -> (f32,f32,f32) {
        let (dx, dy, dz) = (to.0-from.0, to.1-from.1, to.2-from.2);
        let d = (dx*dx + dy*dy + dz*dz).sqrt();
        // If joints collapsed too close together, push the child straight down
        // (gravity direction) at minimum spread distance — easy to grab and drag.
        let min_spread = len * 0.35;
        if d < min_spread {
            return (from.0, from.1 + len, from.2); // hang straight down
        }
        let s = len / d;
        (from.0+dx*s, from.1+dy*s, from.2+dz*s)
    }

}
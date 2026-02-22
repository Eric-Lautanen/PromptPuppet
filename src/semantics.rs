// semantics.rs  (kinematics → natural-language pose description)
// Converts a manually-posed Pose into concise prompt text for AI image/video generators.
//
// Coordinate space (screen coords at scale=40):
//   X : negative = character's LEFT,  positive = character's RIGHT
//   Y : SMALLER  = higher on screen  (Y increases downward)
//   Z : negative = toward viewer,    positive = into scene
//
// All spatial reasoning is done in a body-relative frame via BodyMetrics so
// that ground height, torso proportions, and shoulder width are consistent
// reference units across every classification function.
//
// Per-limb body-relative sign convention (shared by arms and legs):
//   sign = +1 for right limbs, −1 for left limbs.
//   Multiplying X by sign makes "outward from body" always +, "inward" always −.
//   This keeps left/right arm and leg logic symmetric around identical thresholds.

use crate::pose::Pose;

pub fn describe(pose: &Pose) -> String {
    let m = BodyMetrics::new(pose);
    let mut parts: Vec<String> = Vec::new();
    let stance_str = stance(pose, &m);
    parts.push(stance_str.clone());
    let is_lying = stance_str.starts_with("lying");
    // Torso lean/twist are meaningless when lying — and actively harmful: the
    // body is horizontal so |neck.y − crotch.y| collapses to near-zero, causing
    // the lean calculation to divide by ~1 px and produce huge spurious angles.
    if !is_lying {
        if let Some(s) = torso_lean(pose)   { parts.push(s); }
        if let Some(s) = torso_twist(pose)  { parts.push(s); }
    }
    if let Some(s) = weight_shift(pose, &m, &stance_str) { parts.push(s); }
    if let Some(s) = head_orient(pose)      { parts.push(s); }
    if let Some(s) = arms(pose, &m)         { parts.push(s); }
    if let Some(s) = legs(pose, &m, &stance_str) { parts.push(s); }
    parts.join(", ")
}

// ─── Body reference frame ─────────────────────────────────────────────────────

struct BodyMetrics {
    /// Y of the lower ankle — ground reference in screen coords.
    floor_y:    f32,
    /// Total height from head to floor in pixels.
    body_h:     f32,
    /// Neck-to-crotch height — used as a proportional unit (~108 px at scale=40).
    torso_h:    f32,
    /// Left-to-right shoulder separation.
    shoulder_w: f32,
    /// Key landmark Y positions (screen coords — smaller = higher on screen).
    neck_y:     f32,
    shoulder_y: f32,   // avg of both shoulders
    hip_y:      f32,   // crotch joint
}

impl BodyMetrics {
    fn new(p: &Pose) -> Self {
        let floor_y   = p.left_ankle.y.max(p.right_ankle.y);
        let body_h    = (floor_y - p.head.y).abs().max(1.0);
        let shoulder_y = (p.left_shoulder.y + p.right_shoulder.y) / 2.0;
        let torso_h   = (p.crotch.y - p.neck.y).abs().max(1.0);
        let shoulder_w = (p.left_shoulder.x - p.right_shoulder.x).abs().max(1.0);
        Self { floor_y, body_h, torso_h, shoulder_w, neck_y: p.neck.y,
               shoulder_y, hip_y: p.crotch.y }
    }

    /// Pixels above the floor. Positive = elevated; 0 = on the ground.
    fn above_floor(&self, y: f32) -> f32 { self.floor_y - y }

    /// Height above floor as fraction of total body height (0 = floor, 1 = head).
    fn height_frac(&self, y: f32) -> f32 { self.above_floor(y) / self.body_h }

    /// Name the body level corresponding to a Y position.
    fn level_name(&self, y: f32) -> &'static str {
        let h = self.height_frac(y);
        if      h > 0.92 { "overhead"              }
        else if h > 0.82 { "above shoulder"        }
        else if h > 0.70 { "at shoulder level"     }
        else if h > 0.57 { "at chest level"        }
        else if h > 0.44 { "at waist level"        }
        else if h > 0.32 { "at hip level"          }
        else if h > 0.20 { "at thigh level"        }
        else if h > 0.08 { "at knee level"         }
        else             { "below knee"             }
    }

    /// Describe how high a raised foot is, relative to total body height.
    fn foot_raise_desc(&self, ankle_y: f32) -> &'static str {
        let h = self.height_frac(ankle_y);
        if      h > 0.50 { "raised high above hip"   }
        else if h > 0.32 { "raised to hip height"    }
        else if h > 0.16 { "raised to knee height"   }
        else             { "slightly raised"          }
    }

    /// Describe foot spread relative to shoulder width.
    fn foot_spread(&self, left_ankle_x: f32, right_ankle_x: f32) -> &'static str {
        let spread = (left_ankle_x - right_ankle_x).abs();
        let ratio  = spread / self.shoulder_w;
        if      ratio < 0.40 { "feet together"        }
        else if ratio < 0.90 { "feet hip-width apart" }
        else if ratio < 1.60 { "feet wide apart"      }
        else                 { "feet very wide apart"  }
    }
}

// ─── Vector helpers ───────────────────────────────────────────────────────────

type V3 = (f32, f32, f32);

#[inline] fn sub(a: V3, b: V3) -> V3 { (a.0-b.0, a.1-b.1, a.2-b.2) }
#[inline] fn dot(a: V3, b: V3) -> f32 { a.0*b.0 + a.1*b.1 + a.2*b.2 }
#[inline] fn mag(a: V3) -> f32 { (a.0*a.0 + a.1*a.1 + a.2*a.2).sqrt() }
#[inline] fn norm(a: V3) -> V3 { let m = mag(a).max(1e-6); (a.0/m, a.1/m, a.2/m) }

/// Angle (degrees) at vertex `b`.  180 = straight, 90 = right angle.
fn angle_at(a: V3, b: V3, c: V3) -> f32 {
    dot(norm(sub(a, b)), norm(sub(c, b))).clamp(-1.0, 1.0).acos().to_degrees()
}

// ─── Stance ───────────────────────────────────────────────────────────────────

/// Direction suffix for a raised foot, using the hip→ankle vector in body-relative space.
/// `sign`: +1 for right limb, −1 for left limb (so "outward" is always positive lateral).
/// Returns a string like " to the side", " forward", " behind", or "" for straight up.
fn raised_foot_dir(hip: V3, ankle: V3, sign: f32) -> &'static str {
    let ha   = sub(ankle, hip);
    let ha_m = mag(ha).max(1e-6);
    let fwd  =  ha.2 / ha_m;            // +1 = into scene = character's forward
    let lat  =  ha.0 * sign / ha_m;     // +1 = outward from body centre
    let up   = -ha.1 / ha_m;            // +1 = ankle above hip

    // atan2-based horizontal angle: 0° = forward, 90° = outward, ±180° = behind.
    let h_mag   = (fwd * fwd + lat * lat).sqrt().max(1e-6);
    let h_angle = lat.atan2(fwd).to_degrees();
    let elev    = up.atan2(h_mag).to_degrees();

    // If the leg is nearly straight up (elevation ≥ 65°) direction is ambiguous — omit.
    if elev >= 65.0 { return ""; }

    // Use 45°-wide bands centred on the four cardinal directions.
    if h_angle >  45.0 && h_angle < 135.0 { " to the side" }
    else if h_angle.abs() < 45.0          { " forward"     }
    else                                   { " behind"      }
}

fn stance(p: &Pose, m: &BodyMetrics) -> String {
    // Lying: body nearly horizontal — head and ankles at very similar Y.
    if m.body_h < 80.0 {
        // Side-lying: head is offset laterally from the crotch by more than the
        // body is tall. Use shoulder X spread as a sanity reference.
        let lateral_offset = (p.head.x - p.crotch.x).abs();
        if lateral_offset > m.body_h * 0.40 {
            let side = if p.head.x < p.crotch.x { "left" } else { "right" };
            return format!("lying on {side} side");
        }
        let face = if p.head.z <= p.crotch.z { "face up" } else { "face down" };
        return format!("lying {face}");
    }

    // Knee angles (angle AT the knee joint — 180 = straight, 90 = bent).
    let l_ka   = angle_at(p.crotch.xyz(), p.left_knee.xyz(),  p.left_ankle.xyz());
    let r_ka   = angle_at(p.crotch.xyz(), p.right_knee.xyz(), p.right_ankle.xyz());
    let l_bent = l_ka < 120.0;
    let r_bent = r_ka < 120.0;

    // Shin direction — the key to distinguishing sitting/kneeling/crouching.
    let l_shin_down = p.left_ankle.y  > p.left_knee.y  + 20.0; // foot below knee
    let r_shin_down = p.right_ankle.y > p.right_knee.y + 20.0;
    let l_shin_back = p.left_ankle.z  > p.left_knee.z  + 20.0; // foot behind knee
    let r_shin_back = p.right_ankle.z > p.right_knee.z + 20.0;

    let crotch_h  = m.height_frac(p.crotch.y);
    let knee_z    = (p.left_knee.z + p.right_knee.z) / 2.0;
    let spread    = m.foot_spread(p.left_ankle.x, p.right_ankle.x);

    if l_bent && r_bent {
        // ── Kneeling: shins going backward into scene, crotch not too high ───
        if (l_shin_back || r_shin_back) && crotch_h < 0.50 {
            // Torso lean forward over knees → "kneeling, torso forward"
            let torso_fwd = p.neck.z - p.crotch.z;
            let vert      = (p.crotch.y - p.neck.y).abs().max(1.0);
            if torso_fwd < -vert * 0.30 {
                return "kneeling, torso leaning forward".into();
            }
            return "kneeling".into();
        }
        // ── Seated variants ──────────────────────────────────────────────────
        if l_shin_down && r_shin_down {
            // Cross-legged: left ankle has crossed to the right of the right ankle.
            if p.left_ankle.x > p.right_ankle.x {
                return "seated cross-legged".into();
            }
            let knees_fwd = p.crotch.z - knee_z > 20.0;
            if knees_fwd || crotch_h > 0.38 {
                // High crotch with feet down = perching on the edge of a seat.
                if crotch_h > 0.52 {
                    return "perched, seated on edge".into();
                }
                return "seated".into();
            }
        }
        // ── Crouch depth ─────────────────────────────────────────────────────
        let depth = if crotch_h < 0.22 { "deep " } else if crotch_h < 0.32 { "" } else { "half " };
        return format!("{depth}squat");
    }

    // ── One knee bent ────────────────────────────────────────────────────────
    if l_bent && !r_bent {
        return if l_shin_back { "kneeling on left knee".into() }
               else { "left knee raised".into() };
    }
    if r_bent && !l_bent {
        return if r_shin_back { "kneeling on right knee".into() }
               else { "right knee raised".into() };
    }

    // ── Standing — check for one foot off the ground ─────────────────────────
    // floor_y = lower (grounded) ankle; the raised ankle will be smaller Y.
    let raise_threshold = m.body_h * 0.08; // at least 8% of body height
    let l_raised = m.above_floor(p.left_ankle.y);
    let r_raised = m.above_floor(p.right_ankle.y);

    if l_raised > raise_threshold && r_raised < raise_threshold / 2.0 {
        let h   = m.foot_raise_desc(p.left_ankle.y);
        let dir = raised_foot_dir(p.crotch.xyz(), p.left_ankle.xyz(), -1.0);
        return format!("balancing on right leg, left foot {h}{dir}");
    }
    if r_raised > raise_threshold && l_raised < raise_threshold / 2.0 {
        let h   = m.foot_raise_desc(p.right_ankle.y);
        let dir = raised_foot_dir(p.crotch.xyz(), p.right_ankle.xyz(), 1.0);
        return format!("balancing on left leg, right foot {h}{dir}");
    }

    // ── Splits: legs very wide AND crotch near the floor ────────────────────
    // Side splits:    ankles spread laterally (X), crotch dropped to floor level.
    // Forward splits: one ankle far forward, one far back (Z), crotch dropped.
    let lat_ratio = (p.left_ankle.x - p.right_ankle.x).abs() / m.shoulder_w;
    let sag_ratio = (p.left_ankle.z - p.right_ankle.z).abs() / m.shoulder_w;
    if crotch_h < 0.32 {
        if lat_ratio >= 1.60 {
            return "doing the side splits".into();
        }
        if sag_ratio >= 1.60 {
            let fwd_leg = if p.left_ankle.z < p.right_ankle.z { "left" } else { "right" };
            return format!("doing the forward splits, {fwd_leg} leg forward");
        }
    }

    // ── Tip-toe: both ankles elevated above their natural resting position ──────
    // Measured as average ankle-above-floor fraction. At normal standing both ankles
    // rest at floor_y (frac ≈ 0). When both heels are lifted the average rises.
    // Only fires when legs are otherwise straight (no bend catch above triggered).
    {
        let l_frac = m.height_frac(p.left_ankle.y);
        let r_frac = m.height_frac(p.right_ankle.y);
        // Both ankles slightly elevated and close to each other → tip-toe
        if l_frac > 0.06 && r_frac > 0.06 && (l_frac - r_frac).abs() < 0.06 {
            return format!("standing on tip-toe, {spread}");
        }
    }

    format!("standing, {spread}")
}

// ─── Torso lean ───────────────────────────────────────────────────────────────

fn torso_lean(p: &Pose) -> Option<String> {
    let lean_x = p.neck.x - p.crotch.x;
    let lean_z = p.neck.z - p.crotch.z;
    let vert   = (p.crotch.y - p.neck.y).abs().max(1.0);

    let fwd_angle  = (lean_z.abs() / vert).atan().to_degrees();
    let side_angle = (lean_x.abs() / vert).atan().to_degrees();

    let fwd = if lean_z < -25.0 && fwd_angle > 12.0 {
        if fwd_angle > 50.0 { Some("leaning far forward") }
        else if fwd_angle > 26.0 { Some("leaning forward") }
        else { Some("leaning slightly forward") }
    } else if lean_z > 25.0 && fwd_angle > 12.0 {
        if fwd_angle > 50.0 { Some("leaning far back") }
        else if fwd_angle > 26.0 { Some("leaning back") }
        else { Some("leaning slightly back") }
    } else { None };

    let side = if side_angle > 10.0 {
        if side_angle > 30.0 {
            if lean_x < 0.0 { Some("tilted far left") } else { Some("tilted far right") }
        } else if side_angle > 18.0 {
            if lean_x < 0.0 { Some("tilted left") } else { Some("tilted right") }
        } else {
            if lean_x < 0.0 { Some("tilted slightly left") } else { Some("tilted slightly right") }
        }
    } else { None };

    // Diagonal lean: when both forward and lateral components are significant,
    // collapse into a single descriptive phrase rather than two independent fragments.
    let base = match (fwd, side) {
        (Some(_f), Some(_s)) => {
            // Classify the combined direction into an 8-point compass word.
            let fwd_dir  = if lean_z < 0.0 { "forward" } else { "back" };
            let side_dir = if lean_x < 0.0 { "left"    } else { "right" };
            let intensity = if fwd_angle > 35.0 || side_angle > 25.0 { "leaning" } else { "leaning slightly" };
            Some(format!("{intensity} {fwd_dir} and to the {side_dir}"))
        },
        (Some(f), None)    => Some(f.into()),
        (None, Some(s))    => Some(s.into()),
        _                  => None,
    };

    // Shoulder tilt: one shoulder noticeably higher than the other.
    // Threshold is proportional to torso height so it stays consistent at any body scale.
    let sh_dy = p.left_shoulder.y - p.right_shoulder.y; // negative = left shoulder higher
    let sh_tilt_threshold = (p.crotch.y - p.neck.y).abs() * 0.11; // ~12 px at default scale=40
    let sh_tilt = if sh_dy < -sh_tilt_threshold * 2.0 { Some("left shoulder sharply raised") }
                  else if sh_dy < -sh_tilt_threshold   { Some("left shoulder raised") }
                  else if sh_dy > sh_tilt_threshold * 2.0 { Some("right shoulder sharply raised") }
                  else if sh_dy > sh_tilt_threshold    { Some("right shoulder raised") }
                  else { None };

    match (base, sh_tilt) {
        (Some(b), Some(t)) => Some(format!("{b}, {t}")),
        (Some(b), None)    => Some(b),
        (None, Some(t))    => Some(t.into()),
        _                  => None,
    }
}

// ─── Torso twist ─────────────────────────────────────────────────────────────
// Detects rotation of the shoulder bar in the XZ plane.
// When square-on to the camera the shoulder vector is purely lateral (dz ≈ 0).
// Z positive = into scene = character's forward, so:
//   dz > 0  → left shoulder closer to viewer, right further → character turned to their RIGHT
//   dz < 0  → right shoulder closer, left further          → character turned to their LEFT
fn torso_twist(p: &Pose) -> Option<String> {
    let dz = p.left_shoulder.z - p.right_shoulder.z;
    let dx = (p.left_shoulder.x - p.right_shoulder.x).abs().max(1.0);
    // Angle between shoulder bar and the pure-lateral axis (0° = square, 90° = profile)
    let twist_deg = dz.abs().atan2(dx).to_degrees();
    if twist_deg < 16.0 { return None; }
    let dir = if dz > 0.0 { "right" } else { "left" };
    Some(if twist_deg > 62.0 {
        format!("in profile, facing {dir}")
    } else if twist_deg > 34.0 {
        format!("body turned {dir}")
    } else {
        format!("body slightly turned {dir}")
    })
}

// ─── Weight shift ─────────────────────────────────────────────────────────────
// Contrapposto / weight on one foot. Only meaningful when both feet are grounded.
// Hip (crotch) offset from the ankle midpoint tells us which leg bears the load.
fn weight_shift(p: &Pose, m: &BodyMetrics, stance_str: &str) -> Option<String> {
    // Contrapposto is only meaningful when upright and both feet are planted.
    // For seated, kneeling, squat etc. the hip offset is irrelevant or misleading.
    if !stance_str.starts_with("standing") { return None; }
    let raise_threshold = m.body_h * 0.08;
    // Skip if either foot is raised — stance() already describes that case.
    if m.above_floor(p.left_ankle.y)  > raise_threshold { return None; }
    if m.above_floor(p.right_ankle.y) > raise_threshold { return None; }
    let ankle_mid_x = (p.left_ankle.x + p.right_ankle.x) / 2.0;
    let hip_offset  = p.crotch.x - ankle_mid_x;
    // Threshold: 22% of shoulder width — subtle but clear contrapposto.
    if hip_offset.abs() < m.shoulder_w * 0.22 { return None; }
    // Magnitude gradation: slight / clear / pronounced contrapposto.
    let side = if hip_offset > 0.0 { "right" } else { "left" };
    let magnitude = if hip_offset.abs() > m.shoulder_w * 0.55 { "strongly " }
                    else if hip_offset.abs() > m.shoulder_w * 0.38 { "" }
                    else { "slightly " };
    Some(format!("{magnitude}weight on {side} foot"))
}


// ─── Head orientation ─────────────────────────────────────────────────────────

fn head_orient(p: &Pose) -> Option<String> {
    let d = norm(sub(p.head.xyz(), p.neck.xyz()));
    let nod_deg = (-d.2).asin().to_degrees(); // + = chin toward viewer (looking down)
    let yaw_deg = d.0.asin().to_degrees();    // + = turned to character's right

    // Head roll: lateral tilt of the head (ear toward shoulder).
    // Approximated by measuring how far the head drifts laterally relative to
    // the neck, normalised against the head-to-neck segment length.
    // Positive = head tilted toward character's right shoulder.
    let neck_to_head_len = mag(sub(p.head.xyz(), p.neck.xyz())).max(1.0);
    let roll_x  = p.head.x - p.neck.x;
    let roll_deg = (roll_x / neck_to_head_len).clamp(-1.0, 1.0).asin().to_degrees();

    let nod = match nod_deg as i32 {
        n if n >  35 => Some("head bowed down"),
        n if n >  15 => Some("looking slightly down"),
        n if n < -35 => Some("head tilted back, looking up"),
        n if n < -15 => Some("looking slightly up"),
        _             => None,
    };
    let yaw = match yaw_deg as i32 {
        y if y >  35 => Some("head turned right"),
        y if y >  15 => Some("glancing right"),
        y if y < -35 => Some("head turned left"),
        y if y < -15 => Some("glancing left"),
        _             => None,
    };
    let roll = match roll_deg as i32 {
        r if r >  20 => Some("head tilted to the right"),
        r if r >  10 => Some("head slightly tilted right"),
        r if r < -20 => Some("head tilted to the left"),
        r if r < -10 => Some("head slightly tilted left"),
        _             => None,
    };

    let base = match (nod, yaw) {
        (Some(n), Some(y)) => Some(format!("{n}, {y}")),
        (Some(n), None)    => Some(n.into()),
        (None, Some(y))    => Some(y.into()),
        _                  => None,
    };

    match (base, roll) {
        (Some(b), Some(r)) => Some(format!("{b}, {r}")),
        (Some(b), None)    => Some(b),
        (None, Some(r))    => Some(r.into()),
        _                  => None,
    }
}

// ─── Arms ─────────────────────────────────────────────────────────────────────

fn arms(p: &Pose, m: &BodyMetrics) -> Option<String> {
    let head: V3 = p.head.xyz();

    // ── Hands clasped / prayer ────────────────────────────────────────────────
    // Both wrists very close together — clasped hands, prayer, pleading, etc.
    {
        let wr_dist = mag(sub(p.left_wrist.xyz(), p.right_wrist.xyz()));
        if wr_dist < m.torso_h * 0.20 {
            let mid_y    = (p.left_wrist.y + p.right_wrist.y) / 2.0;
            let head_dist = mag(sub(p.left_wrist.xyz(), head))
                            .min(mag(sub(p.right_wrist.xyz(), head)));
            if head_dist < m.torso_h * 0.22 {
                return Some("hands pressed together near face".into());
            }
            let pos = if mid_y < m.neck_y + m.torso_h * 0.05 { "raised overhead" }
                      else if mid_y < m.shoulder_y + m.torso_h * 0.25 { "at chest" }
                      else if (mid_y - m.hip_y).abs() < m.torso_h * 0.35 { "at waist" }
                      else { "together" };
            return Some(format!("hands clasped {pos}"));
        }
    }

    // ── Guard / fighting stance ───────────────────────────────────────────────
    // Both arms bent with fists near face/chin level — boxing guard, defensive pose.
    {
        let l_ang = angle_at(p.left_shoulder.xyz(),  p.left_elbow.xyz(),  p.left_wrist.xyz());
        let r_ang = angle_at(p.right_shoulder.xyz(), p.right_elbow.xyz(), p.right_wrist.xyz());
        let l_face = (p.left_wrist.y  - m.neck_y).abs() < m.torso_h * 0.38;
        let r_face = (p.right_wrist.y - m.neck_y).abs() < m.torso_h * 0.38;
        if l_ang < 110.0 && r_ang < 110.0 && l_face && r_face {
            return Some("arms raised in guard position".into());
        }
    }

    // ── Arms folded across chest ──────────────────────────────────────────────
    // Both elbows bent ~90°, each wrist crossing past the body midline to the
    // opposite side. Distinct from "arms crossed" (elbow-only displacement check).
    {
        let l_ang  = angle_at(p.left_shoulder.xyz(),  p.left_elbow.xyz(),  p.left_wrist.xyz());
        let r_ang  = angle_at(p.right_shoulder.xyz(), p.right_elbow.xyz(), p.right_wrist.xyz());
        let mid_x  = (p.left_shoulder.x + p.right_shoulder.x) / 2.0;
        let l_wrist_crossed = p.left_wrist.x  > mid_x + 10.0;
        let r_wrist_crossed = p.right_wrist.x < mid_x - 10.0;
        let chest_band_y = m.shoulder_y + m.torso_h * 0.30;
        let l_at_chest = (p.left_wrist.y  - chest_band_y).abs() < m.torso_h * 0.35;
        let r_at_chest = (p.right_wrist.y - chest_band_y).abs() < m.torso_h * 0.35;
        if l_ang < 110.0 && r_ang < 110.0 && l_wrist_crossed && r_wrist_crossed
           && l_at_chest && r_at_chest {
            return Some("arms folded across chest".into());
        }
    }

    // ── Hand on chest ────────────────────────────────────────────────────────
    // One or both wrists resting near the sternum — expressive or defensive gesture.
    {
        let sternum_x: f32 = (p.left_shoulder.x + p.right_shoulder.x) / 2.0;
        let sternum_y: f32 = m.shoulder_y + m.torso_h * 0.25;
        let sternum_z: f32 = (p.left_shoulder.z + p.right_shoulder.z) / 2.0;
        let sternum: V3    = (sternum_x, sternum_y, sternum_z);
        let thresh         = m.torso_h * 0.28;
        let l_chest = mag(sub(p.left_wrist.xyz(),  sternum)) < thresh;
        let r_chest = mag(sub(p.right_wrist.xyz(), sternum)) < thresh;
        if l_chest && r_chest {
            return Some("both hands on chest".into());
        } else if l_chest {
            return Some("left hand on chest".into());
        } else if r_chest {
            return Some("right hand on chest".into());
        }
    }

    // ── Hands on hips ─────────────────────────────────────────────────────────
    {
        let l_at_hip = (p.left_wrist.y  - m.hip_y).abs() < m.torso_h * 0.30;
        let r_at_hip = (p.right_wrist.y - m.hip_y).abs() < m.torso_h * 0.30;
        let l_out    = p.left_wrist.x  < p.left_shoulder.x  - 15.0;
        let r_out    = p.right_wrist.x > p.right_shoulder.x + 15.0;
        let l_angle  = angle_at(p.left_shoulder.xyz(),  p.left_elbow.xyz(),  p.left_wrist.xyz());
        let r_angle  = angle_at(p.right_shoulder.xyz(), p.right_elbow.xyz(), p.right_wrist.xyz());
        if l_at_hip && r_at_hip && l_out && r_out && l_angle < 120.0 && r_angle < 120.0 {
            return Some("hands on hips".into());
        }
        // ── One hand on hip (akimbo) — fall through to per-arm for the other side
        if l_at_hip && l_out && l_angle < 120.0 && !(r_at_hip && r_out && r_angle < 120.0) {
            // Record left akimbo; right arm will be described individually below.
            // Return early only if right arm is also classifiable as "at side" or similar,
            // otherwise rely on per-arm logic by breaking out.
            let r_desc = describe_arm(p.right_shoulder.xyz(), p.right_elbow.xyz(),
                                      p.right_wrist.xyz(), head, "right", m);
            if let Some(rd) = r_desc {
                return Some(format!("left hand on hip, {rd}"));
            }
        }
        if r_at_hip && r_out && r_angle < 120.0 && !(l_at_hip && l_out && l_angle < 120.0) {
            let l_desc = describe_arm(p.left_shoulder.xyz(), p.left_elbow.xyz(),
                                      p.left_wrist.xyz(), head, "left", m);
            if let Some(ld) = l_desc {
                return Some(format!("right hand on hip, {ld}"));
            }
        }
    }

    // ── Hands on knees ────────────────────────────────────────────────────────
    // Wrists near knee joints — resting/bent-over pose.
    {
        let l_knee_dist = mag(sub(p.left_wrist.xyz(),  p.left_knee.xyz()));
        let r_knee_dist = mag(sub(p.right_wrist.xyz(), p.right_knee.xyz()));
        if l_knee_dist < m.torso_h * 0.28 && r_knee_dist < m.torso_h * 0.28 {
            return Some("hands on knees".into());
        }
    }

    // ── Hand on neck ─────────────────────────────────────────────────────────
    // One wrist near the neck joint — common in surprise, vulnerability, or thinking poses.
    {
        let neck: V3 = p.neck.xyz();
        let l_neck = mag(sub(p.left_wrist.xyz(),  neck)) < m.torso_h * 0.24;
        let r_neck = mag(sub(p.right_wrist.xyz(), neck)) < m.torso_h * 0.24;
        if l_neck && r_neck {
            return Some("both hands at neck".into());
        } else if l_neck {
            let r_desc = describe_arm(p.right_shoulder.xyz(), p.right_elbow.xyz(),
                                      p.right_wrist.xyz(), head, "right", m);
            if let Some(rd) = r_desc { return Some(format!("left hand at neck, {rd}")); }
            return Some("left hand at neck".into());
        } else if r_neck {
            let l_desc = describe_arm(p.left_shoulder.xyz(), p.left_elbow.xyz(),
                                      p.left_wrist.xyz(), head, "left", m);
            if let Some(ld) = l_desc { return Some(format!("right hand at neck, {ld}")); }
            return Some("right hand at neck".into());
        }
    }

    // ── Parade rest / fig-leaf — wrists crossed at pelvis ────────────────────
    // Both wrists near the hip/pelvis level and very close together.
    // Wrist overlap (one in front of the other in X) distinguishes from clasped hands.
    {
        let wr_dist    = mag(sub(p.left_wrist.xyz(), p.right_wrist.xyz()));
        let mid_y      = (p.left_wrist.y + p.right_wrist.y) / 2.0;
        let at_pelvis  = (mid_y - m.hip_y).abs() < m.torso_h * 0.28;
        if at_pelvis && wr_dist < m.torso_h * 0.30 {
            // X-overlap: wrists laterally coincident rather than widely clasped
            let x_sep = (p.left_wrist.x - p.right_wrist.x).abs();
            if x_sep < m.shoulder_w * 0.25 {
                return Some("hands clasped at rest in front".into());
            }
        }
    }

    let left  = describe_arm(p.left_shoulder.xyz(),  p.left_elbow.xyz(),
                             p.left_wrist.xyz(),  head, "left",  m);
    let right = describe_arm(p.right_shoulder.xyz(), p.right_elbow.xyz(),
                             p.right_wrist.xyz(), head, "right", m);

    // Symmetric collapse — only works when both arms produce the same base label.
    // The level qualifiers attached to some labels prevent exact matches when
    // the arms are at different heights, which is the correct behaviour.
    let sym = symmetrize_prefix(&left, &right, &[
        ("left arm at side",                  "right arm at side",                  "arms at sides"),
        ("left arm raised overhead",          "right arm raised overhead",          "arms raised overhead"),
        ("left arm raised",                   "right arm raised",                   "arms raised"),
        ("left arm slightly raised",          "right arm slightly raised",          "arms slightly raised"),
        ("left arm extended forward",         "right arm extended forward",         "arms extended forward"),
        ("left arm extended forward-outward", "right arm extended forward-outward", "arms extended forward-outward"),
        ("left arm reaching forward",         "right arm reaching forward",         "arms reaching forward"),
        ("left arm pointing forward",         "right arm pointing forward",         "arms pointing forward"),
        ("left arm outstretched sideways",    "right arm outstretched sideways",    "arms outstretched sideways"),
        ("left arm crossed",                  "right arm crossed",                  "arms crossed"),
        ("left arm behind back",              "right arm behind back",              "arms behind back"),
        ("left arm slightly behind",          "right arm slightly behind",          "arms slightly behind"),
        ("left arm resting against body",     "right arm resting against body",     "arms resting at sides"),
        // Bent arms: collapse only when both are at the same level (exact match).
        // If levels differ, per-arm description is more informative, so no prefix rule.
    ]);
    if let Some(s) = sym { return Some(s); }

    match (left.as_deref(), right.as_deref()) {
        (None, None)       => None,
        (Some(l), None)    => Some(l.into()),
        (None, Some(r))    => Some(r.into()),
        (Some(l), Some(r)) => Some(format!("{l}, {r}")),
    }
}

/// Classify one arm in a body-relative frame so both sides share identical thresholds.
///
/// **Why body-relative?**
/// Raw world-space X is negative on the left side and positive on the right.
/// Multiplying X components by `sign` (+1 right / −1 left) makes "outward from
/// body" always map to a positive value, fixing the left-arm asymmetry bug.
fn describe_arm(sh: V3, el: V3, wr: V3, head: V3, side: &str, m: &BodyMetrics) -> Option<String> {
    let sign: f32 = if side == "right" { 1.0 } else { -1.0 };

    let sw    = sub(wr, sh);
    let sw_m  = mag(sw);
    if sw_m < 1.0 { return None; }

    // Body-relative unit components of the shoulder→wrist direction:
    //   up  : +1 = wrist straight above shoulder
    //   out : +1 = wrist directly to the side (away from body centre)
    //   fwd : +1 = wrist into scene = character's forward
    let up  = -sw.1 / sw_m;
    let out =  sw.0 * sign / sw_m;
    let fwd =  sw.2 / sw_m; // +1 = into scene = character's forward

    // atan2-based angles for more precise directional classification.
    //   horiz_angle : angle in the horizontal plane measured from forward axis.
    //                 0° = pure forward, 90° = pure outward, ±180° = pure back.
    //   elev_angle  : elevation above/below horizontal.
    //                 +90° = straight up, −90° = straight down, 0° = horizontal.
    let horiz_mag   = (fwd * fwd + out * out).sqrt().max(1e-6);
    let horiz_angle = out.atan2(fwd).to_degrees(); // signed: + = outward sweep
    let elev_angle  = up.atan2(horiz_mag).to_degrees();

    // Elbow outward displacement from shoulder (negative = crossed to opposite side).
    let el_out     = (el.0 - sh.0) * sign;
    let elbow_angle = angle_at(sh, el, wr);

    // ── Overhead — most dramatic, check first ─────────────────────────────────
    if elev_angle > 40.0 {
        // horiz_angle: 0°=fwd, 90°=out, ±180°=back. Use 45° bands for clean blends.
        let dir = if horiz_angle.abs() < 45.0 { " forward" }
                  else if horiz_angle.abs() > 135.0 { " back" }
                  else if out > 0.0 { " to the side" }
                  else { "" };
        let degree = if elev_angle > 65.0 { "overhead" } else { "raised" };
        return Some(format!("{side} arm {degree}{dir}"));
    }
    // Arm lifted partway — not dramatically raised but clearly elevated
    if elev_angle > 10.0 {
        let dir = if fwd > 0.50 { " forward" } else if out > 0.50 { " to the side" } else { "" };
        return Some(format!("{side} arm slightly raised{dir}"));
    }

    // ── Elbow crossed to opposite side of body ────────────────────────────────
    if el_out < -(m.shoulder_w * 0.5) {
        return Some(format!("{side} arm crossed"));
    }

    // ── Pointing — arm fully extended, aimed in a clear direction ────────────
    // elbow_angle > 155° distinguishes a true point from a general extend/reach.
    if elbow_angle > 155.0 {
        if elev_angle > 35.0 {
            let dir = if horiz_angle.abs() < 45.0 { " forward" }
                      else if out > 0.0 { " outward" } else { "" };
            return Some(format!("{side} arm pointing up{dir}"));
        }
        if fwd > 0.55 {
            let level = m.level_name(wr.1);
            return Some(format!("{side} arm pointing forward {level}"));
        }
        if out > 0.55 {
            let level = m.level_name(wr.1);
            return Some(format!("{side} arm pointing sideways {level}"));
        }
        if fwd < -0.45 {
            return Some(format!("{side} arm pointing behind"));
        }
    }

    // ── Forward / behind / sideways — straight-ish arm reaching ──────────────
    // horiz_angle bands: |h| < 55° = forward dominant, |h| > 125° = behind dominant,
    // otherwise lateral. Combined with elev_angle gives cleaner blended directions.
    // Threshold 120° (was 130°) closes the dead zone where bent-arm check also
    // starts at 120°, eliminating silent None returns for arms in that range.
    if fwd > 0.50 && elbow_angle > 120.0 {
        let level = m.level_name(wr.1);
        // Distinguish diagonal-forward from straight-forward using horiz_angle
        let dir = if horiz_angle.abs() < 30.0 { "extended forward" }
                  else { "extended forward-outward" };
        return Some(format!("{side} arm {dir} {level}"));
    }
    // Partial forward reach — arm angled forward but not fully extended
    if fwd > 0.28 && elbow_angle > 120.0 {
        let level = m.level_name(wr.1);
        return Some(format!("{side} arm reaching forward {level}"));
    }
    if fwd < -0.50 && elbow_angle > 120.0 {
        return Some(format!("{side} arm behind back"));
    }
    if fwd < -0.28 && elbow_angle > 120.0 {
        return Some(format!("{side} arm slightly behind"));
    }
    // T-pose / sideways extension — add level for partial raises
    if out > 0.50 && elbow_angle > 120.0 {
        let level = m.level_name(wr.1);
        return Some(format!("{side} arm outstretched sideways {level}"));
    }

    // ── Arm hanging at side ───────────────────────────────────────────────────
    if up < -0.30 && out.abs() < 0.55 && fwd.abs() < 0.55 {
        return Some(format!("{side} arm at side"));
    }

    // ── Wrist resting on/near torso ───────────────────────────────────────────
    // Wrist has drifted inward and is close to the body centre in all axes.
    // This is the "arm tucked against chest" case that otherwise falls to None.
    {
        let torso_centre_x = sh.0 - sh.0 * sign * 0.5; // rough body centreline
        let wrist_in       = out < -0.10;
        let wrist_fwd_near = fwd.abs() < 0.50;
        let wrist_mid_y    = (wr.1 - m.shoulder_y).abs() < m.torso_h * 0.55;
        if wrist_in && wrist_fwd_near && wrist_mid_y {
            let level = m.level_name(wr.1);
            return Some(format!("{side} arm resting against body {level}"));
        }
        let _ = torso_centre_x; // suppress unused warning
        let _ = (horiz_angle, elev_angle); // used above; suppress if only partially consumed
    }

    // ── Bent elbow — where is the hand, and where does the elbow point? ────────
    // Threshold raised to 130° so it overlaps with the straight-arm checks above,
    // ensuring no angle falls silently through both conditions.
    if elbow_angle < 130.0 {
        // ── Hand near head — sub-region breakdown ─────────────────────────────
        // Order matters: most specific checks first.
        let dist_to_head = mag(sub(wr, head));
        if dist_to_head < m.torso_h * 0.22 {
            // Determine which part of the head the hand is near using Y and Z offsets.
            let wr_above_head = wr.1 < head.1 - m.torso_h * 0.08; // wrist above head centre
            let wr_at_chin    = wr.1 > head.1 + m.torso_h * 0.06; // wrist below head centre (chin)
            let wr_fwd_of_head = wr.2 < head.2 - 10.0;            // wrist toward viewer = covering face
            return Some(if wr_above_head {
                format!("{side} hand on top of head")
            } else if wr_at_chin {
                format!("{side} hand at chin")
            } else if wr_fwd_of_head {
                format!("{side} hand covering face")
            } else {
                format!("{side} hand near face")
            });
        }
        if wr.2 < head.2 - 18.0 && (wr.1 - head.1).abs() < m.torso_h * 0.22 {
            return Some(format!("{side} hand behind head"));
        }

        let level = m.level_name(wr.1);
        let se    = sub(el, sh);
        let se_m  = mag(se).max(1e-6);
        let el_up  = -se.1 / se_m;
        let el_fwd =  se.2 / se_m;
        let el_out =  se.0 * sign / se_m;

        // Degree of bend adds nuance — sharply bent reads very differently to slightly bent
        let bend_deg = if elbow_angle < 75.0 { "sharply bent" }
                       else if elbow_angle < 100.0 { "bent" }
                       else { "slightly bent" };

        let elbow_dir = if el_up > 0.55  { " elbow up" }
                        else if el_fwd > 0.55 { " elbow forward" }
                        else if el_fwd < -0.55 { " elbow back" }
                        else if el_out > 0.50 { " elbow out" }
                        else if el_out < -0.30 { " elbow in" }
                        else { "" };

        // Wrist at belly / lower abdomen — thinking or resting pose
        let belly_y = m.hip_y - m.torso_h * 0.15;
        if (wr.1 - belly_y).abs() < m.torso_h * 0.20 && out.abs() < 0.45 {
            return Some(format!("{side} arm {bend_deg}{elbow_dir}, hand at abdomen"));
        }

        return Some(format!("{side} arm {bend_deg}{elbow_dir}, hand {level}"));
    }

    None
}

// ─── Legs ─────────────────────────────────────────────────────────────────────

fn legs(p: &Pose, m: &BodyMetrics, stance_str: &str) -> Option<String> {
    // ── Early exit: stance already owns the lower-body description ────────────
    // These postures are fully characterised by stance(); appending per-leg detail
    // would be redundant or directly contradict the primary description.
    // "standing" and "standing, feet …" are the only cases where legs() adds value.
    if stance_str.starts_with("lying")
        || stance_str.starts_with("balancing")
        || stance_str.starts_with("seated")
        || stance_str.starts_with("perched")
        || stance_str.contains("squat")
        || stance_str.contains("kneeling")
        || stance_str.contains("knee raised")
        || stance_str.contains("splits")
        || stance_str.contains("tip-toe")
    {
        return None;
    }

    // ── Lateral spread: overrides per-leg descriptions ────────────────────────
    // Use the same ratio thresholds as foot_spread() so legs() and stance() can
    // never disagree about how wide the feet are.
    //   ratio < 0.90  → hip-width or closer: no override, let per-leg logic run
    //   ratio 0.90–1.60 → "feet wide apart" territory → "wide"
    //   ratio ≥ 1.60  → "feet very wide apart" territory → "very wide"
    let spread_ratio = (p.left_ankle.x - p.right_ankle.x).abs() / m.shoulder_w;
    if spread_ratio >= 0.90 {
        let width = if spread_ratio >= 1.60 { "very wide" } else { "wide" };
        // Still describe stride within a wide stance
        let l = describe_leg(p.crotch.xyz(), p.left_knee.xyz(),  p.left_ankle.xyz(),  "left",  m);
        let r = describe_leg(p.crotch.xyz(), p.right_knee.xyz(), p.right_ankle.xyz(), "right", m);
        let stride = symmetrize(&l, &r, &[
            ("left leg forward", "right leg back",    "legs in stride"),
            ("left leg back",    "right leg forward", "legs in stride"),
        ]);
        return Some(match stride {
            Some(s) => format!("legs spread {width}, {s}"),
            None    => format!("legs spread {width}"),
        });
    }

    let left  = describe_leg(p.crotch.xyz(), p.left_knee.xyz(),  p.left_ankle.xyz(),  "left",  m);
    let right = describe_leg(p.crotch.xyz(), p.right_knee.xyz(), p.right_ankle.xyz(), "right", m);

    // ── Crossed ankles (standing rest pose) ───────────────────────────────────
    // Left ankle has drifted right of the right ankle — ankles crossed.
    // Only meaningful when both legs are mostly straight (not a lunge/step already described).
    {
        let ankles_crossed = p.left_ankle.x > p.right_ankle.x + 8.0;
        let l_straight = left.as_deref().map_or(false,  |s| s.contains("straight") || s.contains("slightly bent"));
        let r_straight = right.as_deref().map_or(false, |s| s.contains("straight") || s.contains("slightly bent"));
        if ankles_crossed && l_straight && r_straight {
            return Some("ankles crossed".into());
        }
    }

    // ── Symmetric leg pairs ───────────────────────────────────────────────────
    // Only exact-string pairs collapse; "raised to hip height" etc. won't match
    // unless both legs are at the exact same level, which is usually fine.
    let sym = symmetrize(&left, &right, &[
        ("left leg forward",               "right leg back",             "legs in stride"),
        ("left leg back",                  "right leg forward",          "legs in stride"),
        ("left leg forward",               "right leg forward",          "both legs forward"),
        ("left leg forward-outward",       "right leg back",             "legs in diagonal stride"),
        ("left leg back",                  "right leg forward-outward",  "legs in diagonal stride"),
        ("left leg bent",                  "right leg bent",             "both legs bent"),
        ("left leg slightly bent",         "right leg slightly bent",    "legs slightly bent"),
        ("left leg deeply bent",           "right leg deeply bent",      "legs deeply bent"),
        ("left leg straight",              "right leg straight",         "legs straight"),
        ("left leg out to the side",       "right leg out to the side",  "legs out to the sides"),
        ("left leg forward bent",          "right leg stepping back",    "legs in stride, lead knee bent"),
        ("left leg stepping forward",      "right leg back",             "legs in stride"),
        ("left leg back",                  "right leg stepping forward", "legs in stride"),
    ]);
    if let Some(s) = sym { return Some(s); }

    // ── "Legs together" only when feet are actually close ─────────────────────
    // Collapsing two "straight" legs into "legs together" is only semantically
    // correct when the ankles are genuinely close.  If the feet are spread the
    // stance() description already covers that; emitting "legs together" here
    // would directly contradict it (e.g. "feet very wide apart, legs together").
    if left.as_deref() == Some("left leg straight") && right.as_deref() == Some("right leg straight") {
        let spread_ratio = (p.left_ankle.x - p.right_ankle.x).abs() / m.shoulder_w;
        if spread_ratio < 0.40 {
            return Some("legs together".into());
        }
        // Feet are spread but legs are otherwise straight — stance() already
        // describes the spread, so no additional leg phrase is needed.
        return None;
    }

    // ── Lunge: forward+bent lead leg, back trailing leg ──────────────────────
    // Uses starts_with so knee-dir suffixes (" knee out" etc.) don't block the match.
    let l = left.as_deref().unwrap_or("");
    let r = right.as_deref().unwrap_or("");
    // Match forward-outward (diagonal combat/dance lunge) as well as straight forward.
    let l_fwd_bent = l.starts_with("left leg forward bent")
        || l.starts_with("left leg forward deeply bent")
        || l.starts_with("left leg forward-outward bent")
        || l.starts_with("left leg forward-outward deeply bent")
        || l.starts_with("left leg stepping forward bent");
    let r_fwd_bent = r.starts_with("right leg forward bent")
        || r.starts_with("right leg forward deeply bent")
        || r.starts_with("right leg forward-outward bent")
        || r.starts_with("right leg forward-outward deeply bent")
        || r.starts_with("right leg stepping forward bent");
    // "trailing" = the opposite leg is back; check the *other* leg for the back pattern.
    let right_is_back = r.starts_with("right leg back") || r.starts_with("right leg stepping back");
    let left_is_back  = l.starts_with("left leg back")  || l.starts_with("left leg stepping back");
    if l_fwd_bent && right_is_back { return Some("lunge, left leg leading".into()); }
    if r_fwd_bent && left_is_back  { return Some("lunge, right leg leading".into()); }

    match (left.as_deref(), right.as_deref()) {
        (None, None)       => None,
        (Some(l), None)    => Some(l.into()),
        (None, Some(r))    => Some(r.into()),
        (Some(l), Some(r)) => Some(format!("{l}, {r}")),
    }
}

/// Classify one leg using hip→ankle and hip→knee vectors in a body-relative frame.
///
/// Body-relative frame (sign flipped for left side so "outward" is always +):
///   up  : +1 = ankle above hip
///   fwd : +1 = ankle into scene = character's forward
///   lat : +1 = ankle away from body centre (outward)
///
/// Knee deviation is measured perpendicular to the hip→ankle line:
///   knee_dev > 0 = knee bowed outward (varus)
///   knee_dev < 0 = knee caved inward  (valgus)
fn describe_leg(hip: V3, kn: V3, an: V3, side: &str, m: &BodyMetrics) -> Option<String> {
    let sign: f32 = if side == "right" { 1.0 } else { -1.0 };

    let ha   = sub(an, hip);
    let ha_m = mag(ha);
    if ha_m < 1.0 { return None; }

    let up  = -ha.1 / ha_m;           // +1 = ankle above hip
    let fwd =  ha.2 / ha_m;           // +1 = into scene = character's forward
    let lat =  ha.0 * sign / ha_m;    // +1 = ankle outward (away from centre)
    let bend = angle_at(hip, kn, an); // angle at the knee; 180 = straight

    // atan2-based angles for precise directional labeling:
    //   h_angle: angle in horizontal plane from forward axis (+° = outward sweep).
    //   elev   : elevation of ankle relative to hip (+° = raised, −° = lowered).
    let h_mag   = (fwd * fwd + lat * lat).sqrt().max(1e-6);
    let h_angle = lat.atan2(fwd).to_degrees(); // 0°=fwd, 90°=outward, 180°=back
    let elev    = up.atan2(h_mag).to_degrees();

    // ── Knee lateral deviation from the hip→ankle centreline ─────────────────
    // Interpolate the hip→ankle line at the knee's Y to find the "neutral" X.
    let t = if (an.1 - hip.1).abs() > 1.0 { (kn.1 - hip.1) / (an.1 - hip.1) } else { 0.5 };
    let line_x    = hip.0 + t * (an.0 - hip.0);
    let knee_dev  = (kn.0 - line_x) * sign; // + = outward (varus), − = inward (valgus)
    let knee_dir  = if knee_dev > 18.0 { " knee out" }
                    else if knee_dev < -18.0 { " knee in" }
                    else { "" };

    // ── Shin direction (ankle relative to knee in Z) ──────────────────────────
    // Useful for distinguishing a deep squat (shin vertical) from a lunge (shin forward).
    let shin_fwd  = an.2 - kn.2 > 20.0; // ankle further into scene than knee → shin forward
    let shin_back = kn.2 - an.2 > 20.0; // ankle closer to viewer than knee → shin angled back

    // ── Ankle clearly above hip (leg raised / kicked) ─────────────────────────
    if elev > 17.0 {  // atan2: ~17° corresponds to up ≈ 0.30 of ha_m
        let h   = m.foot_raise_desc(an.1);
        // Use h_angle bands for cleaner directional blends
        let dir = if h_angle.abs() < 55.0   { " forward" }
                  else if h_angle.abs() > 125.0 { " behind" }
                  else if lat > 0.0         { " to the side" }
                  else                      { "" };
        return Some(format!("{side} leg {h}{dir}"));
    }

    // ── Lateral swing (leg kicked / planted out to the side) ─────────────────
    if h_angle > 55.0 && h_angle < 125.0 {   // clearly lateral, forward bias < 55°
        let bent_sfx = if bend < 130.0 { ", bent" } else { "" };
        return Some(format!("{side} leg out to the side{bent_sfx}"));
    }

    // ── Forward step ──────────────────────────────────────────────────────────
    if fwd > 0.55 {
        let bent_sfx = if bend < 100.0 { " deeply bent" } else if bend < 130.0 { " bent" }
                       else if bend < 155.0 { " slightly bent" } else { " straight" };
        // Diagonal forward-outward is a common combat or dance stance worth naming
        let dir = if h_angle > 30.0 && h_angle < 80.0 { " forward-outward" } else { " forward" };
        return Some(format!("{side} leg{dir}{bent_sfx}{knee_dir}"));
    }
    if fwd > 0.35 {
        let bent_sfx = if bend < 130.0 { " bent" } else if bend < 155.0 { " slightly bent" } else { "" };
        return Some(format!("{side} leg stepping forward{bent_sfx}{knee_dir}"));
    }

    // ── Back step ─────────────────────────────────────────────────────────────
    if fwd < -0.55 {
        let bent_sfx = if bend < 100.0 { " deeply bent" } else if bend < 130.0 { " bent" }
                       else if bend < 155.0 { " slightly bent" } else { " straight" };
        return Some(format!("{side} leg back{bent_sfx}{knee_dir}"));
    }
    if fwd < -0.35 {
        let bent_sfx = if bend < 130.0 { " bent" } else if bend < 155.0 { " slightly bent" } else { "" };
        return Some(format!("{side} leg stepping back{bent_sfx}{knee_dir}"));
    }

    // ── Bent without notable stride — shin direction is the key detail ────────
    if bend < 100.0 {
        let shin_sfx = if shin_fwd { ", shin angled forward" }
                       else if shin_back { ", shin angled back" } else { "" };
        return Some(format!("{side} leg deeply bent{knee_dir}{shin_sfx}"));
    }
    if bend < 130.0 {
        let shin_sfx = if shin_fwd { ", shin angled forward" }
                       else if shin_back { ", shin angled back" } else { "" };
        return Some(format!("{side} leg bent{knee_dir}{shin_sfx}"));
    }
    if bend < 155.0 {
        return Some(format!("{side} leg slightly bent{knee_dir}"));
    }

    // ── Fully straight ────────────────────────────────────────────────────────
    // `else` rather than `if bend > 155.0` to close the float gap at exactly 155.0,
    // which would otherwise fall silently through to None.
    let _ = (h_angle, elev); // used above; suppress if residual paths don't reach them
    Some(format!("{side} leg straight{knee_dir}"))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Exact-match symmetrize: both strings must match the pair precisely.
fn symmetrize(left: &Option<String>, right: &Option<String>,
              pairs: &[(&str, &str, &str)]) -> Option<String> {
    let l = left.as_deref().unwrap_or("");
    let r = right.as_deref().unwrap_or("");
    for &(lp, rp, combined) in pairs {
        if l == lp && r == rp { return Some(combined.into()); }
    }
    None
}

/// Prefix-match symmetrize: checks whether each string STARTS WITH the given
/// prefix. Used for arm descriptions that may have a level suffix appended
/// (e.g. "left arm extended forward at chest level"). When both arms start with
/// matching prefixes, collapses to the combined form.
fn symmetrize_prefix(left: &Option<String>, right: &Option<String>,
                     pairs: &[(&str, &str, &str)]) -> Option<String> {
    let l = left.as_deref().unwrap_or("");
    let r = right.as_deref().unwrap_or("");
    for &(lp, rp, combined) in pairs {
        if l.starts_with(lp) && r.starts_with(rp) {
            // If both have an identical suffix (e.g. same level), append it.
            let l_suffix = l[lp.len()..].trim();
            let r_suffix = r[rp.len()..].trim();
            if l_suffix == r_suffix && !l_suffix.is_empty() {
                return Some(format!("{combined} {l_suffix}"));
            }
            return Some(combined.into());
        }
    }
    None
}
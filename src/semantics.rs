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
    parts.push(stance(pose, &m));
    if let Some(s) = torso_lean(pose)     { parts.push(s); }
    if let Some(s) = torso_twist(pose)    { parts.push(s); }
    if let Some(s) = weight_shift(pose, &m) { parts.push(s); }
    if let Some(s) = head_orient(pose)    { parts.push(s); }
    if let Some(s) = arms(pose, &m)       { parts.push(s); }
    if let Some(s) = legs(pose, &m)       { parts.push(s); }
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

fn stance(p: &Pose, m: &BodyMetrics) -> String {
    // Lying: body nearly horizontal — head and ankles at very similar Y.
    if m.body_h < 80.0 {
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
        let h = m.foot_raise_desc(p.left_ankle.y);
        return format!("balancing on right leg, left foot {h}");
    }
    if r_raised > raise_threshold && l_raised < raise_threshold / 2.0 {
        let h = m.foot_raise_desc(p.right_ankle.y);
        return format!("balancing on left leg, right foot {h}");
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

    // Shoulder tilt: one shoulder noticeably higher than the other.
    let sh_dy = p.left_shoulder.y - p.right_shoulder.y; // negative = left shoulder higher
    let sh_tilt = if sh_dy < -12.0 { Some("left shoulder raised") }
                  else if sh_dy > 12.0 { Some("right shoulder raised") }
                  else { None };

    let base = match (fwd, side) {
        (Some(f), Some(s)) => Some(format!("{f}, {s}")),
        (Some(f), None)    => Some(f.into()),
        (None, Some(s))    => Some(s.into()),
        _                  => None,
    };

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
fn weight_shift(p: &Pose, m: &BodyMetrics) -> Option<String> {
    let raise_threshold = m.body_h * 0.08;
    // Skip if either foot is raised — stance() already describes that case.
    if m.above_floor(p.left_ankle.y)  > raise_threshold { return None; }
    if m.above_floor(p.right_ankle.y) > raise_threshold { return None; }
    let ankle_mid_x = (p.left_ankle.x + p.right_ankle.x) / 2.0;
    let hip_offset  = p.crotch.x - ankle_mid_x;
    // Threshold: 22% of shoulder width — subtle but clear contrapposto.
    if hip_offset.abs() < m.shoulder_w * 0.22 { return None; }
    Some(if hip_offset > 0.0 { "weight on right foot".into() } else { "weight on left foot".into() })
}



fn head_orient(p: &Pose) -> Option<String> {
    let d = norm(sub(p.head.xyz(), p.neck.xyz()));
    let nod_deg = (-d.2).asin().to_degrees(); // + = chin toward viewer (looking down)
    let yaw_deg = d.0.asin().to_degrees();    // + = turned to character's right

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

    match (nod, yaw) {
        (Some(n), Some(y)) => Some(format!("{n}, {y}")),
        (Some(n), None)    => Some(n.into()),
        (None, Some(y))    => Some(y.into()),
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

    let left  = describe_arm(p.left_shoulder.xyz(),  p.left_elbow.xyz(),
                             p.left_wrist.xyz(),  head, "left",  m);
    let right = describe_arm(p.right_shoulder.xyz(), p.right_elbow.xyz(),
                             p.right_wrist.xyz(), head, "right", m);

    // Symmetric collapse — only works when both arms produce the same base label.
    // The level qualifiers attached to some labels prevent exact matches when
    // the arms are at different heights, which is the correct behaviour.
    let sym = symmetrize_prefix(&left, &right, &[
        ("left arm at side",               "right arm at side",               "arms at sides"),
        ("left arm raised overhead",       "right arm raised overhead",       "arms raised overhead"),
        ("left arm raised",                "right arm raised",                "arms raised"),
        ("left arm extended forward",      "right arm extended forward",      "arms extended forward"),
        ("left arm outstretched sideways", "right arm outstretched sideways", "arms outstretched sideways"),
        ("left arm crossed",               "right arm crossed",               "arms crossed"),
        ("left arm behind back",           "right arm behind back",           "arms behind back"),
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

    // Elbow outward displacement from shoulder (negative = crossed to opposite side).
    let el_out     = (el.0 - sh.0) * sign;
    let elbow_angle = angle_at(sh, el, wr);

    // ── Overhead — most dramatic, check first ─────────────────────────────────
    if up > 0.65 {
        let dir = if fwd > 0.40 { " forward" } else if fwd < -0.40 { " back" }
                  else if out > 0.40 { " to the side" } else { "" };
        return Some(format!("{side} arm raised overhead{dir}"));
    }
    if up > 0.30 {
        let dir = if fwd > 0.45 { " forward" } else if fwd < -0.45 { " back" }
                  else if out > 0.45 { " to the side" } else { "" };
        return Some(format!("{side} arm raised{dir}"));
    }
    // Arm lifted partway — not dramatically raised but clearly elevated
    if up > 0.12 {
        let dir = if fwd > 0.50 { " forward" } else if out > 0.50 { " to the side" } else { "" };
        return Some(format!("{side} arm slightly raised{dir}"));
    }

    // ── Elbow crossed to opposite side of body ────────────────────────────────
    if el_out < -(m.shoulder_w * 0.5) {
        return Some(format!("{side} arm crossed"));
    }

    // ── Forward / behind / sideways — straight-ish arm reaching ──────────────
    // Threshold 120° (was 130°) closes the dead zone where bent-arm check also
    // starts at 120°, eliminating silent None returns for arms in that range.
    if fwd > 0.50 && elbow_angle > 120.0 {
        let level = m.level_name(wr.1);
        return Some(format!("{side} arm extended forward {level}"));
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
    }

    // ── Bent elbow — where is the hand, and where does the elbow point? ────────
    // Threshold raised to 130° so it overlaps with the straight-arm checks above,
    // ensuring no angle falls silently through both conditions.
    if elbow_angle < 130.0 {
        // Hand proximity overrides level — far more descriptive for AI prompts
        let dist_to_head = mag(sub(wr, head));
        if dist_to_head < m.torso_h * 0.22 {
            return Some(format!("{side} hand near face"));
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
        return Some(format!("{side} arm {bend_deg}{elbow_dir}, hand {level}"));
    }

    None
}

// ─── Legs ─────────────────────────────────────────────────────────────────────

fn legs(p: &Pose, m: &BodyMetrics) -> Option<String> {
    // ── Lateral spread: overrides per-leg descriptions ────────────────────────
    let spread_x    = (p.left_ankle.x - p.right_ankle.x).abs();
    let spread_knee = (p.left_knee.x  - p.right_knee.x).abs();
    if spread_x > 160.0 || spread_knee > 140.0 {
        let width = if spread_x > 240.0 { "very wide" } else { "wide" };
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

    // ── Symmetric leg pairs ───────────────────────────────────────────────────
    // Only exact-string pairs collapse; "raised to hip height" etc. won't match
    // unless both legs are at the exact same level, which is usually fine.
    let sym = symmetrize(&left, &right, &[
        ("left leg forward",  "right leg back",     "legs in stride"),
        ("left leg back",     "right leg forward",  "legs in stride"),
        ("left leg forward",  "right leg forward",  "both legs forward"),
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
    if (l.starts_with("left leg forward bent") || l.starts_with("left leg forward deeply bent")
        || l.starts_with("left leg stepping forward bent"))
       && (r.starts_with("right leg back") || r.starts_with("right leg stepping back")) {
        return Some("lunge, left leg leading".into());
    }
    if (r.starts_with("right leg forward bent") || r.starts_with("right leg forward deeply bent")
        || r.starts_with("right leg stepping forward bent"))
       && (l.starts_with("left leg back") || l.starts_with("left leg stepping back")) {
        return Some("lunge, right leg leading".into());
    }

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
    if up > 0.30 {
        let h   = m.foot_raise_desc(an.1);
        let dir = if fwd > 0.35 { " forward" }
                  else if fwd < -0.35 { " behind" }
                  else if lat > 0.35 { " to the side" }
                  else { "" };
        return Some(format!("{side} leg {h}{dir}"));
    }

    // ── Lateral swing (leg kicked / planted out to the side) ─────────────────
    if lat > 0.45 {
        let bent_sfx = if bend < 130.0 { ", bent" } else { "" };
        return Some(format!("{side} leg out to the side{bent_sfx}"));
    }

    // ── Forward step ──────────────────────────────────────────────────────────
    if fwd > 0.55 {
        let bent_sfx = if bend < 100.0 { " deeply bent" } else if bend < 130.0 { " bent" }
                       else if bend < 155.0 { " slightly bent" } else { " straight" };
        return Some(format!("{side} leg forward{bent_sfx}{knee_dir}"));
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
    if bend > 155.0 {
        return Some(format!("{side} leg straight{knee_dir}"));
    }

    None
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
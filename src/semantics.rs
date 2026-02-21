// semantics.rs
// Converts a manually-posed Pose into concise prompt text for AI image/video generators.
// Aims for 1-2 punchy lines max — enough for the renderer to understand body position
// without turning into a novel.
//
// Coordinate space (stored in Pose after scale=40 applied):
//   X: negative = char's left,  positive = char's right
//   Y: SMALLER = higher on body (screen coords, Y increases downward)
//   Z: negative = toward viewer, positive = into scene
//
// Rough unit guide at scale=40:  shoulder_width≈64  thigh≈100  torso≈108

use crate::pose::Pose;

pub fn describe(pose: &Pose) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push(stance(pose));

    if let Some(s) = torso_lean(pose)  { parts.push(s); }
    if let Some(s) = head_orient(pose) { parts.push(s); }
    if let Some(s) = arms(pose)        { parts.push(s); }
    if let Some(s) = legs(pose)        { parts.push(s); }

    parts.join(", ")
}

// ── Stance ────────────────────────────────────────────────────────────────────

fn stance(p: &Pose) -> String {
    // Y-span: difference between highest joint (smallest Y) and lowest (largest Y)
    let all_y = [p.head.y, p.neck.y, p.waist.y, p.crotch.y,
                 p.left_knee.y, p.right_knee.y, p.left_ankle.y, p.right_ankle.y];
    let y_min = all_y.iter().cloned().fold(f32::MAX, f32::min);
    let y_max = all_y.iter().cloned().fold(f32::MIN, f32::max);
    let y_span = y_max - y_min;

    // Head vs ankle height — if close, figure is horizontal
    let head_ankle_diff = (p.left_ankle.y + p.right_ankle.y) / 2.0 - p.head.y;

    if y_span < 80.0 || head_ankle_diff.abs() < 60.0 {
        // Flat — determine face-up or face-down by Z of head vs pelvis
        let face_dir = if p.head.z < p.crotch.z { "face-up" } else { "face-down" };
        return format!("lying {face_dir}");
    }

    // Knees — bent forward into scene (large Z) = seated tendency
    let knee_z_avg  = (p.left_knee.z + p.right_knee.z) / 2.0;
    let hip_z       = p.crotch.z;
    let knees_fwd   = knee_z_avg - hip_z > 40.0;

    // How high are the knees relative to the hips
    let knee_y_avg  = (p.left_knee.y + p.right_knee.y) / 2.0;
    let knees_up    = p.crotch.y - knee_y_avg; // positive = knees higher than hips

    // Ankles relative to knees — if ankles are near knee height, legs are bent
    let ankle_y_avg = (p.left_ankle.y + p.right_ankle.y) / 2.0;
    let ankles_tucked = (ankle_y_avg - knee_y_avg).abs() < 50.0;

    if knees_fwd && ankles_tucked {
        return "seated".into();
    }
    if knees_up > 60.0 {
        return "crouching".into();
    }
    // One knee up = kneeling
    let lk_up = p.crotch.y - p.left_knee.y;
    let rk_up = p.crotch.y - p.right_knee.y;
    if lk_up > 60.0 && rk_up < 20.0 { return "kneeling on right knee".into(); }
    if rk_up > 60.0 && lk_up < 20.0 { return "kneeling on left knee".into(); }
    if lk_up > 50.0 && rk_up > 50.0 { return "kneeling".into(); }

    "standing".into()
}

// ── Torso lean ────────────────────────────────────────────────────────────────

fn torso_lean(p: &Pose) -> Option<String> {
    // Horizontal offset of neck vs crotch
    let lean_x = p.neck.x - p.crotch.x;
    let lean_z = p.neck.z - p.crotch.z;
    let lean_y = p.crotch.y - p.neck.y; // positive = neck above crotch (normal)

    // Expressed as angle from vertical
    let fwd_angle  = (lean_z.abs() / lean_y.abs().max(1.0)).atan().to_degrees();
    let side_angle = (lean_x.abs() / lean_y.abs().max(1.0)).atan().to_degrees();

    let fwd = if lean_z < -30.0 && fwd_angle > 20.0 {
        if fwd_angle > 50.0 { Some("leaning far forward") } else { Some("leaning forward") }
    } else if lean_z > 30.0 && fwd_angle > 20.0 {
        if fwd_angle > 50.0 { Some("leaning far back") } else { Some("leaning back") }
    } else { None };

    let side = if side_angle > 15.0 {
        if lean_x < 0.0 { Some("tilted left") } else { Some("tilted right") }
    } else { None };

    match (fwd, side) {
        (Some(f), Some(s)) => Some(format!("{f}, {s}")),
        (Some(f), None)    => Some(f.into()),
        (None, Some(s))    => Some(s.into()),
        _                  => None,
    }
}

// ── Head orientation ──────────────────────────────────────────────────────────

fn head_orient(p: &Pose) -> Option<String> {
    // neck→head vector
    let dx = p.head.x - p.neck.x;
    let dy = p.head.y - p.neck.y; // negative = head above neck (normal)
    let dz = p.head.z - p.neck.z;
    let len = (dx*dx + dy*dy + dz*dz).sqrt();
    if len < 1.0 { return None; }

    let nod_deg = (-dz / len).asin().to_degrees();   // + = chin down/forward
    let yaw_deg = ( dx / len).asin().to_degrees();   // + = turned right

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

// ── Arms ──────────────────────────────────────────────────────────────────────

fn arms(p: &Pose) -> Option<String> {
    let left  = describe_arm(p.left_shoulder.xyz(),  p.left_elbow.xyz(),  p.left_wrist.xyz(),  "left");
    let right = describe_arm(p.right_shoulder.xyz(), p.right_elbow.xyz(), p.right_wrist.xyz(), "right");

    // Collapse symmetric cases to a single phrase
    let symmetric = symmetrize(&left, &right, &[
        ("left arm at side",       "right arm at side",       "arms at sides"),
        ("left arm raised",        "right arm raised",        "both arms raised"),
        ("left arm raised overhead","right arm raised overhead","arms raised overhead"),
        ("left arm extended",      "right arm extended",      "arms extended"),
        ("left arm bent",          "right arm bent",          "arms bent"),
        ("left arm crossed",       "right arm crossed",       "arms crossed"),
    ]);

    if let Some(s) = symmetric { return Some(s); }

    match (left.as_deref(), right.as_deref()) {
        (None, None)       => None,
        (Some(l), None)    => Some(l.into()),
        (None, Some(r))    => Some(r.into()),
        (Some(l), Some(r)) => Some(format!("{l}, {r}")),
    }
}

fn describe_arm(sh: (f32,f32,f32), el: (f32,f32,f32), wr: (f32,f32,f32), side: &str) -> Option<String> {
    // Y: smaller = higher.  sh.y is shoulder height.
    let wrist_above_shoulder = wr.1 < sh.1 - 30.0;  // wrist much higher than shoulder
    let elbow_above_shoulder = el.1 < sh.1 - 20.0;
    let wrist_near_shoulder_y = (wr.1 - sh.1).abs() < 40.0;

    // How far laterally is the wrist from body centre (X=0)
    let wrist_out = wr.0.abs();
    // Elbow crossing body centre (toward opposite side)
    let elbow_crossed = if side == "left" { el.0 > 20.0 } else { el.0 < -20.0 };

    // Arm straight: elbow roughly on line from shoulder to wrist
    let bend_angle = joint_angle(sh, el, wr);

    if wrist_above_shoulder && elbow_above_shoulder {
        return Some(format!("{side} arm raised overhead"));
    }
    if elbow_above_shoulder && !wrist_above_shoulder {
        return Some(format!("{side} arm raised"));
    }
    if elbow_crossed {
        return Some(format!("{side} arm crossed"));
    }
    // Wrist near hip/waist level and elbow not out — arm down
    if wr.1 > sh.1 + 40.0 && wrist_out < 50.0 {
        return Some(format!("{side} arm at side"));
    }
    // Extended forward/back
    let arm_fwd = sh.2 - wr.2; // positive = wrist in front of shoulder
    if arm_fwd > 60.0 && bend_angle > 140.0 {
        return Some(format!("{side} arm extended forward"));
    }
    if arm_fwd < -60.0 && bend_angle > 140.0 {
        return Some(format!("{side} arm extended back"));
    }
    // Elbow-level wrist, arm out to side
    if wrist_near_shoulder_y && wrist_out > 80.0 {
        return Some(format!("{side} arm extended sideways"));
    }
    if bend_angle < 100.0 {
        return Some(format!("{side} arm bent"));
    }

    None // arm is roughly neutral/at side — not worth mentioning
}

// ── Legs ──────────────────────────────────────────────────────────────────────

fn legs(p: &Pose) -> Option<String> {
    let left  = describe_leg(p.crotch.xyz(), p.left_knee.xyz(),  p.left_ankle.xyz(),  "left");
    let right = describe_leg(p.crotch.xyz(), p.right_knee.xyz(), p.right_ankle.xyz(), "right");

    // Leg spread — most useful single descriptor
    let spread_x = (p.left_ankle.x - p.right_ankle.x).abs();
    let spread_knee = (p.left_knee.x - p.right_knee.x).abs();

    if spread_x > 160.0 || spread_knee > 140.0 {
        let width = if spread_x > 240.0 { "very wide" } else { "wide" };
        // Check if also kicking forward vs just spread
        let symmetric = symmetrize(&left, &right, &[
            ("left leg forward", "right leg back", "legs in stride"),
            ("left leg back",    "right leg forward", "legs in stride"),
        ]);
        if let Some(s) = symmetric { return Some(format!("legs spread {width}, {s}")); }
        return Some(format!("legs spread {width}"));
    }

    let symmetric = symmetrize(&left, &right, &[
        ("left leg forward",    "right leg back",    "legs in stride"),
        ("left leg back",       "right leg forward", "legs in stride"),
        ("left leg forward",    "right leg forward", "both legs forward"),
        ("left knee raised",    "right knee raised", "both knees raised"),
        ("left leg straight",   "right leg straight","legs together"),
    ]);

    if let Some(s) = symmetric { return Some(s); }

    match (left.as_deref(), right.as_deref()) {
        (None, None)       => None,
        (Some(l), None)    => Some(l.into()),
        (None, Some(r))    => Some(r.into()),
        (Some(l), Some(r)) => Some(format!("{l}, {r}")),
    }
}

fn describe_leg(hip: (f32,f32,f32), kn: (f32,f32,f32), an: (f32,f32,f32), side: &str) -> Option<String> {
    // Y: smaller = higher on body
    let knee_raised  = hip.1 - kn.1 > 40.0;   // knee above hip
    let knee_fwd     = hip.2 - kn.2 > 40.0;   // knee toward viewer
    let knee_back    = kn.2 - hip.2 > 40.0;   // knee into scene
    let ankle_fwd    = hip.2 - an.2 > 60.0;   // ankle toward viewer (step forward)
    let ankle_back   = an.2 - hip.2 > 60.0;   // ankle behind
    let leg_out      = (an.1 - kn.1).abs() < 40.0 && (kn.1 - hip.1).abs() > 50.0;

    let bend = joint_angle(hip, kn, an);

    if knee_raised {
        return Some(format!("{side} knee raised"));
    }
    if ankle_fwd && !knee_raised {
        return Some(format!("{side} leg forward"));
    }
    if ankle_back && !knee_raised {
        return Some(format!("{side} leg back"));
    }
    if knee_fwd && bend < 120.0 {
        return Some(format!("{side} leg bent"));
    }
    if knee_back && bend < 120.0 {
        return Some(format!("{side} leg bent back"));
    }
    if bend > 160.0 {
        return Some(format!("{side} leg straight"));
    }

    None
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Angle at the middle joint in degrees (180 = fully straight).
fn joint_angle(a: (f32,f32,f32), b: (f32,f32,f32), c: (f32,f32,f32)) -> f32 {
    let ba = (a.0-b.0, a.1-b.1, a.2-b.2);
    let bc = (c.0-b.0, c.1-b.1, c.2-b.2);
    let dot = ba.0*bc.0 + ba.1*bc.1 + ba.2*bc.2;
    let mag = ((ba.0*ba.0+ba.1*ba.1+ba.2*ba.2) * (bc.0*bc.0+bc.1*bc.1+bc.2*bc.2)).sqrt();
    if mag < 0.001 { return 180.0; }
    (dot / mag).clamp(-1.0, 1.0).acos().to_degrees()
}

/// If left and right descriptions match a known symmetric pair, return the collapsed form.
fn symmetrize(left: &Option<String>, right: &Option<String>, pairs: &[(&str, &str, &str)]) -> Option<String> {
    let l = left.as_deref().unwrap_or("");
    let r = right.as_deref().unwrap_or("");
    for (lp, rp, combined) in pairs {
        if l == *lp && r == *rp { return Some((*combined).into()); }
    }
    None
}
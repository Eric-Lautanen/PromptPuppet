// ui_canvas.rs
use egui::{Vec2, Pos2, Color32, Stroke, Response, Ui, Rect};
use crate::pose::{Pose, Joint};

pub struct CanvasState {
    pub dragging_joint: Option<String>,
    pub image_scale: f32,
    pub image_rect: Rect,
    pub last_debug_time: f64,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self { 
            dragging_joint: None, 
            image_scale: 1.0, 
            image_rect: Rect::NOTHING,
            last_debug_time: 0.0,
        }
    }
}

const UPPER_ARM: f32 = 89.4;
const FOREARM:   f32 = 89.4;
const THIGH:     f32 = 89.4;
const SHIN:      f32 = 80.0;
const NECK_LEN:  f32 = 40.0;
const TORSO_UPPER: f32 = 160.0;

fn constrain(from: (f32, f32), to: (f32, f32), length: f32) -> (f32, f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.1 { return (from.0 + length, from.1); }
    let s = length / dist;
    (from.0 + dx * s, from.1 + dy * s)
}

fn debug_all_joints(label: &str, pose: &Pose, last_debug_time: &mut f64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    
    if now - *last_debug_time < 0.5 { return; }
    *last_debug_time = now;
    
    println!("\n═══════════════════════════════════════════════════════════════");
    println!(" {} JOINT POSITIONS", label);
    println!("═══════════════════════════════════════════════════════════════");
    
    let joints = [
        ("head", &pose.head),
        ("L shoulder", &pose.left_shoulder),
        ("R shoulder", &pose.right_shoulder),
        ("L elbow", &pose.left_elbow),
        ("R elbow", &pose.right_elbow),
        ("L wrist", &pose.left_wrist),
        ("R wrist", &pose.right_wrist),
        ("hips", &pose.hips),
        ("L knee", &pose.left_knee),
        ("R knee", &pose.right_knee),
        ("L ankle", &pose.left_ankle),
        ("R ankle", &pose.right_ankle),
    ];
    
    for (name, joint) in joints {
        println!(" {:<12} │ 2D: ({:>7.1}, {:>7.1}) │ 3D: ({:>7.1}, {:>7.1}, {:>6.1})",
            name, joint.x, joint.y, joint.x, joint.y, joint.z);
    }
    
    println!("═══════════════════════════════════════════════════════════════\n");
}

fn set_xy(j: &mut Joint, (x, y): (f32, f32)) { j.x = x; j.y = y; }

pub fn draw_pose_canvas(
    ui: &mut Ui, pose: &mut Pose, canvas_state: &mut CanvasState,
    available_size: Vec2, status_message: &str, status_timer: f32,
) -> Response {
    let (response, painter) = ui.allocate_painter(available_size, egui::Sense::click_and_drag());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0,
        if ui.visuals().dark_mode { Color32::from_gray(15) } else { Color32::from_gray(85) });

    // Calculate pose bounds from torso joints only (for stable horizontal centering)
    let torso_joints = [
        &pose.head, &pose.left_shoulder, &pose.right_shoulder, &pose.hips,
    ];
    
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    
    // Get torso bounds for X centering
    for j in torso_joints {
        min_x = min_x.min(j.x);
        max_x = max_x.max(j.x);
    }
    
    // Get full pose bounds for Y (need to fit everything vertically)
    let all_joints = [
        &pose.head, &pose.left_shoulder, &pose.right_shoulder,
        &pose.left_elbow, &pose.right_elbow, &pose.left_wrist, &pose.right_wrist, &pose.hips,
        &pose.left_knee, &pose.right_knee, &pose.left_ankle, &pose.right_ankle,
    ];
    
    for j in all_joints {
        min_y = min_y.min(j.y);
        max_y = max_y.max(j.y);
        // Also expand X if limbs go beyond torso
        min_x = min_x.min(j.x);
        max_x = max_x.max(j.x);
    }
    
    // Add 10% padding (minimum 50px)
    let padding_x = ((max_x - min_x) * 0.1).max(50.0);
    let padding_y = ((max_y - min_y) * 0.1).max(50.0);
    let padded_width = (max_x - min_x) + padding_x * 2.0;
    let padded_height = (max_y - min_y) + padding_y * 2.0;
    
    // Scale to fit with 5% margin
    let scale = ((rect.width() / padded_width).min(rect.height() / padded_height) * 0.95).max(0.1);
    let display_width = padded_width * scale;
    let display_height = padded_height * scale;
    
    // Center in canvas
    let img_rect = Rect::from_min_size(
        Pos2::new(
            rect.center().x - display_width / 2.0,
            rect.center().y - display_height / 2.0,
        ),
        Vec2::new(display_width, display_height),
    );
    
    canvas_state.image_rect  = img_rect;
    canvas_state.image_scale = scale;

    let min_x = min_x - padding_x;
    let min_y = min_y - padding_y;
    
    let to_screen = |j: &Joint| Pos2::new(
        img_rect.min.x + ((j.x - min_x) / padded_width) * img_rect.width(),
        img_rect.min.y + ((j.y - min_y) / padded_height) * img_rect.height(),
    );
    let to_joint = |pos: Pos2| -> (f32, f32) {(
        min_x + ((pos.x - img_rect.min.x) / img_rect.width()).clamp(0.0, 1.0) * padded_width,
        min_y + ((pos.y - img_rect.min.y) / img_rect.height()).clamp(0.0, 1.0) * padded_height,
    )};

    let sw = 6.0;
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let (neck_c, torso_u, torso_l) = (c(180, 80, 255), c(100, 150, 255), c(0, 200, 220));
    let (ls_c, le_c) = (c(255, 160, 0), c(255, 200, 0));
    let (rs_c, re_c) = (c(80, 200, 80), c(120, 220, 100));
    let (lhip_c, lk_c) = (c(100, 220, 100), c(80, 200, 140));
    let (rhip_c, rk_c) = (c(60, 140, 255), c(80, 160, 240));

    let seg = |a: &Joint, b: &Joint, col: Color32| {
        painter.line_segment([to_screen(a), to_screen(b)], Stroke::new(sw, col));
    };
    let seg_pos = |a: Pos2, b: Pos2, col: Color32| {
        painter.line_segment([a, b], Stroke::new(sw, col));
    };

    // Arms
    seg(&pose.left_shoulder,  &pose.left_elbow,  ls_c);
    seg(&pose.left_elbow,     &pose.left_wrist,  le_c);
    seg(&pose.right_shoulder, &pose.right_elbow, rs_c);
    seg(&pose.right_elbow,    &pose.right_wrist, re_c);

    // Torso
    let ls = to_screen(&pose.left_shoulder);
    let rs = to_screen(&pose.right_shoulder);
    let hips = to_screen(&pose.hips);
    let neck_pos    = Pos2::new((ls.x + rs.x) / 2.0, ls.y - 30.0);
    let torso_mid   = Pos2::new((ls.x + rs.x) / 2.0, (ls.y + hips.y) / 2.0);
    seg_pos(to_screen(&pose.head), neck_pos, neck_c);
    seg(&pose.left_shoulder, &pose.right_shoulder, c(255, 120, 0));
    seg_pos(ls, torso_mid, torso_u);
    seg_pos(rs, torso_mid, torso_u);
    seg_pos(torso_mid, hips, torso_l);

    // Hip bar
    let hw = ls.x - rs.x;
    let left_hip  = Pos2::new(hips.x + hw * 0.15, hips.y);
    let right_hip = Pos2::new(hips.x - hw * 0.15, hips.y);
    seg_pos(left_hip, right_hip, torso_l);

    // Legs
    let draw_leg = |kn: &Joint, an: &Joint, hip: Pos2, kc: Color32, ac: Color32| {
        seg_pos(hip, to_screen(kn), kc);
        seg_pos(to_screen(kn), to_screen(an), ac);
    };
    draw_leg(&pose.left_knee,  &pose.left_ankle, left_hip,  lhip_c, lk_c);
    draw_leg(&pose.right_knee, &pose.right_ankle, right_hip, rhip_c, rk_c);

    // Joint interaction
    let ptr = response.interact_pointer_pos();
    if response.drag_started() {
        if let Some(pos) = ptr {
            let (jx, jy) = to_joint(pos);
            canvas_state.dragging_joint = find_nearest_joint(pose, jx, jy);
            if let Some(ref name) = canvas_state.dragging_joint {
                canvas_state.last_debug_time = 0.0; // Reset timer to force immediate debug
                println!("\n▶ DRAGGING JOINT: {}", name);
                debug_all_joints("START", pose, &mut canvas_state.last_debug_time);
            }
        }
    }
    if response.dragged() {
        if let (Some(name), Some(pos)) = (&canvas_state.dragging_joint.clone(), ptr) {
            let (jx, jy) = to_joint(pos);
            update_joint_position(pose, name, jx, jy);
            debug_all_joints(&format!("AFTER MOVING {}", name), pose, &mut canvas_state.last_debug_time);
        }
    }
    if response.drag_stopped() { canvas_state.dragging_joint = None; }

    // Joint handles
    let joints_and_labels: &[(&Joint, &str)] = &[
        (&pose.head,           "Head"),
        (&pose.left_shoulder,  "L Shoulder"), (&pose.right_shoulder, "R Shoulder"),
        (&pose.left_elbow,     "L Elbow"),    (&pose.right_elbow,    "R Elbow"),
        (&pose.left_wrist,     "L Wrist"),    (&pose.right_wrist,    "R Wrist"),
        (&pose.hips,           "Hips"),
        (&pose.left_knee,      "L Knee"),     (&pose.right_knee,     "R Knee"),
        (&pose.left_ankle,     "L Ankle"),    (&pose.right_ankle,    "R Ankle"),
    ];
    for (joint, label) in joints_and_labels {
        draw_joint_handle(&painter, to_screen(joint), label, &canvas_state.dragging_joint);
    }

    for joint in [&pose.left_elbow, &pose.right_elbow, &pose.left_wrist, &pose.right_wrist,
                  &pose.left_knee, &pose.right_knee, &pose.left_ankle, &pose.right_ankle] {
        draw_angle_label(&painter, to_screen(joint), joint.angle);
    }

    // Status toast
    if !status_message.is_empty() && status_timer > 0.0 {
        let is_ok  = status_message.starts_with("✅");
        let is_err = status_message.starts_with("❌");
        let alpha  = ((status_timer / 0.5).min(1.0) * 230.0) as u8;
        let rgba = |r, g, b| Color32::from_rgba_premultiplied(r, g, b, alpha);

        let (bg_col, border_col, text_col) = if is_ok {
            (rgba( 20,  60,  20), rgba( 60, 200,  60), rgba(140, 255, 140))
        } else if is_err {
            (rgba( 60,  20,  20), rgba(200,  60,  60), rgba(255, 140, 140))
        } else {
            (rgba( 30,  30,  50), rgba(120, 140, 220), rgba(200, 210, 255))
        };

        let galley = painter.layout_no_wrap(status_message.to_string(),
            egui::FontId::proportional(13.0), text_col);
        let pad = egui::vec2(14.0, 8.0);
        let toast_size = galley.size() + pad * 2.0;
        let toast_pos  = Pos2::new(rect.max.x - toast_size.x - 16.0, rect.min.y + 16.0);
        let toast_rect = Rect::from_min_size(toast_pos, toast_size);

        painter.rect_filled(toast_rect.translate(Vec2::new(2.0, 3.0)), 8.0,
            Color32::from_rgba_premultiplied(0, 0, 0, alpha / 3));
        painter.rect_filled(toast_rect, 8.0, bg_col);
        painter.rect_stroke(toast_rect, 8.0, Stroke::new(1.5, border_col), egui::StrokeKind::Inside);
        painter.galley(toast_pos + pad, galley, text_col);
    }

    response
}

fn draw_joint_handle(painter: &egui::Painter, pos: Pos2, label: &str, dragging: &Option<String>) {
    let is_dragging = dragging.as_ref().map_or(false, |d| d.contains(label));
    let base_r = match label {
        l if l.contains("Head")     => 12.0,
        l if l.contains("Hips")     => 10.0,
        l if l.contains("Shoulder") =>  9.5,
        l if l.contains("Knee") || l.contains("Elbow") => 9.0,
        l if l.contains("Hand") || l.contains("Foot")  => 8.0,
        _ => 7.0,
    };
    let r = if is_dragging { base_r * 1.25 } else { base_r };

    let fill = match label {
        "Head"               => Color32::from_rgb(255,  50, 180),
        "L Shoulder"         => Color32::from_rgb(255, 160,   0),
        "L Elbow"            => Color32::from_rgb(255, 200,   0),
        "L Wrist" | "L Hand" => Color32::from_rgb(255, 220,  80),
        "R Shoulder"         => Color32::from_rgb( 80, 200,  80),
        "R Elbow"            => Color32::from_rgb(120, 220, 100),
        "R Wrist" | "R Hand" => Color32::from_rgb(160, 255, 120),
        "Hips"               => Color32::from_rgb(  0, 200, 220),
        "L Knee"             => Color32::from_rgb( 80, 200, 140),
        "L Ankle" | "L Foot" => Color32::from_rgb( 60, 180, 200),
        "R Knee"             => Color32::from_rgb( 80, 160, 240),
        "R Ankle" | "R Foot" => Color32::from_rgb(100, 180, 255),
        _                    => Color32::from_rgb(150, 150, 200),
    };

    let stroke_col = if is_dragging { Color32::WHITE } else {
        Color32::from_rgb(
            (fill.r() as u16 + 60).min(255) as u8,
            (fill.g() as u16 + 60).min(255) as u8,
            (fill.b() as u16 + 60).min(255) as u8,
        )
    };

    painter.circle_filled(pos + Vec2::new(1.0, 1.0), r + 3.0,
        Color32::from_rgba_premultiplied(fill.r() / 2, fill.g() / 2, fill.b() / 2, 80));
    painter.circle_filled(pos, r, fill);
    painter.circle_stroke(pos, r, Stroke::new(2.0, stroke_col));
    painter.circle_filled(pos + Vec2::new(-r * 0.3, -r * 0.3), r * 0.4,
        Color32::from_rgba_premultiplied(255, 255, 255, 180));
}

fn draw_angle_label(painter: &egui::Painter, pos: Pos2, angle: f32) {
    let text = format!("{:.0}°", angle);
    let col  = Color32::from_rgb(240, 240, 255);
    let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(10.0), col);
    let tp  = pos + Vec2::new(12.0, -8.0);
    let bg  = Rect::from_min_size(tp - Vec2::new(2.0, 1.0), galley.size() + Vec2::new(6.0, 3.0));
    painter.rect_filled(bg, 3.0, Color32::from_rgba_premultiplied(20, 30, 50, 200));
    painter.rect_stroke(bg, 3.0, Stroke::new(1.0, Color32::from_rgb(100, 140, 200)), egui::StrokeKind::Inside);
    painter.text(tp + Vec2::new(2.0, 1.0), egui::Align2::LEFT_TOP, text, egui::FontId::proportional(10.0), col);
}

fn find_nearest_joint(pose: &Pose, x: f32, y: f32) -> Option<String> {
    [("head", &pose.head), ("left_shoulder", &pose.left_shoulder), ("right_shoulder", &pose.right_shoulder),
     ("left_elbow", &pose.left_elbow), ("right_elbow", &pose.right_elbow),
     ("left_wrist", &pose.left_wrist), ("right_wrist", &pose.right_wrist),
     ("hips", &pose.hips),
     ("left_knee", &pose.left_knee), ("right_knee", &pose.right_knee),
     ("left_ankle", &pose.left_ankle), ("right_ankle", &pose.right_ankle)]
        .iter()
        .filter_map(|(name, j)| {
            let d = j.distance_to(x, y);
            if d < 25.0 { Some((name, d)) } else { None }
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(name, _)| name.to_string())
}

fn update_joint_position(pose: &mut Pose, joint_name: &str, x: f32, y: f32) {
    match joint_name {
        "head" => {
            // Head constrained to neck position (above shoulder midpoint)
            let neck_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
            let neck_y = pose.left_shoulder.y.min(pose.right_shoulder.y) - 30.0;
            let constrained = constrain((neck_x, neck_y), (x, y), NECK_LEN);
            pose.head.x = constrained.0;
            pose.head.y = constrained.1;
        }
        "hips" => {
            // Hips constrained to torso (below shoulder midpoint)
            let torso_top_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
            let torso_top_y = (pose.left_shoulder.y + pose.right_shoulder.y) / 2.0;
            let constrained = constrain((torso_top_x, torso_top_y), (x, y), TORSO_UPPER);
            pose.hips.x = constrained.0;
            pose.hips.y = constrained.1;
        }

        "left_shoulder" => {
            pose.left_shoulder.x = x; pose.left_shoulder.y = y;
            let el = (pose.left_elbow.x, pose.left_elbow.y);
            let wr = (pose.left_wrist.x, pose.left_wrist.y);
            let el2 = constrain((x, y), el, UPPER_ARM);
            set_xy(&mut pose.left_elbow, el2);
            set_xy(&mut pose.left_wrist, constrain(el2, wr, FOREARM));
        }
        "right_shoulder" => {
            pose.right_shoulder.x = x; pose.right_shoulder.y = y;
            let el = (pose.right_elbow.x, pose.right_elbow.y);
            let wr = (pose.right_wrist.x, pose.right_wrist.y);
            let el2 = constrain((x, y), el, UPPER_ARM);
            set_xy(&mut pose.right_elbow, el2);
            set_xy(&mut pose.right_wrist, constrain(el2, wr, FOREARM));
        }

        "left_elbow" => {
            let sh = (pose.left_shoulder.x, pose.left_shoulder.y);
            let wr = (pose.left_wrist.x, pose.left_wrist.y);
            let el2 = constrain(sh, (x, y), UPPER_ARM);
            set_xy(&mut pose.left_elbow, el2);
            set_xy(&mut pose.left_wrist, constrain(el2, wr, FOREARM));
            pose.update_joint_angle("left_elbow", sh.0, sh.1);
        }
        "right_elbow" => {
            let sh = (pose.right_shoulder.x, pose.right_shoulder.y);
            let wr = (pose.right_wrist.x, pose.right_wrist.y);
            let el2 = constrain(sh, (x, y), UPPER_ARM);
            set_xy(&mut pose.right_elbow, el2);
            set_xy(&mut pose.right_wrist, constrain(el2, wr, FOREARM));
            pose.update_joint_angle("right_elbow", sh.0, sh.1);
        }

        "left_wrist" => {
            let el = (pose.left_elbow.x, pose.left_elbow.y);
            set_xy(&mut pose.left_wrist, constrain(el, (x, y), FOREARM));
            pose.update_joint_angle("left_wrist", el.0, el.1);
        }
        "right_wrist" => {
            let el = (pose.right_elbow.x, pose.right_elbow.y);
            set_xy(&mut pose.right_wrist, constrain(el, (x, y), FOREARM));
            pose.update_joint_angle("right_wrist", el.0, el.1);
        }

        "left_knee" => {
            let hip = (pose.left_shoulder.x, pose.hips.y);
            let hxy = (pose.hips.x, pose.hips.y);
            let an = (pose.left_ankle.x, pose.left_ankle.y);
            let kn2 = constrain(hip, (x, y), THIGH);
            set_xy(&mut pose.left_knee, kn2);
            set_xy(&mut pose.left_ankle, constrain(kn2, an, SHIN));
            pose.update_joint_angle("left_knee", hxy.0, hxy.1);
        }
        "right_knee" => {
            let hip = (pose.right_shoulder.x, pose.hips.y);
            let hxy = (pose.hips.x, pose.hips.y);
            let an = (pose.right_ankle.x, pose.right_ankle.y);
            let kn2 = constrain(hip, (x, y), THIGH);
            set_xy(&mut pose.right_knee, kn2);
            set_xy(&mut pose.right_ankle, constrain(kn2, an, SHIN));
            pose.update_joint_angle("right_knee", hxy.0, hxy.1);
        }

        "left_ankle" => {
            let kn = (pose.left_knee.x, pose.left_knee.y);
            set_xy(&mut pose.left_ankle, constrain(kn, (x, y), SHIN));
            pose.update_joint_angle("left_ankle", kn.0, kn.1);
        }
        "right_ankle" => {
            let kn = (pose.right_knee.x, pose.right_knee.y);
            set_xy(&mut pose.right_ankle, constrain(kn, (x, y), SHIN));
            pose.update_joint_angle("right_ankle", kn.0, kn.1);
        }
        _ => {}
    }
    pose.clamp_angles();
}

pub fn normalize_pose(pose: &mut Pose) {
    // Fix head relative to shoulder midpoint
    let neck_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
    let neck_y = pose.left_shoulder.y.min(pose.right_shoulder.y) - 30.0;
    let head_pos = constrain((neck_x, neck_y), (pose.head.x, pose.head.y), NECK_LEN);
    set_xy(&mut pose.head, head_pos);
    
    // Fix hips relative to shoulder midpoint
    let torso_top_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
    let torso_top_y = (pose.left_shoulder.y + pose.right_shoulder.y) / 2.0;
    let hips_pos = constrain((torso_top_x, torso_top_y), (pose.hips.x, pose.hips.y), TORSO_UPPER);
    set_xy(&mut pose.hips, hips_pos);
    
    // Fix left arm chain: shoulder → elbow → wrist
    let ls = (pose.left_shoulder.x, pose.left_shoulder.y);
    let le = (pose.left_elbow.x, pose.left_elbow.y);
    let lw = (pose.left_wrist.x, pose.left_wrist.y);
    let le2 = constrain(ls, le, UPPER_ARM);
    set_xy(&mut pose.left_elbow, le2);
    set_xy(&mut pose.left_wrist, constrain(le2, lw, FOREARM));
    
    // Fix right arm chain: shoulder → elbow → wrist
    let rs = (pose.right_shoulder.x, pose.right_shoulder.y);
    let re = (pose.right_elbow.x, pose.right_elbow.y);
    let rw = (pose.right_wrist.x, pose.right_wrist.y);
    let re2 = constrain(rs, re, UPPER_ARM);
    set_xy(&mut pose.right_elbow, re2);
    set_xy(&mut pose.right_wrist, constrain(re2, rw, FOREARM));
    
    // Fix left leg chain: hip → knee → ankle
    let lhip = (pose.left_shoulder.x, pose.hips.y);
    let lk = (pose.left_knee.x, pose.left_knee.y);
    let la = (pose.left_ankle.x, pose.left_ankle.y);
    let lk2 = constrain(lhip, lk, THIGH);
    set_xy(&mut pose.left_knee, lk2);
    set_xy(&mut pose.left_ankle, constrain(lk2, la, SHIN));
    
    // Fix right leg chain: hip → knee → ankle
    let rhip = (pose.right_shoulder.x, pose.hips.y);
    let rk = (pose.right_knee.x, pose.right_knee.y);
    let ra = (pose.right_ankle.x, pose.right_ankle.y);
    let rk2 = constrain(rhip, rk, THIGH);
    set_xy(&mut pose.right_knee, rk2);
    set_xy(&mut pose.right_ankle, constrain(rk2, ra, SHIN));
    
    pose.clamp_angles();
}
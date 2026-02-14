// ui_canvas.rs
use egui::{Vec2, Pos2, Color32, Stroke, Response, Ui, Rect};
use crate::pose::{Pose, Joint};

pub struct CanvasState {
    pub dragging_joint: Option<String>,
    pub image_scale: f32,
    pub image_rect: Rect,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self { dragging_joint: None, image_scale: 1.0, image_rect: Rect::NOTHING }
    }
}

const UPPER_ARM: f32 = 116.0;
const FOREARM:   f32 =  90.0;
const HAND_LEN:  f32 =  30.0;
const THIGH:     f32 = 116.0;
const SHIN:      f32 =  86.0;
const FOOT_LEN:  f32 =  30.0;

fn constrain(from: (f32, f32), to: (f32, f32), length: f32) -> (f32, f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.1 { return (from.0 + length, from.1); }
    let s = length / dist;
    (from.0 + dx * s, from.1 + dy * s)
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

    let default_size = Vec2::new(800.0, 600.0);
    let scale = (rect.width() / default_size.x).min(rect.height() / default_size.y) * 0.85;
    let img_rect = Rect::from_center_size(rect.center(), default_size * scale);
    canvas_state.image_rect  = img_rect;
    canvas_state.image_scale = scale;

    let to_screen = |j: &Joint| Pos2::new(
        img_rect.min.x + (j.x / 800.0) * img_rect.width(),
        img_rect.min.y + (j.y / 600.0) * img_rect.height(),
    );
    let to_joint = |pos: Pos2| -> (f32, f32) {(
        ((pos.x - img_rect.min.x) / img_rect.width()).clamp(0.0, 1.0) * 800.0,
        ((pos.y - img_rect.min.y) / img_rect.height()).clamp(0.0, 1.0) * 600.0,
    )};

    let sw = 6.0;
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let (neck_c, torso_u, torso_l) = (c(180, 80, 255), c(100, 150, 255), c(0, 200, 220));
    let (ls_c, le_c, lh_c) = (c(255, 160, 0), c(255, 200, 0), c(255, 220, 80));
    let (rs_c, re_c, rh_c) = (c(80, 200, 80), c(120, 220, 100), c(160, 255, 120));
    let (lhip_c, lk_c, lf_c) = (c(100, 220, 100), c(80, 200, 140), c(60, 180, 200));
    let (rhip_c, rk_c, rf_c) = (c(60, 140, 255), c(80, 160, 240), c(100, 180, 255));

    let seg = |a: &Joint, b: &Joint, col: Color32| {
        painter.line_segment([to_screen(a), to_screen(b)], Stroke::new(sw, col));
    };
    let seg_pos = |a: Pos2, b: Pos2, col: Color32| {
        painter.line_segment([a, b], Stroke::new(sw, col));
    };

    // Arms
    seg(&pose.left_shoulder,  &pose.left_elbow,  ls_c);
    seg(&pose.left_elbow,     &pose.left_wrist,  le_c);
    seg(&pose.left_wrist,     &pose.left_hand,   lh_c);
    seg(&pose.right_shoulder, &pose.right_elbow, rs_c);
    seg(&pose.right_elbow,    &pose.right_wrist, re_c);
    seg(&pose.right_wrist,    &pose.right_hand,  rh_c);

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
    let draw_leg = |kn: &Joint, an: &Joint, ft: &Joint, hip: Pos2, kc: Color32, ac: Color32, fc: Color32| {
        seg_pos(hip,            to_screen(kn), kc);
        seg_pos(to_screen(kn),  to_screen(an), ac);
        seg_pos(to_screen(an),  to_screen(ft), fc);
    };
    draw_leg(&pose.left_knee,  &pose.left_ankle,  &pose.left_foot,  left_hip,  lhip_c, lk_c, lf_c);
    draw_leg(&pose.right_knee, &pose.right_ankle, &pose.right_foot, right_hip, rhip_c, rk_c, rf_c);

    // Joint interaction
    let ptr = response.interact_pointer_pos();
    if response.drag_started() {
        if let Some(pos) = ptr {
            let (jx, jy) = to_joint(pos);
            canvas_state.dragging_joint = find_nearest_joint(pose, jx, jy);
        }
    }
    if response.dragged() {
        if let (Some(name), Some(pos)) = (&canvas_state.dragging_joint.clone(), ptr) {
            let (jx, jy) = to_joint(pos);
            update_joint_position(pose, name, jx, jy);
        }
    }
    if response.drag_stopped() { canvas_state.dragging_joint = None; }

    // Joint handles
    let joints_and_labels: &[(&Joint, &str)] = &[
        (&pose.head,           "Head"),
        (&pose.left_shoulder,  "L Shoulder"), (&pose.right_shoulder, "R Shoulder"),
        (&pose.left_elbow,     "L Elbow"),    (&pose.right_elbow,    "R Elbow"),
        (&pose.left_wrist,     "L Wrist"),    (&pose.right_wrist,    "R Wrist"),
        (&pose.left_hand,      "L Hand"),     (&pose.right_hand,     "R Hand"),
        (&pose.hips,           "Hips"),
        (&pose.left_knee,      "L Knee"),     (&pose.right_knee,     "R Knee"),
        (&pose.left_ankle,     "L Ankle"),    (&pose.right_ankle,    "R Ankle"),
        (&pose.left_foot,      "L Foot"),     (&pose.right_foot,     "R Foot"),
    ];
    for (joint, label) in joints_and_labels {
        draw_joint_handle(&painter, to_screen(joint), label, &canvas_state.dragging_joint);
    }

    for joint in [&pose.left_elbow, &pose.right_elbow, &pose.left_knee, &pose.right_knee] {
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
     ("left_hand", &pose.left_hand), ("right_hand", &pose.right_hand),
     ("hips", &pose.hips),
     ("left_knee", &pose.left_knee), ("right_knee", &pose.right_knee),
     ("left_ankle", &pose.left_ankle), ("right_ankle", &pose.right_ankle),
     ("left_foot", &pose.left_foot), ("right_foot", &pose.right_foot)]
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
        "head" => { pose.head.x = x; pose.head.y = y; }
        "hips" => { pose.hips.x = x; pose.hips.y = y; }

        "left_shoulder" => {
            pose.left_shoulder.x = x; pose.left_shoulder.y = y;
            let (sh, el, wr, ha) = ((x, y), (pose.left_elbow.x, pose.left_elbow.y),
                (pose.left_wrist.x, pose.left_wrist.y), (pose.left_hand.x, pose.left_hand.y));
            let el2 = constrain(sh, el, UPPER_ARM);
            let wr2 = constrain(el2, wr, FOREARM);
            set_xy(&mut pose.left_elbow, el2); set_xy(&mut pose.left_wrist, wr2);
            set_xy(&mut pose.left_hand, constrain(wr2, ha, HAND_LEN));
        }
        "right_shoulder" => {
            pose.right_shoulder.x = x; pose.right_shoulder.y = y;
            let (sh, el, wr, ha) = ((x, y), (pose.right_elbow.x, pose.right_elbow.y),
                (pose.right_wrist.x, pose.right_wrist.y), (pose.right_hand.x, pose.right_hand.y));
            let el2 = constrain(sh, el, UPPER_ARM);
            let wr2 = constrain(el2, wr, FOREARM);
            set_xy(&mut pose.right_elbow, el2); set_xy(&mut pose.right_wrist, wr2);
            set_xy(&mut pose.right_hand, constrain(wr2, ha, HAND_LEN));
        }

        "left_elbow" => {
            let (sh, wr, ha) = ((pose.left_shoulder.x, pose.left_shoulder.y),
                (pose.left_wrist.x, pose.left_wrist.y), (pose.left_hand.x, pose.left_hand.y));
            let el2 = constrain(sh, (x, y), UPPER_ARM);
            let wr2 = constrain(el2, wr, FOREARM);
            set_xy(&mut pose.left_elbow, el2); set_xy(&mut pose.left_wrist, wr2);
            set_xy(&mut pose.left_hand, constrain(wr2, ha, HAND_LEN));
            pose.update_joint_angle("left_elbow", sh.0, sh.1);
        }
        "right_elbow" => {
            let (sh, wr, ha) = ((pose.right_shoulder.x, pose.right_shoulder.y),
                (pose.right_wrist.x, pose.right_wrist.y), (pose.right_hand.x, pose.right_hand.y));
            let el2 = constrain(sh, (x, y), UPPER_ARM);
            let wr2 = constrain(el2, wr, FOREARM);
            set_xy(&mut pose.right_elbow, el2); set_xy(&mut pose.right_wrist, wr2);
            set_xy(&mut pose.right_hand, constrain(wr2, ha, HAND_LEN));
            pose.update_joint_angle("right_elbow", sh.0, sh.1);
        }

        "left_wrist" => {
            let (el, ha) = ((pose.left_elbow.x, pose.left_elbow.y), (pose.left_hand.x, pose.left_hand.y));
            let wr2 = constrain(el, (x, y), FOREARM);
            set_xy(&mut pose.left_wrist, wr2);
            set_xy(&mut pose.left_hand, constrain(wr2, ha, HAND_LEN));
            pose.update_joint_angle("left_wrist", el.0, el.1);
        }
        "right_wrist" => {
            let (el, ha) = ((pose.right_elbow.x, pose.right_elbow.y), (pose.right_hand.x, pose.right_hand.y));
            let wr2 = constrain(el, (x, y), FOREARM);
            set_xy(&mut pose.right_wrist, wr2);
            set_xy(&mut pose.right_hand, constrain(wr2, ha, HAND_LEN));
            pose.update_joint_angle("right_wrist", el.0, el.1);
        }

        "left_hand" => {
            let wr = (pose.left_wrist.x, pose.left_wrist.y);
            set_xy(&mut pose.left_hand, constrain(wr, (x, y), HAND_LEN));
            pose.update_joint_angle("left_hand", wr.0, wr.1);
        }
        "right_hand" => {
            let wr = (pose.right_wrist.x, pose.right_wrist.y);
            set_xy(&mut pose.right_hand, constrain(wr, (x, y), HAND_LEN));
            pose.update_joint_angle("right_hand", wr.0, wr.1);
        }

        "left_knee" => {
            let (hip, an, ft, hxy) = ((pose.left_shoulder.x, pose.hips.y),
                (pose.left_ankle.x, pose.left_ankle.y), (pose.left_foot.x, pose.left_foot.y),
                (pose.hips.x, pose.hips.y));
            let kn2 = constrain(hip, (x, y), THIGH);
            let an2 = constrain(kn2, an, SHIN);
            set_xy(&mut pose.left_knee, kn2); set_xy(&mut pose.left_ankle, an2);
            set_xy(&mut pose.left_foot, constrain(an2, ft, FOOT_LEN));
            pose.update_joint_angle("left_knee", hxy.0, hxy.1);
        }
        "right_knee" => {
            let (hip, an, ft, hxy) = ((pose.right_shoulder.x, pose.hips.y),
                (pose.right_ankle.x, pose.right_ankle.y), (pose.right_foot.x, pose.right_foot.y),
                (pose.hips.x, pose.hips.y));
            let kn2 = constrain(hip, (x, y), THIGH);
            let an2 = constrain(kn2, an, SHIN);
            set_xy(&mut pose.right_knee, kn2); set_xy(&mut pose.right_ankle, an2);
            set_xy(&mut pose.right_foot, constrain(an2, ft, FOOT_LEN));
            pose.update_joint_angle("right_knee", hxy.0, hxy.1);
        }

        "left_ankle" => {
            let (kn, ft) = ((pose.left_knee.x, pose.left_knee.y), (pose.left_foot.x, pose.left_foot.y));
            let an2 = constrain(kn, (x, y), SHIN);
            set_xy(&mut pose.left_ankle, an2);
            set_xy(&mut pose.left_foot, constrain(an2, ft, FOOT_LEN));
            pose.update_joint_angle("left_ankle", kn.0, kn.1);
        }
        "right_ankle" => {
            let (kn, ft) = ((pose.right_knee.x, pose.right_knee.y), (pose.right_foot.x, pose.right_foot.y));
            let an2 = constrain(kn, (x, y), SHIN);
            set_xy(&mut pose.right_ankle, an2);
            set_xy(&mut pose.right_foot, constrain(an2, ft, FOOT_LEN));
            pose.update_joint_angle("right_ankle", kn.0, kn.1);
        }

        "left_foot" => {
            let an = (pose.left_ankle.x, pose.left_ankle.y);
            set_xy(&mut pose.left_foot, constrain(an, (x, y), FOOT_LEN));
            pose.update_joint_angle("left_foot", an.0, an.1);
        }
        "right_foot" => {
            let an = (pose.right_ankle.x, pose.right_ankle.y);
            set_xy(&mut pose.right_foot, constrain(an, (x, y), FOOT_LEN));
            pose.update_joint_angle("right_foot", an.0, an.1);
        }
        _ => {}
    }
    pose.clamp_angles();
}
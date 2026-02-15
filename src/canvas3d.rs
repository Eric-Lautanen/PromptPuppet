// src/canvas3d.rs
//
// Software 3D renderer for the pose, drawn entirely with egui's painter.
//
// Coordinate system:
//   Pose space:  x[0..800], y[0..600] (y-down), z arbitrary (~±100)
//   World space: centred at origin, 1 unit = 100 pose units
//                x: (pose_x - 400) / 100
//                y: -(pose_y - 300) / 100   ← flip Y
//                z:  pose_z / 100
//
// Camera: spherical orbit around a focus point.
// Projection: simple perspective divide.

use egui::{Painter, Pos2, Vec2, Color32, Stroke, Rect, Ui, Response, Sense};
use crate::pose::{Pose, Joint};

// ── Bone length constants ─────────────────────────────────────────────────────

const UPPER_ARM: f32 = 89.4;
const FOREARM:   f32 = 89.4;
const THIGH:     f32 = 89.4;
const SHIN:      f32 = 80.0;
const NECK_LEN:  f32 = 40.0;
const TORSO_UPPER: f32 = 160.0;

// ── Camera state ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Camera3D {
    pub focus:  [f32; 3],
    pub yaw:    f32,   // radians, horizontal rotation
    pub pitch:  f32,   // radians, vertical rotation
    pub radius: f32,   // distance from focus
    pub fov:    f32,   // vertical field of view in radians
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            focus:  [0.0, 0.0, 0.0],     // Focus at center
            yaw:    0.5,                  // Slight angle
            pitch:  0.15,                 // Slight downward angle
            radius: 5.5,                  // Distance for full figure view
            fov:    std::f32::consts::FRAC_PI_4,
        }
    }
}

impl Camera3D {
    /// World-space eye position.
    fn eye(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [
            self.focus[0] + self.radius * cp * sy,
            self.focus[1] + self.radius * sp,
            self.focus[2] + self.radius * cp * cy,
        ]
    }

    /// Project a world-space point to normalised screen coords [-1..1].
    /// Returns None if behind camera.
    fn project(&self, p: [f32; 3], aspect: f32) -> Option<[f32; 3]> {
        let eye   = self.eye();
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();

        // Camera basis vectors
        let fwd = [-cp * sy, -sp, -cp * cy];
        let right = [cy, 0.0, -sy];
        let up = [
            sp * sy,
            cp,
            sp * cy,
        ];

        let d = [p[0]-eye[0], p[1]-eye[1], p[2]-eye[2]];
        let z = dot(d, fwd);
        if z < 0.01 { return None; }

        let x = dot(d, right);
        let y = dot(d, up);

        let half_h = (self.fov * 0.5).tan();
        let half_w = half_h * aspect;

        Some([x / (z * half_w), y / (z * half_h), z])
    }

    /// Project to a pixel Pos2 inside `rect`. Returns (pos, depth).
    pub fn project_to_rect(&self, p: [f32; 3], rect: Rect) -> Option<(Pos2, f32)> {
        let aspect = rect.width() / rect.height();
        let [nx, ny, z] = self.project(p, aspect)?;
        let px = rect.center().x + nx * rect.width()  * 0.5;
        let py = rect.center().y - ny * rect.height() * 0.5;
        Some((Pos2::new(px, py), z))
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0]*b[0] + a[1]*b[1] + a[2]*b[2]
}

// ── Pose → world ──────────────────────────────────────────────────────────────

fn to_world(j: &Joint) -> [f32; 3] {
    [
         (j.x - 400.0) / 150.0,
        -(j.y - 539.0) / 150.0,  // Use cy=539 to match app.rs
          j.z           / 150.0,
    ]
}

// ── Public draw function ──────────────────────────────────────────────────────

pub fn draw_3d_canvas(
    ui:     &mut Ui,
    pose:   &mut Pose,
    camera: &mut Camera3D,
    size:   Vec2,
    dragging_joint: &mut Option<String>,
) -> Response {
    let (response, painter) =
        ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;

    // Background
    painter.rect_filled(rect, 0.0,
        if ui.visuals().dark_mode { Color32::from_gray(18) }
        else                      { Color32::from_gray(80) });

    // Calculate pose bounds in world space for auto-framing
    let joints = [
        &pose.head, &pose.left_shoulder, &pose.right_shoulder,
        &pose.left_elbow, &pose.right_elbow, &pose.left_wrist, &pose.right_wrist, &pose.hips,
        &pose.left_knee, &pose.right_knee, &pose.left_ankle, &pose.right_ankle,
    ];
    
    let world_joints: Vec<[f32; 3]> = joints.iter().map(|j| to_world(j)).collect();
    
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
    
    for w in &world_joints {
        min_x = min_x.min(w[0]); max_x = max_x.max(w[0]);
        min_y = min_y.min(w[1]); max_y = max_y.max(w[1]);
        min_z = min_z.min(w[2]); max_z = max_z.max(w[2]);
    }
    
    // Calculate pose center and size
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    let center_z = (min_z + max_z) / 2.0;
    
    let size_y = max_y - min_y;
    let max_dimension = (max_x - min_x).max(size_y).max(max_z - min_z);
    
    // Auto-adjust camera on first load (radius at default 5.5 or focus at origin)
    let is_default_camera = (camera.radius - 5.5).abs() < 0.1 || 
                            (camera.focus[0].abs() + camera.focus[1].abs() + camera.focus[2].abs()) < 0.1;
    
    if is_default_camera {
        camera.focus = [center_x, center_y + size_y * 0.1, center_z]; // Focus slightly above center
        camera.radius = (max_dimension * 2.2).max(3.5); // 2.2x for comfortable framing
        camera.pitch = 0.0; // Straight-on view
    }

    // ── Input: joint manipulation or camera rotation ─────────────────────────

    let ptr = response.interact_pointer_pos();
    
    // Start dragging a joint
    if response.drag_started() {
        if let Some(pos) = ptr {
            *dragging_joint = find_nearest_joint_3d(pose, camera, rect, pos);
        }
    }
    
    // Update joint position or rotate camera
    if response.dragged() {
        if let (Some(joint_name), Some(pos)) = (dragging_joint.as_ref(), ptr) {
            // Dragging a joint - move it in screen space
            update_joint_3d(pose, joint_name, camera, rect, pos);
        } else {
            // No joint selected - rotate camera
            camera.yaw -= response.drag_delta().x * 0.008;
        }
    }
    
    // Stop dragging
    if response.drag_stopped() {
        *dragging_joint = None;
    }
    
    // Zoom
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            camera.radius = (camera.radius - scroll * 0.01).clamp(1.0, 20.0);
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    let proj = |j: &Joint| camera.project_to_rect(to_world(j), rect);

    // ── Draw ground grid ──────────────────────────────────────────────────────

    draw_grid(&painter, camera, rect);

    // ── Collect bones ─────────────────────────────────────────────────────────

    #[derive(Clone)]
    struct BoneDrawCmd { a: Pos2, b: Pos2, depth: f32, color: Color32 }
    #[derive(Clone)]
    struct JointDrawCmd { pos: Pos2, depth: f32, radius: f32, color: Color32 }

    let mut bones: Vec<BoneDrawCmd> = Vec::new();
    let mut joints: Vec<JointDrawCmd> = Vec::new();

    let bone = |a: &Joint, b: &Joint, col: Color32,
                bones: &mut Vec<BoneDrawCmd>| {
        if let (Some((pa, za)), Some((pb, zb))) = (proj(a), proj(b)) {
            bones.push(BoneDrawCmd { a: pa, b: pb, depth: (za+zb)*0.5, color: col });
        }
    };

    // Colors matching the 2D canvas
    let c = |r: u8, g: u8, b: u8| Color32::from_rgb(r, g, b);

    // Arms L
    bone(&pose.left_shoulder, &pose.left_elbow,  c(255,160,  0), &mut bones);
    bone(&pose.left_elbow,    &pose.left_wrist,  c(255,200,  0), &mut bones);
    // Arms R
    bone(&pose.right_shoulder, &pose.right_elbow,  c( 80,200, 80), &mut bones);
    bone(&pose.right_elbow,    &pose.right_wrist,  c(120,220,100), &mut bones);
    // Shoulders
    bone(&pose.left_shoulder,  &pose.right_shoulder, c(255,120,  0), &mut bones);
    
    // Torso structure (matching 2D: neck, upper torso, lower torso, hip bar)
    // Neck from shoulders midpoint to head
    if let (Some((ls_pos, ls_z)), Some((rs_pos, rs_z)), Some((head_pos, head_z))) = 
        (proj(&pose.left_shoulder), proj(&pose.right_shoulder), proj(&pose.head)) {
        let neck_pos = Pos2::new((ls_pos.x + rs_pos.x) / 2.0, ls_pos.y - 30.0);
        let neck_depth = (ls_z + rs_z + head_z) / 3.0;
        bones.push(BoneDrawCmd { a: head_pos, b: neck_pos, depth: neck_depth, color: c(180, 80, 255) });
    }
    
    // Upper and lower torso
    if let (Some((ls_pos, ls_z)), Some((rs_pos, rs_z)), Some((hips_pos, hips_z))) = 
        (proj(&pose.left_shoulder), proj(&pose.right_shoulder), proj(&pose.hips)) {
        let torso_mid = Pos2::new((ls_pos.x + rs_pos.x) / 2.0, (ls_pos.y + hips_pos.y) / 2.0);
        let mid_depth = (ls_z + rs_z + hips_z) / 3.0;
        
        // Upper torso (shoulders to mid)
        bones.push(BoneDrawCmd { a: ls_pos, b: torso_mid, depth: (ls_z + mid_depth) / 2.0, color: c(100,150,255) });
        bones.push(BoneDrawCmd { a: rs_pos, b: torso_mid, depth: (rs_z + mid_depth) / 2.0, color: c(100,150,255) });
        // Lower torso (mid to hips)
        bones.push(BoneDrawCmd { a: torso_mid, b: hips_pos, depth: (mid_depth + hips_z) / 2.0, color: c(0,200,220) });
        
        // Hip bar
        let hw = (ls_pos.x - rs_pos.x).abs();
        let left_hip = Pos2::new(hips_pos.x + hw * 0.15, hips_pos.y);
        let right_hip = Pos2::new(hips_pos.x - hw * 0.15, hips_pos.y);
        bones.push(BoneDrawCmd { a: left_hip, b: right_hip, depth: hips_z, color: c(0,200,220) });
    }
    
    // Legs L (from hip bar position)
    if let (Some((hips_pos, hips_z)), Some((lk_pos, lk_z))) = 
        (proj(&pose.hips), proj(&pose.left_knee)) {
        if let Some((ls_pos, _)) = proj(&pose.left_shoulder) {
            let hw = (ls_pos.x - hips_pos.x).abs();
            let left_hip = Pos2::new(hips_pos.x + hw * 0.15, hips_pos.y);
            bones.push(BoneDrawCmd { a: left_hip, b: lk_pos, depth: (hips_z + lk_z) / 2.0, color: c(100,220,100) });
        }
    }
    bone(&pose.left_knee,  &pose.left_ankle, c( 80,200,140), &mut bones);
    
    // Legs R (from hip bar position)
    if let (Some((hips_pos, hips_z)), Some((rk_pos, rk_z))) = 
        (proj(&pose.hips), proj(&pose.right_knee)) {
        if let Some((rs_pos, _)) = proj(&pose.right_shoulder) {
            let hw = (hips_pos.x - rs_pos.x).abs();
            let right_hip = Pos2::new(hips_pos.x - hw * 0.15, hips_pos.y);
            bones.push(BoneDrawCmd { a: right_hip, b: rk_pos, depth: (hips_z + rk_z) / 2.0, color: c(60,140,255) });
        }
    }
    bone(&pose.right_knee,  &pose.right_ankle, c( 80,160,240), &mut bones);

    // Joints
    let joint_data: &[(&Joint, f32, Color32)] = &[
        (&pose.head,           14.0, c(255, 50,180)),
        (&pose.left_shoulder,  10.0, c(255,160,  0)),
        (&pose.right_shoulder, 10.0, c( 80,200, 80)),
        (&pose.left_elbow,      9.0, c(255,200,  0)),
        (&pose.right_elbow,     9.0, c(120,220,100)),
        (&pose.left_wrist,      8.0, c(255,220, 80)),
        (&pose.right_wrist,     8.0, c(160,255,120)),
        (&pose.hips,           11.0, c(  0,200,220)),
        (&pose.left_knee,       9.0, c( 80,200,140)),
        (&pose.right_knee,      9.0, c( 80,160,240)),
        (&pose.left_ankle,      8.0, c( 60,180,200)),
        (&pose.right_ankle,     8.0, c(100,180,255)),
    ];
    for (j, base_r, col) in joint_data {
        if let Some((pos, depth)) = proj(j) {
            // Scale radius by depth — farther = smaller
            let r = base_r * (4.0 / depth).clamp(0.4, 2.5);
            joints.push(JointDrawCmd { pos, depth, radius: r, color: *col });
        }
    }

    // ── Sort back-to-front (painter draws in order) ───────────────────────────

    bones.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap());
    joints.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap());

    // ── Draw bones ────────────────────────────────────────────────────────────

    for bone in &bones {
        // Shadow
        painter.line_segment(
            [bone.a + Vec2::new(1.5, 2.0), bone.b + Vec2::new(1.5, 2.0)],
            Stroke::new(5.0, Color32::from_black_alpha(60)),
        );
        painter.line_segment([bone.a, bone.b], Stroke::new(4.0, bone.color));
    }

    // ── Draw joints ───────────────────────────────────────────────────────────

    for j in &joints {
        // Shadow
        painter.circle_filled(j.pos + Vec2::new(1.5, 2.0), j.radius + 1.0,
            Color32::from_black_alpha(60));
        painter.circle_filled(j.pos, j.radius, j.color);
        // Rim highlight
        painter.circle_stroke(j.pos, j.radius,
            Stroke::new(1.5, Color32::from_rgba_premultiplied(255,255,255,80)));
        // Specular dot
        painter.circle_filled(
            j.pos + Vec2::new(-j.radius * 0.3, -j.radius * 0.35),
            j.radius * 0.35,
            Color32::from_rgba_premultiplied(255,255,255,160),
        );
    }

    // ── Controls hint ─────────────────────────────────────────────────────────

    let hint = if dragging_joint.is_some() {
        "Dragging joint..."
    } else {
        "Drag joint: move   Drag empty: rotate   Scroll: zoom"
    };
    painter.text(
        rect.min + Vec2::new(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        hint,
        egui::FontId::proportional(11.0),
        Color32::from_rgba_premultiplied(200, 200, 200, 120),
    );

    response
}

// ── Ground grid ───────────────────────────────────────────────────────────────

fn draw_grid(painter: &Painter, camera: &Camera3D, rect: Rect) {
    let grid_col = Color32::from_rgba_premultiplied(100, 120, 100, 50);
    let stroke   = Stroke::new(1.0, grid_col);
    let y = -0.05_f32; // ground level just below feet (feet at canvas y≈539 → world y≈0)

    for i in -5i32..=5 {
        let v = i as f32;
        // Lines along X
        let a = camera.project_to_rect([v, y, -5.0], rect);
        let b = camera.project_to_rect([v, y,  5.0], rect);
        if let (Some((pa, _)), Some((pb, _))) = (a, b) {
            painter.line_segment([pa, pb], stroke);
        }
        // Lines along Z
        let a = camera.project_to_rect([-5.0, y, v], rect);
        let b = camera.project_to_rect([ 5.0, y, v], rect);
        if let (Some((pa, _)), Some((pb, _))) = (a, b) {
            painter.line_segment([pa, pb], stroke);
        }
    }
}

// ── Joint manipulation helpers ────────────────────────────────────────────────

fn find_nearest_joint_3d(pose: &Pose, camera: &Camera3D, rect: Rect, screen_pos: Pos2) -> Option<String> {
    let joint_names = [
        ("head", &pose.head), ("left_shoulder", &pose.left_shoulder), ("right_shoulder", &pose.right_shoulder),
        ("left_elbow", &pose.left_elbow), ("right_elbow", &pose.right_elbow),
        ("left_wrist", &pose.left_wrist), ("right_wrist", &pose.right_wrist),
        ("hips", &pose.hips),
        ("left_knee", &pose.left_knee), ("right_knee", &pose.right_knee),
        ("left_ankle", &pose.left_ankle), ("right_ankle", &pose.right_ankle),
    ];
    
    joint_names.iter()
        .filter_map(|(name, joint)| {
            let world_pos = to_world(joint);
            if let Some((proj_pos, depth)) = camera.project_to_rect(world_pos, rect) {
                let dx = proj_pos.x - screen_pos.x;
                let dy = proj_pos.y - screen_pos.y;
                let screen_dist = (dx * dx + dy * dy).sqrt();
                if screen_dist < 25.0 { 
                    // Use a combined metric: screen distance + depth penalty
                    // This makes closer joints easier to select when overlapping
                    let selection_score = screen_dist + depth * 5.0;
                    Some((name, selection_score)) 
                } else { 
                    None 
                }
            } else {
                None
            }
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(name, _)| name.to_string())
}

fn update_joint_3d(pose: &mut Pose, joint_name: &str, camera: &Camera3D, rect: Rect, screen_pos: Pos2) {
    // Helper to constrain 3D distances
    let constrain_3d = |from: (f32, f32, f32), to: (f32, f32, f32), length: f32| -> (f32, f32, f32) {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let dz = to.2 - from.2;
        let dist = (dx*dx + dy*dy + dz*dz).sqrt();
        if dist < 0.1 { return (from.0 + length, from.1, from.2); }
        let s = length / dist;
        (from.0 + dx * s, from.1 + dy * s, from.2 + dz * s)
    };
    
    let set_xyz = |j: &mut Joint, (x, y, z): (f32, f32, f32)| {
        j.x = x; j.y = y; j.z = z;
    };
    
    // Get the joint and calculate target position in canvas space
    let target_canvas = match joint_name {
        "head" | "hips" | "left_shoulder" | "right_shoulder" | "left_elbow" | "right_elbow" |
        "left_wrist" | "right_wrist" | "left_knee" | "right_knee" | "left_ankle" | "right_ankle" => {
            let joint = match joint_name {
                "head" => &pose.head,
                "hips" => &pose.hips,
                "left_shoulder" => &pose.left_shoulder,
                "right_shoulder" => &pose.right_shoulder,
                "left_elbow" => &pose.left_elbow,
                "right_elbow" => &pose.right_elbow,
                "left_wrist" => &pose.left_wrist,
                "right_wrist" => &pose.right_wrist,
                "left_knee" => &pose.left_knee,
                "right_knee" => &pose.right_knee,
                "left_ankle" => &pose.left_ankle,
                "right_ankle" => &pose.right_ankle,
                _ => return,
            };
            
            let original_world = to_world(joint);
            if let Some(new_world) = unproject_screen_to_world(camera, rect, screen_pos, original_world) {
                (
                    new_world[0] * 150.0 + 400.0,
                    -(new_world[1] * 150.0 - 539.0),
                    new_world[2] * 150.0
                )
            } else {
                return;
            }
        }
        _ => return,
    };
    
    // Apply constraints based on which joint is being moved
    match joint_name {
        "head" => {
            let neck_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
            let neck_y = pose.left_shoulder.y.min(pose.right_shoulder.y) - 30.0;
            let neck_z = (pose.left_shoulder.z + pose.right_shoulder.z) / 2.0;
            let constrained = constrain_3d((neck_x, neck_y, neck_z), target_canvas, NECK_LEN);
            set_xyz(&mut pose.head, constrained);
        }
        "hips" => {
            let torso_x = (pose.left_shoulder.x + pose.right_shoulder.x) / 2.0;
            let torso_y = (pose.left_shoulder.y + pose.right_shoulder.y) / 2.0;
            let torso_z = (pose.left_shoulder.z + pose.right_shoulder.z) / 2.0;
            let constrained = constrain_3d((torso_x, torso_y, torso_z), target_canvas, TORSO_UPPER);
            set_xyz(&mut pose.hips, constrained);
        }
        "left_shoulder" => {
            set_xyz(&mut pose.left_shoulder, target_canvas);
            let el = (pose.left_elbow.x, pose.left_elbow.y, pose.left_elbow.z);
            let wr = (pose.left_wrist.x, pose.left_wrist.y, pose.left_wrist.z);
            let el2 = constrain_3d(target_canvas, el, UPPER_ARM);
            set_xyz(&mut pose.left_elbow, el2);
            set_xyz(&mut pose.left_wrist, constrain_3d(el2, wr, FOREARM));
        }
        "right_shoulder" => {
            set_xyz(&mut pose.right_shoulder, target_canvas);
            let el = (pose.right_elbow.x, pose.right_elbow.y, pose.right_elbow.z);
            let wr = (pose.right_wrist.x, pose.right_wrist.y, pose.right_wrist.z);
            let el2 = constrain_3d(target_canvas, el, UPPER_ARM);
            set_xyz(&mut pose.right_elbow, el2);
            set_xyz(&mut pose.right_wrist, constrain_3d(el2, wr, FOREARM));
        }
        "left_elbow" => {
            let sh = (pose.left_shoulder.x, pose.left_shoulder.y, pose.left_shoulder.z);
            let wr = (pose.left_wrist.x, pose.left_wrist.y, pose.left_wrist.z);
            let el2 = constrain_3d(sh, target_canvas, UPPER_ARM);
            set_xyz(&mut pose.left_elbow, el2);
            set_xyz(&mut pose.left_wrist, constrain_3d(el2, wr, FOREARM));
        }
        "right_elbow" => {
            let sh = (pose.right_shoulder.x, pose.right_shoulder.y, pose.right_shoulder.z);
            let wr = (pose.right_wrist.x, pose.right_wrist.y, pose.right_wrist.z);
            let el2 = constrain_3d(sh, target_canvas, UPPER_ARM);
            set_xyz(&mut pose.right_elbow, el2);
            set_xyz(&mut pose.right_wrist, constrain_3d(el2, wr, FOREARM));
        }
        "left_wrist" => {
            let el = (pose.left_elbow.x, pose.left_elbow.y, pose.left_elbow.z);
            set_xyz(&mut pose.left_wrist, constrain_3d(el, target_canvas, FOREARM));
        }
        "right_wrist" => {
            let el = (pose.right_elbow.x, pose.right_elbow.y, pose.right_elbow.z);
            set_xyz(&mut pose.right_wrist, constrain_3d(el, target_canvas, FOREARM));
        }
        "left_knee" => {
            let hip_x = pose.left_shoulder.x;
            let hip_y = pose.hips.y;
            let hip_z = pose.hips.z;
            let an = (pose.left_ankle.x, pose.left_ankle.y, pose.left_ankle.z);
            let kn2 = constrain_3d((hip_x, hip_y, hip_z), target_canvas, THIGH);
            set_xyz(&mut pose.left_knee, kn2);
            set_xyz(&mut pose.left_ankle, constrain_3d(kn2, an, SHIN));
        }
        "right_knee" => {
            let hip_x = pose.right_shoulder.x;
            let hip_y = pose.hips.y;
            let hip_z = pose.hips.z;
            let an = (pose.right_ankle.x, pose.right_ankle.y, pose.right_ankle.z);
            let kn2 = constrain_3d((hip_x, hip_y, hip_z), target_canvas, THIGH);
            set_xyz(&mut pose.right_knee, kn2);
            set_xyz(&mut pose.right_ankle, constrain_3d(kn2, an, SHIN));
        }
        "left_ankle" => {
            let kn = (pose.left_knee.x, pose.left_knee.y, pose.left_knee.z);
            set_xyz(&mut pose.left_ankle, constrain_3d(kn, target_canvas, SHIN));
        }
        "right_ankle" => {
            let kn = (pose.right_knee.x, pose.right_knee.y, pose.right_knee.z);
            set_xyz(&mut pose.right_ankle, constrain_3d(kn, target_canvas, SHIN));
        }
        _ => {}
    }
}

fn unproject_screen_to_world(camera: &Camera3D, rect: Rect, screen_pos: Pos2, original_world_pos: [f32; 3]) -> Option<[f32; 3]> {
    // Get camera basis vectors
    let eye = camera.eye();
    let (sy, cy) = camera.yaw.sin_cos();
    let (sp, cp) = camera.pitch.sin_cos();
    
    let fwd = [-cp * sy, -sp, -cp * cy];
    let right = [cy, 0.0, -sy];
    let up = [sp * sy, cp, sp * cy];
    
    // Calculate how far the original point is from camera
    let to_original = [
        original_world_pos[0] - eye[0],
        original_world_pos[1] - eye[1],
        original_world_pos[2] - eye[2],
    ];
    let distance_from_camera = (to_original[0]*to_original[0] + 
                                  to_original[1]*to_original[1] + 
                                  to_original[2]*to_original[2]).sqrt();
    
    // Convert screen position to normalized device coordinates
    let aspect = rect.width() / rect.height();
    let nx = (screen_pos.x - rect.center().x) / (rect.width() * 0.5);
    let ny = -(screen_pos.y - rect.center().y) / (rect.height() * 0.5);
    
    // Calculate world-space offset from camera center
    let half_h = (camera.fov * 0.5).tan();
    let half_w = half_h * aspect;
    
    let offset_right = nx * half_w * distance_from_camera;
    let offset_up = ny * half_h * distance_from_camera;
    
    // New position: move from eye, forward by distance, then offset by screen movement
    Some([
        eye[0] + fwd[0] * distance_from_camera + right[0] * offset_right + up[0] * offset_up,
        eye[1] + fwd[1] * distance_from_camera + right[1] * offset_right + up[1] * offset_up,
        eye[2] + fwd[2] * distance_from_camera + right[2] * offset_right + up[2] * offset_up,
    ])
}
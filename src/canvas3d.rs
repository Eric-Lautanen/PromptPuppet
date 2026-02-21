// canvas3d.rs
use egui::{Pos2, Vec2, Color32, Stroke, Rect, Ui, Response, Sense};
use crate::pose::{Pose, Joint};
use crate::skeleton::{self, Skeleton, color32};

#[derive(Clone, Debug)]
pub struct Camera3D { pub focus: [f32;3], pub yaw: f32, pub pitch: f32, pub radius: f32, pub scale: f32 }
impl Default for Camera3D {
    fn default() -> Self { Self { focus: [0.0;3], yaw: 0.0, pitch: 0.0, radius: 700.0, scale: 1.6 } }
}

impl Camera3D {
    fn eye(&self) -> [f32;3] {
        let ((sy,cy),(sp,cp)) = (self.yaw.sin_cos(), self.pitch.sin_cos());
        [self.focus[0]+self.radius*cp*sy, self.focus[1]+self.radius*sp, self.focus[2]+self.radius*cp*cy]
    }

    fn project(&self, p: [f32;3], r: Rect) -> Option<(Pos2,f32)> {
        let eye = self.eye();
        let ((sy,cy),(sp,cp)) = (self.yaw.sin_cos(), self.pitch.sin_cos());
        let (fwd,right,up) = ([-cp*sy,-sp,-cp*cy],[cy,0.,-sy],[sp*sy,cp,sp*cy]);
        let d = [p[0]-eye[0],p[1]-eye[1],p[2]-eye[2]];
        let z = d[0]*fwd[0]+d[1]*fwd[1]+d[2]*fwd[2];
        if z < 0.01 { return None; }
        let (x,y) = (d[0]*right[0]+d[1]*right[1]+d[2]*right[2], d[0]*up[0]+d[1]*up[1]+d[2]*up[2]);
        // Orthographic projection: direct scale without perspective division
        Some((Pos2::new(r.center().x + x * self.scale, r.center().y + y * self.scale), z))
    }
}

fn world(j: &Joint) -> [f32;3] { [j.x, j.y, j.z] }

fn get<'a>(pose: &'a Pose, name: &str) -> Option<&'a Joint> {
    Some(match name {
        "head"           => &pose.head,          "neck"           => &pose.neck,
        "left_shoulder"  => &pose.left_shoulder, "right_shoulder" => &pose.right_shoulder,
        "left_elbow"     => &pose.left_elbow,    "right_elbow"    => &pose.right_elbow,
        "left_wrist"     => &pose.left_wrist,    "right_wrist"    => &pose.right_wrist,
        "waist"          => &pose.waist,          "crotch"         => &pose.crotch,
        "left_knee"      => &pose.left_knee,      "right_knee"     => &pose.right_knee,
        "left_ankle"     => &pose.left_ankle,     "right_ankle"    => &pose.right_ankle,
        _ => return None,
    })
}

pub fn draw_3d_canvas(ui: &mut Ui, pose: &mut Pose, cam: &mut Camera3D, size: Vec2, drag: &mut Option<String>, status: Option<(&str, f32)>, disco_time: Option<f32>) -> Response {
    let sk = skeleton::get();
    let (resp,p) = ui.allocate_painter(size, Sense::click_and_drag());

    // ── Disco helpers ─────────────────────────────────────────────────────────
    // hsv_to_rgb: h/s/v all in [0,1]
    let hsv = |h: f32, s: f32, v: f32| -> Color32 {
        let h6 = h.rem_euclid(1.0) * 6.0;
        let i  = h6 as u32;
        let f  = h6 - i as f32;
        let (pp, q, t) = (v*(1.-s), v*(1.-s*f), v*(1.-s*(1.-f)));
        let (r,g,b) = match i { 0=>(v,t,pp), 1=>(q,v,pp), 2=>(pp,v,t), 3=>(pp,q,v), 4=>(t,pp,v), _=>(v,pp,q) };
        Color32::from_rgb((r*255.) as u8, (g*255.) as u8, (b*255.) as u8)
    };

    // Background: dark base with a slowly pulsing hue tint in disco mode
    let bg = if let Some(dt) = disco_time {
        let pulse = (dt * 0.4).sin() * 0.5 + 0.5;           // 0..1 slow breathe
        let hue   = (dt * 0.12).rem_euclid(1.0);              // full hue rotation ~8s
        let dark  = hsv(hue, 0.6, 0.07 + pulse * 0.04);      // very dark, hint of colour
        dark
    } else if ui.visuals().dark_mode { Color32::from_gray(18) } else { Color32::from_gray(80) };

    p.rect_filled(resp.rect, 0.0, bg);

    // Calculate current figure bounds
    let all = [&pose.head,&pose.neck,&pose.left_shoulder,&pose.right_shoulder,
               &pose.left_elbow,&pose.right_elbow,&pose.left_wrist,&pose.right_wrist,
               &pose.waist,&pose.crotch,&pose.left_knee,&pose.right_knee,
               &pose.left_ankle,&pose.right_ankle];
    let (min_x,max_x) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.x),hi.max(j.x)));
    let (min_y,max_y) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.y),hi.max(j.y)));
    let (min_z,max_z) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.z),hi.max(j.z)));
    let target_focus = [(min_x+max_x)/2.0, (min_y+max_y)/2.0, (min_z+max_z)/2.0];
    let feet_y = pose.left_ankle.y.max(pose.right_ankle.y);

    // X/Z: snap to figure center during rotation so it stays the horizontal orbit pivot.
    // Y: creep very slowly (0.03/frame) — effectively frozen during any rotation gesture.
    //    This is the three.js OrbitControls insight: a stable vertical pivot makes the
    //    grid appear as genuinely static world geometry rather than swimming with pitch.
    let is_first_frame = cam.focus[0].abs() < 0.001 && cam.focus[1].abs() < 0.001 && cam.focus[2].abs() < 0.001;
    let is_rotating = resp.dragged() && drag.is_none();
    let lerp_xz = if is_first_frame || is_rotating { 1.0 } else if drag.is_some() { 0.15 } else { 0.25 };
    let lerp_y  = if is_first_frame { 1.0 } else { 0.03 }; // near-frozen during rotation
    cam.focus[0] += (target_focus[0] - cam.focus[0]) * lerp_xz;
    cam.focus[1] += (target_focus[1] - cam.focus[1]) * lerp_y;
    cam.focus[2] += (target_focus[2] - cam.focus[2]) * lerp_xz;

    // View preset buttons
    let button_area = draw_view_buttons(ui, cam, resp.rect);

    // Capture joint on raw pointer press — before egui's drag threshold displaces the position.
    // drag_started() fires too late: the pointer has already moved and we miss small joints.
    let just_pressed = resp.hovered() && ui.input(|i| i.pointer.primary_pressed());
    if just_pressed {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if !button_area.contains(pos) {
                *drag = find_nearest(pose, &sk, cam, resp.rect, pos);
                // drag == None means empty space → rotation mode
            }
        }
    }
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if button_area.contains(pos) { *drag = None; }
        }
        if let Some(_pos) = resp.interact_pointer_pos() {
            match drag.as_ref() {
                Some(name) => move_joint(pose, name, &sk, cam, resp.drag_delta()),
                None => cam.yaw -= resp.drag_delta().x * 0.008,
            }
        }
    }
    if resp.drag_stopped() {
        *drag = None;
    }
    
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        if s != 0.0 { cam.scale *= 1.0 + s*0.001; cam.scale = cam.scale.clamp(0.1, 10.0); }
    }

    // Draw XZ ground grid at floor level (feet_y already computed above)
    let grid_y = feet_y + 10.0;
    let grid_size = 600.0;
    let grid_step = 60.0;
    let center_x = cam.focus[0];
    let center_z = cam.focus[2];

    // ── Disco spotlights: coloured circles rotating on the floor ─────────────
    if let Some(dt) = disco_time {
        let spot_radius = 110.0_f32;
        for i in 0..3_u32 {
            let angle = dt * 1.1 + (i as f32) * std::f32::consts::TAU / 3.0;
            let sx = center_x + angle.sin() * 160.0;
            let sz = center_z + angle.cos() * 160.0;
            let hue  = (dt * 0.18 + i as f32 / 3.0).rem_euclid(1.0);
            // Project centre + four rim points, average to get an on-screen ellipse.
            if let Some((sp, _)) = cam.project([sx, grid_y, sz], resp.rect) {
                // Quick: project a rim point to estimate screen radius
                let rim = cam.project([sx + spot_radius * 0.5, grid_y, sz], resp.rect);
                let sr = rim.map(|(rp,_)| (rp - sp).length()).unwrap_or(40.0) * 1.4;
                let col = hsv(hue, 0.9, 0.9);
                let c = Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 28);
                p.circle_filled(sp, sr, c);
                // Bright centre
                let cc = Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 55);
                p.circle_filled(sp, sr * 0.35, cc);
            }
        }
    }

    // ── Grid lines (rainbow in disco mode, plain otherwise) ──────────────────
    let plain_grid = if ui.visuals().dark_mode { Color32::from_gray(60) } else { Color32::from_gray(100) };

    let mut line_idx = 0_u32;
    let mut x = center_x - grid_size;
    while x <= center_x + grid_size {
        let gc = if let Some(dt) = disco_time {
            let hue = ((x - center_x) / (grid_size * 2.0) + dt * 0.08).rem_euclid(1.0);
            let beat_flash = ((dt * 140.0 / 60.0 * std::f32::consts::TAU).sin() * 0.5 + 0.5) * 0.35;
            let v = 0.30 + beat_flash;
            let c = hsv(hue, 0.85, v);
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180)
        } else { plain_grid };
        let p1 = cam.project([x, grid_y, center_z - grid_size], resp.rect);
        let p2 = cam.project([x, grid_y, center_z + grid_size], resp.rect);
        if let (Some((p1, _)), Some((p2, _))) = (p1, p2) {
            p.line_segment([p1, p2], Stroke::new(1.5, gc));
        }
        x += grid_step;
        line_idx += 1;
    }
    let mut z = center_z - grid_size;
    while z <= center_z + grid_size {
        let gc = if let Some(dt) = disco_time {
            let hue = ((z - center_z) / (grid_size * 2.0) + dt * 0.08 + 0.5).rem_euclid(1.0);
            let beat_flash = ((dt * 140.0 / 60.0 * std::f32::consts::TAU).sin() * 0.5 + 0.5) * 0.35;
            let v = 0.30 + beat_flash;
            let c = hsv(hue, 0.85, v);
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180)
        } else { plain_grid };
        let p1 = cam.project([center_x - grid_size, grid_y, z], resp.rect);
        let p2 = cam.project([center_x + grid_size, grid_y, z], resp.rect);
        if let (Some((p1, _)), Some((p2, _))) = (p1, p2) {
            p.line_segment([p1, p2], Stroke::new(1.5, gc));
        }
        z += grid_step;
        line_idx += 1;
    }
    let _ = line_idx; // suppress unused warning

    // Determine which joint is under cursor for hover highlight
    let hovered_joint: Option<String> = if drag.is_some() {
        drag.clone()
    } else {
        ui.input(|i| i.pointer.hover_pos())
            .filter(|pos| resp.rect.contains(*pos) && !button_area.contains(*pos))
            .and_then(|pos| find_nearest(pose, &sk, cam, resp.rect, pos))
    };

    struct Draw { a:Pos2, b:Pos2, z:f32, c:Color32, is_j:bool, r:f32, hovered:bool }
    let mut draws: Vec<Draw> = Vec::new();

    for bone in &sk.bones {
        if let (Some(ja),Some(jb)) = (get(pose,&bone.a),get(pose,&bone.b)) {
            if let (Some((pa,za)),Some((pb,zb))) = (cam.project(world(ja),resp.rect),cam.project(world(jb),resp.rect)) {
                let c = if let Some(dt) = disco_time {
                    // Each bone gets its own hue offset so the skeleton is fully rainbow
                    let bone_hash = bone.a.len() as f32 * 0.07 + bone.b.len() as f32 * 0.13;
                    let hue = (dt * 0.22 + bone_hash).rem_euclid(1.0);
                    hsv(hue, 1.0, 1.0)
                } else { color32(bone.color) };
                draws.push(Draw{a:pa,b:pb,z:(za+zb)*0.5,c,is_j:false,r:0.0,hovered:false});
            }
        }
    }
    for jd in &sk.joints {
        if let Some(j) = get(pose,&jd.name) {
            if let Some((pos,z)) = cam.project(world(j),resp.rect) {
                let is_hov = hovered_joint.as_deref() == Some(jd.name.as_str());
                let c = if let Some(dt) = disco_time {
                    let joint_hash = jd.name.len() as f32 * 0.11;
                    let hue = (dt * 0.3 + joint_hash).rem_euclid(1.0);
                    hsv(hue, 0.8, 1.0)
                } else { color32(jd.color) };
                draws.push(Draw{a:pos,b:pos,z,c,is_j:true,r:jd.radius*1.5,hovered:is_hov});
            }
        }
    }
    draws.sort_by(|a,b| b.z.partial_cmp(&a.z).unwrap());
    for d in draws {
        if d.is_j {
            if d.hovered {
                p.circle_filled(d.a, d.r + 7.0, Color32::from_rgba_premultiplied(255,255,255,25));
                p.circle_stroke(d.a, d.r + 5.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(255,255,255,170)));
            }
            // In disco mode joints pulse in size with the beat
            let r = if let Some(dt) = disco_time {
                let pulse = (dt * 140.0 / 60.0 * std::f32::consts::TAU * 2.0).sin() * 0.22 + 1.0;
                d.r * pulse
            } else { d.r };
            p.circle_filled(d.a+Vec2::new(1.5,2.0), r+1.0, Color32::from_black_alpha(60));
            p.circle_filled(d.a, r, d.c);
            let rim_w = if d.hovered { 2.5 } else { 1.5 };
            let rim_a = if d.hovered { 220 } else { 80 };
            p.circle_stroke(d.a, r, Stroke::new(rim_w, Color32::from_rgba_premultiplied(255,255,255,rim_a)));
            p.circle_filled(d.a+Vec2::new(-r*0.3,-r*0.35), r*0.35, Color32::from_rgba_premultiplied(255,255,255,160));
        } else {
            let stroke_w = if let Some(dt) = disco_time {
                // Bones throb on the beat
                let pulse = (dt * 140.0 / 60.0 * std::f32::consts::TAU).sin() * 1.5 + 4.0;
                pulse
            } else { 4.0 };
            p.line_segment([d.a+Vec2::new(1.5,2.0),d.b+Vec2::new(1.5,2.0)], Stroke::new(stroke_w+1.0,Color32::from_black_alpha(60)));
            p.line_segment([d.a,d.b], Stroke::new(stroke_w, d.c));
        }
    }

    // ── Disco sparkles: tiny flashing stars scattered around the figure ───────
    if let Some(dt) = disco_time {
        // 18 sparkles; each gets a new random-ish position every ~0.1s (floor of t*10)
        let tick = (dt * 10.0).floor() as u32;
        for i in 0_u32..18 {
            // Cheap deterministic hash → pseudo-random position + hue
            let seed = tick.wrapping_mul(2654435761).wrapping_add(i.wrapping_mul(2246822519));
            let nx = ((seed & 0xFFFF) as f32 / 65535.0) - 0.5;   // -0.5..0.5
            let ny = ((seed >> 16) as f32 / 65535.0) - 0.5;
            let hue  = ((seed.wrapping_mul(1013904223) >> 8) as f32 / 16777215.0).rem_euclid(1.0);
            let size = 2.5 + (seed & 7) as f32 * 0.6;

            let sx = resp.rect.center().x + nx * resp.rect.width()  * 0.85;
            let sy = resp.rect.center().y + ny * resp.rect.height() * 0.7;
            let sc = hsv(hue, 0.9, 1.0);
            p.circle_filled(Pos2::new(sx, sy), size, sc);
            // Little 4-point star cross
            let arm = size * 1.8;
            p.line_segment([Pos2::new(sx-arm,sy), Pos2::new(sx+arm,sy)], Stroke::new(1.0, sc));
            p.line_segment([Pos2::new(sx,sy-arm), Pos2::new(sx,sy+arm)], Stroke::new(1.0, sc));
        }
    }
    p.text(resp.rect.min+Vec2::new(8.,6.), egui::Align2::LEFT_TOP,
        if drag.is_some() {"Dragging joint..."} else {"Drag joint: move   Drag empty: rotate   Scroll: zoom"},
        egui::FontId::proportional(11.0), Color32::from_rgba_premultiplied(200,200,200,120));

    // ── Status toast (upper-right corner) ────────────────────────────────────
    if let Some((msg, alpha)) = status {
        if alpha > 0.0 {
            let a = (alpha * 255.0).round() as u8;
            let pad = Vec2::new(12.0, 8.0);
            let font = egui::FontId::proportional(13.0);
            let galley = ui.painter().layout_no_wrap(
                msg.to_string(), font.clone(), Color32::WHITE);
            let text_size = galley.size();
            let bg_size   = text_size + pad * 2.0;
            let bg_pos    = egui::Pos2::new(
                resp.rect.max.x - bg_size.x - 10.0,
                resp.rect.min.y + 10.0,
            );
            let bg_rect = egui::Rect::from_min_size(bg_pos, bg_size);
            p.rect_filled(bg_rect, 6.0,
                Color32::from_rgba_premultiplied(20, 20, 20, (a as f32 * 0.82) as u8));
            p.rect_stroke(bg_rect, 6.0,
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, (a as f32 * 0.18) as u8)),
                egui::StrokeKind::Outside);
            p.text(bg_pos + pad, egui::Align2::LEFT_TOP,
                msg, font, Color32::from_rgba_premultiplied(255, 255, 255, a));
        }
    }

    resp
}

fn draw_view_buttons(ui: &mut Ui, cam: &mut Camera3D, rect: Rect) -> Rect {
    let btn_size = Vec2::new(54.0, 28.0);
    let spacing = 6.0;
    let pad = 12.0;
    
    let views = [
        ("Front", 0.0, 0.0, Color32::from_rgb(100, 180, 255)),
        ("Back", std::f32::consts::PI, 0.0, Color32::from_rgb(0, 200, 220)),
        ("Left", -std::f32::consts::FRAC_PI_2, 0.0, Color32::from_rgb(255, 160, 0)),
        ("Right", std::f32::consts::FRAC_PI_2, 0.0, Color32::from_rgb(80, 200, 80)),
    ];
    
    let total_width = (btn_size.x + spacing) * views.len() as f32 - spacing;
    let start_x = rect.center().x - total_width / 2.0;
    let y = rect.min.y + pad;
    
    let button_area = Rect::from_min_size(
        Pos2::new(start_x - spacing, y - spacing),
        Vec2::new(total_width + spacing * 2.0, btn_size.y + spacing * 2.0)
    );
    
    for (i, (label, yaw, pitch, color)) in views.iter().enumerate() {
        let btn_pos = Pos2::new(start_x + (btn_size.x + spacing) * i as f32, y);
        let btn_rect = Rect::from_min_size(btn_pos, btn_size);
        
        let hovered = ui.rect_contains_pointer(btn_rect);
        let clicked = hovered && ui.input(|i| i.pointer.primary_clicked());
        
        let is_active = (cam.yaw - yaw).abs() < 0.1 && (cam.pitch - pitch).abs() < 0.1;
        
        if clicked {
            cam.yaw = *yaw;
            cam.pitch = *pitch;
        }
        
        let (opacity_mult, border_alpha, shadow_alpha) = if is_active {
            (0.55, 200, 80)
        } else if hovered {
            (0.4, 140, 60)
        } else {
            (0.25, 90, 40)
        };
        
        let bg = color.linear_multiply(opacity_mult);
        let border = Color32::from_rgba_premultiplied(
            ((color.r() as u16 + 155) / 2).min(255) as u8,
            ((color.g() as u16 + 155) / 2).min(255) as u8,
            ((color.b() as u16 + 155) / 2).min(255) as u8,
            border_alpha
        );
        
        let painter = ui.painter();
        painter.rect_filled(btn_rect.translate(Vec2::new(1.5, 2.0)), 5.0, Color32::from_black_alpha(shadow_alpha));
        painter.rect_filled(btn_rect, 5.0, bg);
        
        let stroke_width = if is_active { 2.0 } else { 1.5 };
        painter.rect_stroke(btn_rect, 5.0, Stroke::new(stroke_width, border), egui::StrokeKind::Outside);
        
        if is_active {
            let inner_rect = btn_rect.shrink(3.0);
            painter.rect_stroke(inner_rect, 3.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 120)), egui::StrokeKind::Inside);
        }
        
        let text_color = Color32::from_rgba_premultiplied(255, 255, 255, if is_active { 240 } else { 180 });
        painter.text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(if is_active { 12.5 } else { 12.0 }),
            text_color
        );
    }
    
    button_area
}

fn find_nearest(pose: &Pose, sk: &Skeleton, cam: &Camera3D, r: Rect, pos: Pos2) -> Option<String> {
    // Hit radius scales with zoom so joints are equally clickable when zoomed out.
    // Minimum 14px so tiny/distant joints are still reachable.
    let zoom_scale = cam.scale.clamp(0.5, 3.0);
    let mut candidates: Vec<(String, f32, f32)> = sk.joints.iter()
        .filter_map(|jd| {
            let (sp, z) = cam.project(world(get(pose,&jd.name)?),r)?;
            let dist = sp.distance(pos);
            let hit_radius = (jd.radius * 1.5 * zoom_scale + 6.0).max(14.0);
            (dist < hit_radius).then_some((jd.name.clone(), dist, z))
        })
        .collect();
    
    // Closer joints (lower z) take priority; break ties by 2D distance
    candidates.sort_by(|a, b| {
        a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    
    candidates.first().map(|(name, _, _)| name.clone())
}

fn move_joint(pose: &mut Pose, name: &str, sk: &Skeleton, cam: &Camera3D, delta: Vec2) {
    let Some(j_ref) = get(pose, name) else { return };

    // Delta-based movement: convert the tiny per-frame screen delta into a world nudge.
    // This is fundamentally smoother than absolute-position tracking because:
    //   - drag_delta() is 1-5px per frame — noise is tiny and integrates smoothly
    //   - no depth-estimation error (absolute approach must guess joint Z each frame)
    //   - FABRIK receives a position very close to the current one, so it barely has
    //     to move and converges in 1-2 iterations instead of fighting a noisy target
    let ((sy,cy),(sp,cp)) = (cam.yaw.sin_cos(), cam.pitch.sin_cos());
    let right = [cy,    0.,  -sy];
    let up    = [sp*sy, cp, sp*cy];
    let scale = cam.scale;

    // Map screen pixels → world units using camera right/up axes
    let wx = right[0]*delta.x/scale + up[0]*delta.y/scale;
    let wy = right[1]*delta.x/scale + up[1]*delta.y/scale;
    let wz = right[2]*delta.x/scale + up[2]*delta.y/scale;

    let cur = world(j_ref);
    let target = (cur[0]+wx, cur[1]+wy, cur[2]+wz);

    pose.move_joint(name, target, sk);
}
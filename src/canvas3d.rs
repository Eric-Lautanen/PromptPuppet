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

pub fn draw_3d_canvas(ui: &mut Ui, pose: &mut Pose, cam: &mut Camera3D, size: Vec2, drag: &mut Option<String>) -> Response {
    let sk = skeleton::get();
    let (resp,p) = ui.allocate_painter(size, Sense::click_and_drag());
    p.rect_filled(resp.rect, 0.0, if ui.visuals().dark_mode { Color32::from_gray(18) } else { Color32::from_gray(80) });

    // Calculate current figure center
    let all = [&pose.head,&pose.neck,&pose.left_shoulder,&pose.right_shoulder,
               &pose.left_elbow,&pose.right_elbow,&pose.left_wrist,&pose.right_wrist,
               &pose.waist,&pose.crotch,&pose.left_knee,&pose.right_knee,
               &pose.left_ankle,&pose.right_ankle];
    let (min_x,max_x) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.x),hi.max(j.x)));
    let (min_y,max_y) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.y),hi.max(j.y)));
    let (min_z,max_z) = all.iter().fold((f32::MAX,f32::MIN),|(lo,hi),j|(lo.min(j.z),hi.max(j.z)));
    let target_focus = [(min_x+max_x)/2.0, (min_y+max_y)/2.0, (min_z+max_z)/2.0];
    
    // First frame: snap to center immediately. After: smoothly interpolate
    let is_first_frame = cam.focus[0].abs() < 0.001 && cam.focus[1].abs() < 0.001 && cam.focus[2].abs() < 0.001;
    if is_first_frame {
        cam.focus = target_focus;
    } else {
        let lerp_speed = if drag.is_some() { 0.15 } else { 0.25 };
        cam.focus[0] += (target_focus[0] - cam.focus[0]) * lerp_speed;
        cam.focus[1] += (target_focus[1] - cam.focus[1]) * lerp_speed;
        cam.focus[2] += (target_focus[2] - cam.focus[2]) * lerp_speed;
    }

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
        if let Some(pos) = resp.interact_pointer_pos() {
            match drag.as_ref() {
                Some(name) => move_joint(pose, name, &sk, cam, resp.rect, pos),
                None       => cam.yaw -= resp.drag_delta().x * 0.008,
            }
        }
    }
    if resp.drag_stopped() { 
        if let Some(joint_name) = drag.as_ref() {
            // Apply anatomical constraints only to the chain that was moved
            match joint_name.as_str() {
                "left_elbow" | "left_wrist" => {
                    let mut chain = [pose.left_shoulder.xyz(), pose.left_elbow.xyz(), pose.left_wrist.xyz()];
                    Pose::constrain_elbow(&mut chain, &sk.constraints.elbow);
                    pose.left_elbow.set_xyz(chain[1]);
                    pose.left_wrist.set_xyz(chain[2]);
                }
                "right_elbow" | "right_wrist" => {
                    let mut chain = [pose.right_shoulder.xyz(), pose.right_elbow.xyz(), pose.right_wrist.xyz()];
                    Pose::constrain_elbow(&mut chain, &sk.constraints.elbow);
                    pose.right_elbow.set_xyz(chain[1]);
                    pose.right_wrist.set_xyz(chain[2]);
                }
                "left_knee" | "left_ankle" => {
                    let mut chain = [pose.crotch.xyz(), pose.left_knee.xyz(), pose.left_ankle.xyz()];
                    Pose::constrain_knee(&mut chain, &sk.constraints.knee);
                    pose.left_knee.set_xyz(chain[1]);
                    pose.left_ankle.set_xyz(chain[2]);
                }
                "right_knee" | "right_ankle" => {
                    let mut chain = [pose.crotch.xyz(), pose.right_knee.xyz(), pose.right_ankle.xyz()];
                    Pose::constrain_knee(&mut chain, &sk.constraints.knee);
                    pose.right_knee.set_xyz(chain[1]);
                    pose.right_ankle.set_xyz(chain[2]);
                }
                _ => {}
            }
        }
        
        *drag = None;
    }
    
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        if s != 0.0 { cam.scale *= 1.0 + s*0.001; cam.scale = cam.scale.clamp(0.1, 10.0); }
    }

    // Draw XZ ground grid below feet, centered on figure
    let feet_y = pose.left_ankle.y.max(pose.right_ankle.y);
    let grid_y = feet_y + 10.0;
    let grid_color = if ui.visuals().dark_mode { Color32::from_gray(60) } else { Color32::from_gray(100) };
    let grid_size = 300.0;
    let grid_step = 30.0;
    
    // Grid centered at figure's XZ position
    let center_x = cam.focus[0];
    let center_z = cam.focus[2];
    
    let mut x = center_x - grid_size;
    while x <= center_x + grid_size {
        let p1 = cam.project([x, grid_y, center_z - grid_size], resp.rect);
        let p2 = cam.project([x, grid_y, center_z + grid_size], resp.rect);
        if let (Some((p1, _)), Some((p2, _))) = (p1, p2) {
            p.line_segment([p1, p2], Stroke::new(1.5, grid_color));
        }
        x += grid_step;
    }
    let mut z = center_z - grid_size;
    while z <= center_z + grid_size {
        let p1 = cam.project([center_x - grid_size, grid_y, z], resp.rect);
        let p2 = cam.project([center_x + grid_size, grid_y, z], resp.rect);
        if let (Some((p1, _)), Some((p2, _))) = (p1, p2) {
            p.line_segment([p1, p2], Stroke::new(1.5, grid_color));
        }
        z += grid_step;
    }

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
                draws.push(Draw{a:pa,b:pb,z:(za+zb)*0.5,c:color32(bone.color),is_j:false,r:0.0,hovered:false});
            }
        }
    }
    for jd in &sk.joints {
        if let Some(j) = get(pose,&jd.name) {
            if let Some((pos,z)) = cam.project(world(j),resp.rect) {
                let is_hov = hovered_joint.as_deref() == Some(jd.name.as_str());
                draws.push(Draw{a:pos,b:pos,z,c:color32(jd.color),is_j:true,r:jd.radius*1.5,hovered:is_hov});
            }
        }
    }
    draws.sort_by(|a,b| b.z.partial_cmp(&a.z).unwrap());
    for d in draws {
        if d.is_j {
            if d.hovered {
                // Glow ring — shows this joint is grabbable
                p.circle_filled(d.a, d.r + 7.0, Color32::from_rgba_premultiplied(255,255,255,25));
                p.circle_stroke(d.a, d.r + 5.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(255,255,255,170)));
            }
            p.circle_filled(d.a+Vec2::new(1.5,2.0), d.r+1.0, Color32::from_black_alpha(60));
            p.circle_filled(d.a, d.r, d.c);
            let rim_w = if d.hovered { 2.5 } else { 1.5 };
            let rim_a = if d.hovered { 220 } else { 80 };
            p.circle_stroke(d.a, d.r, Stroke::new(rim_w, Color32::from_rgba_premultiplied(255,255,255,rim_a)));
            p.circle_filled(d.a+Vec2::new(-d.r*0.3,-d.r*0.35), d.r*0.35, Color32::from_rgba_premultiplied(255,255,255,160));
        } else {
            p.line_segment([d.a+Vec2::new(1.5,2.0),d.b+Vec2::new(1.5,2.0)], Stroke::new(5.0,Color32::from_black_alpha(60)));
            p.line_segment([d.a,d.b], Stroke::new(4.0,d.c));
        }
    }
    p.text(resp.rect.min+Vec2::new(8.,6.), egui::Align2::LEFT_TOP,
        if drag.is_some() {"Dragging joint..."} else {"Drag joint: move   Drag empty: rotate   Scroll: zoom"},
        egui::FontId::proportional(11.0), Color32::from_rgba_premultiplied(200,200,200,120));
    resp
}

fn draw_view_buttons(ui: &mut Ui, cam: &mut Camera3D, rect: Rect) -> Rect {
    let btn_size = Vec2::new(54.0, 28.0);
    let spacing = 6.0;
    let pad = 12.0;
    
    let views = [
        ("Front", 0.0, 0.0, Color32::from_rgb(100, 180, 255)),
        ("Back", std::f32::consts::PI, 0.0, Color32::from_rgb(0, 200, 220)),
        ("Right", -std::f32::consts::FRAC_PI_2, 0.0, Color32::from_rgb(255, 160, 0)),
        ("Left", std::f32::consts::FRAC_PI_2, 0.0, Color32::from_rgb(80, 200, 80)),
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

fn move_joint(pose: &mut Pose, name: &str, sk: &Skeleton, cam: &Camera3D, r: Rect, pos: Pos2) {
    let Some(j_ref) = get(pose, name) else { return };
    
    // Get camera basis vectors
    let eye = cam.eye();
    let ((sy,cy),(sp,cp)) = (cam.yaw.sin_cos(),cam.pitch.sin_cos());
    let (fwd,right,up) = ([-cp*sy,-sp,-cp*cy],[cy,0.,-sy],[sp*sy,cp,sp*cy]);
    
    // Find the depth of the joint along the forward axis
    let orig = world(j_ref);
    let to_joint = [orig[0]-eye[0], orig[1]-eye[1], orig[2]-eye[2]];
    let depth = to_joint[0]*fwd[0] + to_joint[1]*fwd[1] + to_joint[2]*fwd[2];
    
    // Clamp depth to reasonable range to prevent joints from going behind camera or too far
    let depth = depth.clamp(10.0, 10000.0);
    
    // Convert screen position to world ray direction
    let screen_offset = ((pos.x - r.center().x) / cam.scale, (pos.y - r.center().y) / cam.scale);
    
    // Calculate world position: start from eye, go forward by depth, then add screen offset in right/up directions
    let target = (
        eye[0] + fwd[0]*depth + right[0]*screen_offset.0 + up[0]*screen_offset.1,
        eye[1] + fwd[1]*depth + right[1]*screen_offset.0 + up[1]*screen_offset.1,
        eye[2] + fwd[2]*depth + right[2]*screen_offset.0 + up[2]*screen_offset.1,
    );

    // Use centralized constraint-aware movement
    pose.move_joint_constrained(name, target, sk);
}
// ftlz.rs — 🕺 For The Lulz.  Ctrl+Shift+D.  You didn't see anything.
//
// Axes (pose.rs convention):  X = left→right,  Y = bottom→top (up = positive),  Z = toward viewer.
// All offsets are relative to `base` (the rest pose), so the animation is
// scale-independent and doesn't care where the default pose sits in world space.

use crate::pose::Pose;
use std::f32::consts::TAU;

pub fn apply_dance(pose: &mut Pose, base: &Pose, t: f32) {

    // ── Timing ────────────────────────────────────────────────────────────────
    let bpm    = 140.0_f32;
    let beat   = t * bpm / 60.0;      // beats elapsed (floats up continuously)
    let p1     = beat * TAU;          // 1× per beat   (main groove)
    let p2     = p1  * 2.0;          // 2× per beat   (faster shimmy)
    let ph     = p1  * 0.5;          // ½× per beat   (slow sway, every 2 beats)

    let sin = |x: f32| x.sin();
    let cos = |x: f32| x.cos();

    // ── Whole-body bounce — 2× per beat, up is positive ──────────────────────
    let bounce = sin(p2) * 5.5;       // gentle up/down bob

    // ── Head ─────────────────────────────────────────────────────────────────
    // Hard nod on the beat, look around every 2 beats, slight roll
    pose.head_nod  =  sin(p1)  * 16.0;
    pose.head_yaw  =  cos(ph)  * 12.0;
    pose.head_tilt =  sin(ph + 0.8) * 10.0;
    pose.neck.x    =  base.neck.x + sin(ph) * 3.5;
    pose.neck.y    =  base.neck.y + bounce  * 0.7;
    pose.neck.z    =  base.neck.z;
    pose.head.x    =  base.head.x + sin(ph) * 5.0;
    pose.head.y    =  base.head.y + bounce  * 0.9;
    pose.head.z    =  base.head.z;

    // ── Shoulders — alternating shrug, opposite phase each side ──────────────
    let shrug = sin(p1) * 9.0;
    pose.left_shoulder.x  = base.left_shoulder.x  + sin(ph) * 2.5;
    pose.left_shoulder.y  = base.left_shoulder.y  +  shrug + bounce * 0.55;
    pose.left_shoulder.z  = base.left_shoulder.z;
    pose.right_shoulder.x = base.right_shoulder.x + sin(ph) * 2.5;
    pose.right_shoulder.y = base.right_shoulder.y  - shrug + bounce * 0.55;
    pose.right_shoulder.z = base.right_shoulder.z;

    // ── Arms: "raise the roof" with a pointing flourish ──────────────────────
    // Left arm swings forward/back while right does the opposite.
    // Every 4 beats the arms "point" — a big upward lunge.
    let point_phase = (beat % 4.0) / 4.0;   // 0..1 over 4 beats
    // Smooth point spike: rises quickly, holds briefly, drops — triangle-ish
    let point_spike = if point_phase < 0.12 {
        (point_phase / 0.12) * (point_phase / 0.12)   // quick ramp up
    } else if point_phase < 0.28 {
        1.0                                            // hold
    } else if point_phase < 0.45 {
        1.0 - (point_phase - 0.28) / 0.17             // ramp down
    } else {
        0.0                                            // rest of bar
    };

    // Base arm swing (alternating, 1× per beat)
    let arm_x  = sin(p1) * 30.0;
    let arm_y  = cos(p1) * 20.0;
    let roof   = sin(p2)  * 12.0;                  // fast shimmy up/down
    let pt_up  = point_spike * 35.0;               // point lunge

    pose.left_elbow.x  = base.left_elbow.x  - arm_x  - 10.0;
    pose.left_elbow.y  = base.left_elbow.y  + arm_y  + roof + pt_up + bounce * 0.4;
    pose.left_elbow.z  = base.left_elbow.z  - point_spike * 8.0;  // elbow back when pointing up

    pose.right_elbow.x = base.right_elbow.x + arm_x  + 10.0;
    pose.right_elbow.y = base.right_elbow.y  - arm_y + roof + pt_up + bounce * 0.4;
    pose.right_elbow.z = base.right_elbow.z  - point_spike * 8.0;

    // Wrists trail behind elbows by ~0.35 rad — naturally floppy
    let lag  = 0.35_f32;
    let wx   = sin(p1 - lag) * 50.0;
    let wy   = cos(p1 - lag) * 35.0;
    let wpt  = point_spike * 50.0;    // wrists overshoot the elbow on a point

    pose.left_wrist.x  = base.left_wrist.x  - wx;
    pose.left_wrist.y  = base.left_wrist.y  + wy  + roof * 1.6 + wpt + bounce * 0.2;
    pose.left_wrist.z  = base.left_wrist.z  - point_spike * 12.0;

    pose.right_wrist.x = base.right_wrist.x + wx;
    pose.right_wrist.y = base.right_wrist.y  - wy + roof * 1.6 + wpt + bounce * 0.2;
    pose.right_wrist.z = base.right_wrist.z  - point_spike * 12.0;

    // ── Torso ─────────────────────────────────────────────────────────────────
    // Hips sway side-to-side (ph = every 2 beats), subtle chest pop
    let hip_sway  = sin(ph) * 12.0;
    pose.torso_sway = hip_sway;
    pose.torso_lean = sin(p2) * 4.0;         // chest pops forward/back 2× per beat

    pose.waist.x  = base.waist.x + hip_sway * 0.5;
    pose.waist.y  = base.waist.y + bounce * 0.5;
    pose.waist.z  = base.waist.z;

    pose.crotch.x = base.crotch.x + hip_sway * 0.7;
    pose.crotch.y = base.crotch.y + bounce * 0.35;
    pose.crotch.z = base.crotch.z;

    // ── Legs: alternating high-knee running-man kicks ─────────────────────────
    //
    // The motion: knee drives UP and FORWARD (−Z toward viewer), while the
    // ankle swings BACK (+Z away) — classic running / funky-chicken style.
    // We use the positive lobe of sin so each leg only kicks on its own half-beat.
    // powf(0.65) softens the hard edge a bit → smoother rise and fall.
    //
    // The crotch already has hip sway, so knees drift with it naturally.

    let kick_h  = 38.0;    // knee lift height
    let kick_fwd = 18.0;   // knee forward throw (−Z)
    let kick_sx  = 10.0;   // slight inward pull on the kicking leg

    let raw_l  =  sin(p1).max(0.0);            // 0..1, left kicks on beat 1
    let raw_r  = (-sin(p1)).max(0.0);           // 0..1, right kicks on beat 2
    let kl     = raw_l.powf(0.65);
    let kr     = raw_r.powf(0.65);

    // LEFT knee ── up + forward when kicking
    pose.left_knee.x = base.left_knee.x + hip_sway * 0.35 - kick_sx * kl;
    pose.left_knee.y = base.left_knee.y + kick_h  * kl + bounce * 0.25;
    pose.left_knee.z = base.left_knee.z - kick_fwd * kl;    // toward viewer

    // LEFT ankle ── swings BACK as knee comes forward (like a real stride)
    let alk    = (sin(p1 - 0.55)).max(0.0).powf(0.65);   // ankle lags knee by ~0.55 rad
    pose.left_ankle.x = base.left_ankle.x + hip_sway * 0.20;
    pose.left_ankle.y = base.left_ankle.y + 10.0 * alk;   // slight lift
    pose.left_ankle.z = base.left_ankle.z + 28.0 * kl;    // foot kicks back (+Z) strongly

    // RIGHT knee ── mirror of left
    pose.right_knee.x = base.right_knee.x + hip_sway * 0.35 + kick_sx * kr;
    pose.right_knee.y = base.right_knee.y + kick_h  * kr + bounce * 0.25;
    pose.right_knee.z = base.right_knee.z - kick_fwd * kr;

    // RIGHT ankle
    let ark    = (-sin(p1 - 0.55)).max(0.0).powf(0.65);
    pose.right_ankle.x = base.right_ankle.x + hip_sway * 0.20;
    pose.right_ankle.y = base.right_ankle.y + 10.0 * ark;
    pose.right_ankle.z = base.right_ankle.z + 28.0 * kr;

    // ── Subtle heel-click on the off-beat ─────────────────────────────────────
    // Both ankles briefly come together at the top of a small hop every 4 beats.
    let click_phase = beat % 4.0;          // 0..4
    let click_spike = if (click_phase - 2.0).abs() < 0.25 {
        // small window around beat 2 of every bar
        1.0 - (click_phase - 2.0).abs() / 0.25
    } else { 0.0 };
    // Draw ankles inward (toward X=0) and slightly up
    let click_inward = click_spike * 14.0;
    let click_up     = click_spike * 12.0;
    // Only apply when neither leg is actively mid-kick (avoid fighting the kick motion)
    if kl < 0.15 && kr < 0.15 {
        pose.left_ankle.x  -= click_inward;
        pose.left_ankle.y  += click_up;
        pose.right_ankle.x += click_inward;
        pose.right_ankle.y += click_up;
    }
}
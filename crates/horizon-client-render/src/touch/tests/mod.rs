// Adapted from Starling's touch.rs tests (see the header of touch.rs).
use super::draw::mining_radius;
use super::*;

mod gestures;

fn role(t: &TouchControls, id: u64) -> Role {
    t.fingers[&id].0
}

#[test]
fn factory_swap_stays_beside_inventory_and_moved_swap_stays_put() {
    for (w, h) in [(2560, 1600), (2400, 1080), (1080, 2400), (240, 320)] {
        for left_handed in [false, true] {
            for mode in Mode::ALL {
                let mut t = TouchControls::default();
                t.viewport(w, h);
                t.settings.left_handed = left_handed;
                t.settings.mode = mode;
                let swap = t.placement(Control::Offhand).rect(t.aspect());
                for control in [
                    Control::Inventory,
                    Control::Sprint,
                    Control::Sneak,
                    Control::Jump,
                ] {
                    let other = t.placement(control).rect(t.aspect());
                    let overlaps = swap[0] < other[0] + other[2]
                        && other[0] < swap[0] + swap[2]
                        && swap[1] < other[1] + other[3]
                        && other[1] < swap[1] + swap[3];
                    assert!(!overlaps, "{w}x{h}, {left_handed}, {mode:?}: {control:?}");
                }
                t.settings.layouts[mode.index()][Control::Offhand as usize].center = [0.55, 0.43];
                let moved = t.placement(Control::Offhand).center;
                assert!((moved[0] - if left_handed { 0.45 } else { 0.55 }).abs() < 0.0001);
                assert_eq!(moved[1], 0.43);
            }
        }
    }
}

#[test]
fn swap_hands_sends_one_press_and_shield_holds_until_release() {
    let mut t = TouchControls::default();
    assert!(!t.visible(Control::Offhand));
    t.settings.offhand_button = true;
    t.update(1, t.placement(Control::Offhand).center, true, false);
    assert!(t.clicked);
    assert_eq!(
        t.key_events(),
        [(SWAP_HANDS.into(), true), (SWAP_HANDS.into(), false)]
    );
    t.clear();
    t.settings.mode = Mode::Tap;
    t.shield_available = true;
    t.update(2, t.placement(Control::Shield).center, true, false);
    assert!(t.shield_held());
    // Sliding off the button keeps the shield up.
    t.update(2, [0.5, 0.4], false, false);
    assert!(t.shield_held());
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(2, [0.5, 0.4], false, true);
    assert!(!t.shield_held());
    assert_eq!(t.poll_state().triggers[0], 0.0);
}

#[test]
fn shield_uses_the_use_button_in_crosshair_layouts() {
    let mut t = TouchControls {
        shield_available: true,
        ..TouchControls::default()
    };
    assert!(!t.visible(Control::Shield));
    let p = t.placement(Control::Use).center;
    t.update(1, p, true, false);
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(1, p, false, true);
    assert_eq!(t.poll_state().triggers[0], 0.0);
    for mode in [Mode::Tap, Mode::Dpad] {
        t.clear();
        t.settings.mode = mode;
        assert!(t.visible(Control::Shield));
        let p = t.placement(Control::Shield).center;
        t.update(2, p, true, false);
        assert!(t.shield_held() && t.poll_state().triggers[0] > 0.0);
        t.update(2, p, false, true);
        assert!(!t.shield_held());
        t.force_crosshair = true;
        assert!(!t.visible(Control::Shield));
        t.force_crosshair = false;
        t.settings.shield_button = false;
        assert!(!t.visible(Control::Shield));
        t.settings.shield_button = true;
    }
    // Hiding the use button brings the shield button back.
    t.settings.mode = Mode::Crosshair;
    t.settings.button_visibility[Control::Use as usize] = layout::HIDDEN;
    assert!(t.visible(Control::Shield));
    t.shield_available = false;
    assert!(!t.visible(Control::Shield));
}

#[test]
fn sprint_toggle_survives_release_and_clears_with_focus() {
    let mut t = TouchControls::default();
    t.settings.sprint_toggle = true;
    let point = t.placement(Control::Sprint).center;
    t.update(1, point, true, false);
    t.update(1, point, false, true);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(2, point, true, false);
    t.update(2, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
    t.update(3, point, true, false);
    t.clear();
    assert!(!t.poll_state().held(Button::LeftStick));
    t.settings.sprint_toggle = false;
    t.update(4, point, true, false);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(4, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
}

#[test]
fn button_scale_changes_hit_areas_not_saved_layouts() {
    let mut t = TouchControls::default();
    t.viewport(1920, 1080);
    let before = t.placement(Control::Jump);
    let stored = t.settings.placement(Control::Jump).size;
    t.settings.button_scale = 1.5;
    assert!((t.placement(Control::Jump).size - before.size * 1.5).abs() < 0.00001);
    assert_eq!(t.settings.placement(Control::Jump).size, stored);
}

#[test]
fn big_controls_stay_on_narrow_and_wide_surfaces() {
    let mut t = TouchControls::default();
    for (w, h) in [(400, 1000), (1920, 1080)] {
        t.viewport(w, h);
        t.settings.layouts[0][Control::Jump as usize].center = [0.99, 0.99];
        t.settings.layouts[0][Control::Jump as usize].size = 0.45;
        t.settings.button_scale = 2.0;
        let r = t.placement(Control::Jump).rect(t.aspect());
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= 1.0001 && r[1] + r[3] <= 1.0001);
    }
}

#[test]
fn portrait_spreads_the_factory_top_row_but_not_moved_buttons() {
    let mut t = TouchControls::default();
    t.viewport(480, 854);
    let top = [
        Control::Debug,
        Control::Chat,
        Control::Pause,
        Control::Settings,
        Control::Perspective,
    ];
    let rects: Vec<_> = top
        .iter()
        .map(|c| t.placement(*c).rect(t.aspect()))
        .collect();
    for pair in rects.windows(2) {
        assert!(pair[0][0] + pair[0][2] < pair[1][0]);
    }
    t.settings.layouts[0][Control::Chat as usize].center = [0.25, 0.2];
    assert_eq!(t.placement(Control::Chat).center, [0.25, 0.2]);
}

#[test]
fn shortcut_only_overlay_keeps_menus_without_movement() {
    let mut t = TouchControls {
        auxiliary_only: true,
        ..TouchControls::default()
    };
    assert!(!t.visible(Control::Move) && !t.visible(Control::Jump));
    assert!(t.visible(Control::Debug));
    let at = t.placement(Control::Debug).center;
    assert!(t.auxiliary_at(at));
    assert!(!t.auxiliary_at(t.settings.placement(Control::Jump).center));
    t.update(77, at, true, false);
    assert!(t.debug_requested && t.owns_pointer(77));
    t.update(77, at, false, true);
    assert!(!t.owns_pointer(77));
}

#[test]
fn shortcuts_take_their_finger_and_leave_movement_alone() {
    let mut t = TouchControls::default();
    t.update(1, [0.13, 0.76], true, false);
    t.update(1, [0.13, 0.68], false, false);
    let moving = t.poll_state().left;
    assert_ne!(moving, [0.0; 2]);
    for (id, control) in [
        (2, Control::Debug),
        (3, Control::Perspective),
        (4, Control::Chat),
        (5, Control::Settings),
    ] {
        let point = t.settings.placement(control).center;
        t.update(id, point, true, false);
        assert!(matches!(role(&t, id), Role::Ignore));
        assert_eq!(t.poll_state().left, moving);
        assert_eq!(t.poll_state().triggers, [0.0; 2]);
        t.update(id, point, false, true);
    }
    assert!(t.debug_requested && t.perspective_requested);
    assert!(t.chat_requested && t.settings_requested);
    t.clear();
    assert!(!t.debug_requested && !t.perspective_requested && !t.chat_requested);
}

#[test]
fn contextual_use_button_is_opt_in_and_stays_until_release() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.context_use = true;
    assert!(!t.visible(Control::Use));
    t.settings.contextual_interact = true;
    assert!(t.visible(Control::Use));
    t.update(1, [0.92, 0.52], true, false);
    t.context_use = false;
    assert!(t.visible(Control::Use));
    assert_eq!(t.poll_state().triggers[0], 1.0);
    t.update(1, [0.92, 0.52], false, true);
    assert!(!t.visible(Control::Use));
    t.settings.mode = Mode::Crosshair;
    assert!(t.visible(Control::Use));
}

#[test]
fn quick_taps_survive_one_poll_and_shared_buttons_release_together() {
    let mut t = TouchControls::default();
    t.update(1, [0.92, 0.78], true, false);
    t.update(1, [0.92, 0.78], false, true);
    assert!(t.poll_state().held(Button::South));
    assert!(!t.poll_state().held(Button::South));
    t.update(2, [0.80, 0.64], true, false);
    t.update(3, [0.80, 0.64], true, false);
    t.update(2, [0.80, 0.64], false, true);
    assert_eq!(t.state().triggers[1], 1.0);
    t.update(3, [0.80, 0.64], false, true);
    assert_eq!(t.state().triggers[1], 0.0);
}

#[test]
fn joystick_camera_and_jump_have_their_own_fingers() {
    let mut t = TouchControls::default();
    t.update(1, [0.13, 0.76], true, false);
    t.update(1, [0.20, 0.64], false, false);
    t.update(2, [0.6, 0.4], true, false);
    let d = t.update(2, [0.65, 0.45], false, false);
    assert!((d[0] - 0.05).abs() < 0.0001);
    t.update(3, [0.92, 0.78], true, false);
    assert!(t.state().held(Button::South));
    assert!(t.state().left[0] > 0.6);
    // A second finger on the joystick neither moves nor steals it.
    t.update(4, [0.09, 0.76], true, false);
    t.update(4, [0.02, 0.70], false, false);
    assert!(t.state().left[0] > 0.6);
    t.clear();
    assert_eq!(t.poll_state(), PadState::default());
    assert_eq!(t.update(9, [0.5, 0.5], false, false), [0.0; 2]);
}

#[test]
fn joystick_sprint_starts_past_the_rim_and_stops_inside() {
    let mut t = TouchControls::default();
    t.settings.sprint_with_joystick = true;
    t.update(1, [0.13, 0.76], true, false);
    t.update(1, [0.13, 0.62], false, false);
    assert!(!t.state().held(Button::LeftStick));
    t.update(1, [0.13, 0.58], false, false);
    assert!(t.state().held(Button::LeftStick));
    assert!((t.state().left[1] - 1.0).abs() < 1e-6);
    t.update(1, [0.13, 0.62], false, false);
    assert!(!t.state().held(Button::LeftStick));
}

#[test]
fn dpad_slides_between_directions_crouches_on_double_tap_and_mirrors() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Dpad;
    t.update(1, [0.13, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
    t.update(1, [0.20, 0.76], false, false);
    assert_eq!(t.state().left, [1.0, 0.0]);
    t.clear();
    t.update(10, [0.13, 0.76], true, false);
    assert!(!t.poll_state().held(Button::RightStick));
    t.update(10, [0.13, 0.76], false, true);
    t.update(12, [0.13, 0.76], true, false);
    assert!(t.poll_state().held(Button::RightStick));
    t.clear();
    // Flying, the centre holds sneak (fly down) at once.
    t.vertical_movement = true;
    t.update(13, [0.13, 0.76], true, false);
    assert!(t.state().held(Button::RightStick));
    t.clear();
    t.vertical_movement = false;
    t.settings.swap_jump_sneak = true;
    t.update(11, [0.13, 0.76], true, false);
    assert!(t.state().held(Button::South));
    t.clear();
    t.settings.left_handed = true;
    t.update(2, [0.87, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
}

#[test]
fn hidden_sprint_button_sends_nothing() {
    let mut t = TouchControls::default();
    t.settings.sprint_button = false;
    let center = t.settings.placement(Control::Sprint).center;
    t.update(1, center, true, false);
    assert!(!t.poll_state().held(Button::LeftStick));
    t.update(1, center, false, true);
    t.settings.sprint_button = true;
    t.update(2, center, true, false);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(2, center, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
}

#[test]
fn forced_crosshair_brings_back_attack_and_use_and_ignored_fingers_do_nothing() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    assert!(!t.crosshair() && !t.visible(Control::Attack));
    t.force_crosshair = true;
    assert!(t.crosshair() && t.visible(Control::Attack) && t.visible(Control::Use));
    t.settings.button_visibility[Control::Attack as usize] = layout::HIDDEN;
    assert!(!t.visible(Control::Attack));
    t.ignore(5);
    assert!(t.owns_pointer(5));
    assert_eq!(t.update(5, [0.9, 0.9], false, false), [0.0; 2]);
    assert_eq!(t.state(), PadState::default());
    t.tap(Button::East);
    assert!(t.poll_state().held(Button::East));
    assert!(!t.poll_state().held(Button::East));
}

#[test]
fn a_world_touch_turns_the_camera_only_once_it_drags() {
    let mut t = TouchControls::default();
    t.update(1, [0.6, 0.4], true, false);
    assert!(matches!(role(&t, 1), Role::Look { dragged: false, .. }));
    // A small wobble is not a drag.
    assert_eq!(t.update(1, [0.605, 0.4], false, false), [0.0; 2]);
    let d = t.update(1, [0.7, 0.45], false, false);
    assert!((d[0] - 0.095).abs() < 1e-5 && (d[1] - 0.05).abs() < 1e-5);
    assert_eq!(t.poll_state(), PadState::default());
    t.update(1, [0.7, 0.45], false, true);
    assert!(!t.owns_pointer(1));
    // Buttons never turn the camera.
    let jump = t.placement(Control::Jump).center;
    t.update(2, jump, true, false);
    assert_eq!(
        t.update(2, [jump[0] - 0.1, jump[1]], false, false),
        [0.0; 2]
    );
}

/// Fork-specific layout and editor behaviour.
#[test]
fn factory_swap_stays_beside_inventory_without_covering_action_buttons() {
    for (w, h) in [(2560, 1600), (2400, 1080), (1080, 2400), (240, 320)] {
        for left_handed in [false, true] {
            for mode in Mode::ALL {
                let mut touch = TouchControls::default();
                touch.viewport(w, h);
                touch.settings.left_handed = left_handed;
                touch.settings.mode = mode;
                let swap = touch.placement(Control::Offhand).rect(touch.aspect());
                for control in [
                    Control::Inventory,
                    Control::Sprint,
                    Control::Sneak,
                    Control::Jump,
                ] {
                    let other = touch.placement(control).rect(touch.aspect());
                    let overlaps = swap[0] < other[0] + other[2]
                        && other[0] < swap[0] + swap[2]
                        && swap[1] < other[1] + other[3]
                        && other[1] < swap[1] + swap[3];
                    assert!(!overlaps, "{w}x{h}, {left_handed}, {mode:?}: {control:?}");
                }
                touch.settings.layouts[mode.index()][Control::Offhand as usize].center =
                    [0.55, 0.43];
                let custom = touch.placement(Control::Offhand).center;
                assert!((custom[0] - if left_handed { 0.45 } else { 0.55 }).abs() < 0.0001);
                assert_eq!(custom[1], 0.43);
            }
        }
    }
}
#[test]
fn dynamic_area_follows_the_configured_width_and_height() {
    let mut t = TouchControls::default();
    t.settings.dynamic_area_x = 0.25;
    t.settings.dynamic_area_y = 0.5;
    let area = t.dynamic_area();
    assert_eq!([area[0], area[2], area[3]], [0.0, 0.25, 0.5]);
    let move_y = t.placement(Control::Move).center[1];
    let expected_y = (move_y - 0.25).clamp(0.0, 0.5);
    assert!((area[1] - expected_y).abs() < 1e-4);
    // The height is centred on the movement control, so a touch far above
    // it stays a camera gesture.
    t.settings.dynamic_joystick = true;
    t.update(1, [0.1, 0.05], true, false);
    assert!(matches!(t.fingers[&1].0, Role::Look { .. }));
    t.update(1, [0.1, 0.05], false, true);
    t.update(2, [0.1, move_y], true, false);
    assert!(matches!(t.fingers[&2].0, Role::Move(_)));
    // Y saturates at the surface edges instead of leaving the screen.
    t.settings.dynamic_area_y = 1.0;
    t.settings.left_handed = true;
    let area = t.dynamic_area();
    assert_eq!([area[0], area[1], area[3]], [0.75, 0.0, 1.0]);
}

#[test]
fn dynamic_joystick_spawns_only_in_the_movement_strip_and_locks_its_frame() {
    let mut t = TouchControls::default();
    t.settings.dynamic_joystick = true;
    // Outside the movement-side quarter this stays a camera gesture.
    t.update(1, [0.5, 0.5], true, false);
    assert!(matches!(t.fingers[&1].0, Role::Look { .. }));
    t.update(1, [0.5, 0.5], false, true);
    // Inside it, the frame is where the finger first landed.
    t.update(2, [0.12, 0.6], true, false);
    assert!(
        matches!(t.fingers[&2].0, Role::Move(a) if (a[0] - 0.12).abs() < 1e-4 && (a[1] - 0.6).abs() < 1e-4)
    );
    t.update(2, [0.20, 0.6], false, false);
    assert!(matches!(t.fingers[&2].0, Role::Move(a) if (a[0] - 0.12).abs() < 1e-4));
    assert!(t.state().left[0] > 0.0);
    // Right-handed layouts use the same strip on the other side.
    t.settings.left_handed = true;
    t.settings.mode = Mode::Crosshair;
    t.update(2, [0.20, 0.6], false, true);
    t.update(3, [0.80, 0.6], true, false);
    assert!(matches!(t.fingers[&3].0, Role::Move(_)));
}

#[test]
fn dynamic_joystick_respects_hidden_controls_and_has_no_fixed_hitbox() {
    let mut t = TouchControls::default();
    t.settings.dynamic_joystick = true;
    t.settings.dynamic_area_x = 0.05;
    let home = t.placement(Control::Move).center;
    t.update(1, home, true, false);
    assert!(matches!(t.fingers[&1].0, Role::Look { .. }));
    t.clear();

    // The edge strip participates in the same hit test as the other controls.
    let edge = [0.025, 0.5];
    assert_eq!(t.control_at(edge), Some(Control::Move));
    t.update(2, edge, true, false);
    assert!(matches!(t.fingers[&2].0, Role::Move(_)));
    // A second finger cannot steal the movement anchor.
    t.update(3, [0.03, 0.6], true, false);
    assert!(matches!(t.fingers[&3].0, Role::Look { .. }));
    t.update(2, [0.05, 0.5], false, false);
    assert!(t.state().left[0] > 0.0);
    t.update(2, [0.05, 0.5], false, true);
    assert_eq!(t.state().left, [0.0; 2]);
    t.clear();

    t.settings.button_visibility[Control::Move as usize] = 2;
    t.update(4, edge, true, false);
    assert!(matches!(t.fingers[&4].0, Role::Look { .. }));
    t.clear();
    t.settings.button_visibility[Control::Move as usize] = 0;
    t.settings.dynamic_area_x = 0.0;
    t.update(5, [0.0, 0.5], true, false);
    assert!(matches!(t.fingers[&5].0, Role::Look { .. }));
}

#[test]
fn dynamic_joystick_keeps_fixed_and_dpad_controls_and_button_priority() {
    let mut t = TouchControls::default();
    let home = t.placement(Control::Move).center;
    let start = [home[0] + 0.01, home[1]];
    t.update(1, start, true, false);
    assert!(matches!(t.fingers[&1].0, Role::Move(anchor) if anchor == home));
    t.clear();

    t.settings.dynamic_joystick = true;
    t.settings.mode = Mode::Dpad;
    let home = t.placement(Control::Move).center;
    let up = [home[0], home[1] - t.placement(Control::Move).size / 3.0];
    t.update(2, up, true, false);
    assert!(matches!(t.fingers[&2].0, Role::Move(anchor) if anchor == home));
    t.clear();

    t.settings.mode = Mode::Crosshair;
    t.settings.dynamic_area_x = 1.0;
    let jump = t.placement(Control::Jump).center;
    t.update(3, jump, true, false);
    assert!(t.state().held(Button::South));
    assert!(
        !t.fingers
            .values()
            .any(|(role, _)| matches!(role, Role::Move(_)))
    );
}

#[test]
fn camera_calibration_uses_physical_distance_on_both_axes() {
    let mut settings = Settings::default();
    for (w, h) in [(2400, 1080), (1080, 2400)] {
        let turn = camera_delta([100.0 / w as f32, 100.0 / h as f32], w, h, &settings);
        assert!((turn[0] - turn[1]).abs() < 0.0001);
        settings.sensitivity_vertical = 0.5;
        let tuned = camera_delta([100.0 / w as f32, 100.0 / h as f32], w, h, &settings);
        assert!((tuned[0] - tuned[1] * 2.0).abs() < 0.0001);
        settings.sensitivity_vertical = 1.0;
    }
}

#[test]
fn surface_tools_tap_but_projectiles_keep_hold_use_and_center_aim() {
    for id in [
        "bone_meal",
        "flint_and_steel",
        "fire_charge",
        "water_bucket",
        "honeycomb",
    ] {
        assert!(item_uses_on_tap(&format!("minecraft:{id}")));
    }
    for id in [
        "bow",
        "crossbow",
        "snowball",
        "egg",
        "wind_charge",
        "trident",
    ] {
        let id = format!("minecraft:{id}");
        assert!(!item_uses_on_tap(&id));
        assert!(projectile_crosshair(&id));
    }
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    assert!(!projectile_crosshair("minecraft:firework_rocket"));
    t.projectile_crosshair = true;
    t.hold_uses_item = true;
    t.update(1, [0.6, 0.4], true, false);
    t.update(1, [0.6, 0.4], false, true);
    t.poll_state();
    assert_eq!(t.aim, Some([0.5, 0.5]));
    assert_eq!(t.take_tap(), Some(true));
}

#[test]
fn new_buttons_preserve_old_layouts_and_release_shield() {
    let settings = Settings::default();
    let mut old = serde_json::to_value(&settings).unwrap();
    for row in old["layouts"].as_array_mut().unwrap() {
        row.as_array_mut().unwrap().truncate(12);
    }
    old["button_visibility"]
        .as_array_mut()
        .unwrap()
        .truncate(12);
    old["layouts"][0][0]["center"] = serde_json::json!([0.2, 0.7]);
    let restored: Settings = serde_json::from_value(old).unwrap();
    assert_eq!(restored.layouts[0][0].center, [0.2, 0.7]);
    assert!(!restored.offhand_button);
    let mut t = TouchControls::default();
    assert!(!t.visible(Control::Offhand));
    t.settings.offhand_button = true;
    let p = t.placement(Control::Offhand).center;
    t.update(1, p, true, false);
    assert_eq!(
        t.key_events(),
        [
            ("action:key.swapOffhand".into(), true),
            ("action:key.swapOffhand".into(), false)
        ]
    );
    t.clear();
    t.settings.mode = Mode::Tap;
    t.settings.shield_button = true;
    t.shield_available = true;
    let p = t.placement(Control::Shield).center;
    t.update(2, p, true, false);
    assert!(t.shield_held());
    t.update(2, [0.5, 0.4], false, false);
    assert!(t.shield_held());
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(2, [0.5, 0.4], false, true);
    assert!(!t.shield_held());
    assert_eq!(t.poll_state().triggers[0], 0.0);
}

#[test]
fn shield_uses_the_existing_use_button_in_aim_layouts() {
    let mut t = TouchControls::default();
    t.shield_available = true;
    assert!(!t.visible(Control::Shield));
    let p = t.placement(Control::Use).center;
    t.update(1, p, true, false);
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(1, p, false, true);
    assert_eq!(t.poll_state().triggers[0], 0.0);

    for mode in [Mode::Tap, Mode::Dpad] {
        t.clear();
        t.settings.mode = mode;
        assert!(t.visible(Control::Shield));
        let p = t.placement(Control::Shield).center;
        t.update(2, p, true, false);
        assert!(t.shield_held());
        assert!(t.poll_state().triggers[0] > 0.0);
        t.update(2, p, false, true);
        assert!(!t.shield_held());

        t.force_crosshair = true;
        assert!(!t.visible(Control::Shield));
        t.force_crosshair = false;
        t.settings.shield_button = false;
        assert!(!t.visible(Control::Shield));
        t.settings.shield_button = true;
    }
    // A player hiding the normal use button can retain the dedicated shield button.
    t.settings.mode = Mode::Crosshair;
    t.settings.button_visibility[Control::Use as usize] = 2;
    assert!(t.visible(Control::Shield));
}

#[test]
fn entity_placement_taps_do_not_change_consumption_and_throwing_holds() {
    for id in [
        "oak_boat",
        "oak_chest_boat",
        "bamboo_raft",
        "bamboo_chest_raft",
        "villager_spawn_egg",
        "minecart",
        "chest_minecart",
        "armor_stand",
        "item_frame",
        "painting",
        "end_crystal",
    ] {
        assert!(
            item_places_entity_on_tap(&format!("minecraft:{id}")),
            "{id}"
        );
    }
    for id in [
        "egg",
        "snowball",
        "wind_charge",
        "firework_rocket",
        "fishing_rod",
        "apple",
        "wheat_seeds",
        "diamond_helmet",
        "bow",
    ] {
        assert!(
            !item_places_entity_on_tap(&format!("minecraft:{id}")),
            "{id}"
        );
    }
    assert!(!item_places_entity_on_tap("example:spell_boat"));
}

#[test]
fn world_taps_never_press_attack_or_use_and_remember_the_original_item() {
    for (mode, split) in [
        (Mode::Tap, false),
        (Mode::Dpad, false),
        (Mode::Crosshair, false),
        (Mode::Dpad, true),
    ] {
        for held_use in [false, true] {
            let mut t = TouchControls::default();
            t.settings.mode = mode;
            t.settings.split_controls = split;
            t.settings.crosshair_gestures = true;
            t.hold_uses_item = held_use;
            t.update(1, [0.6, 0.4], true, false);
            t.hold_uses_item = !held_use;
            t.update(1, [0.6, 0.4], false, true);
            assert_eq!(t.poll_state().triggers, [0.0; 2]);
            assert_eq!(t.take_tap(), Some(held_use));
            assert_eq!(t.take_tap(), None);
            t.update(2, [0.6, 0.4], true, false);
            t.update(2, [0.6, 0.4], false, true);
            t.clear();
            assert_eq!(t.take_tap(), None);
        }
    }
}
#[test]
fn sprint_toggle_survives_finger_release_and_clears_on_focus_loss() {
    let mut t = TouchControls {
        aspect: 16.0 / 9.0,
        ..Default::default()
    };
    t.settings.sprint_toggle = true;
    let point = t.placement(Control::Sprint).center;
    t.update(1, point, true, false);
    t.update(1, point, false, true);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(2, point, true, false);
    t.update(2, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
    t.update(3, point, true, false);
    t.clear();
    assert!(!t.poll_state().held(Button::LeftStick));
    t.settings.sprint_toggle = false;
    t.update(4, point, true, false);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(4, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
}
#[test]
fn global_button_scale_changes_hit_geometry_without_rewriting_saved_layouts() {
    let mut touch = TouchControls::default();
    touch.viewport(1920, 1080);
    let before = touch.placement(Control::Jump);
    let stored = touch.settings.placement(Control::Jump).size;
    touch.settings.button_scale = 1.5;
    let after = touch.placement(Control::Jump);
    assert!((after.size - before.size * 1.5).abs() < 0.00001);
    assert_eq!(touch.settings.placement(Control::Jump).size, stored);
    touch.settings.button_scale = f32::NAN;
    touch.settings.sanitize();
    assert_eq!(touch.settings.button_scale, 1.0);
}

#[test]
fn oversized_controls_can_be_dragged_on_narrow_portrait_surfaces() {
    let mut touch = TouchControls {
        aspect: 0.4,
        ..Default::default()
    };
    touch.settings.layouts[0][Control::Move as usize].size = 0.45;
    touch.open_editor();
    let center = touch.placement(Control::Move).center;
    touch.edit_touch(77, center, true, false);
    assert_eq!(touch.editor.as_ref().unwrap().selected, Some(Control::Move));
    touch.edit_touch(77, [0.95, 0.95], false, false);
    let [x, y, w, h] = touch.placement(Control::Move).rect(touch.aspect());
    assert!(x >= 0.0 && y >= 0.0 && x + w <= 1.0001 && y + h <= 1.0001);
    touch.cancel_editor();
}
#[test]
fn portrait_factory_shortcuts_do_not_overlap_and_margins_do_not_move_controls() {
    let mut touch = TouchControls::default();
    touch.viewport(480, 854);
    let controls = [
        Control::Debug,
        Control::Chat,
        Control::Pause,
        Control::Settings,
        Control::Perspective,
    ];
    let rects: Vec<_> = controls
        .iter()
        .map(|c| touch.placement(*c).rect(touch.aspect()))
        .collect();
    for pair in rects.windows(2) {
        assert!(pair[0][0] + pair[0][2] < pair[1][0]);
    }
    touch.settings.horizontal_margin = 0.12;
    touch.settings.vertical_margin = 0.12;
    assert_eq!(
        rects,
        controls
            .iter()
            .map(|c| touch.placement(*c).rect(touch.aspect()))
            .collect::<Vec<_>>()
    );
    touch.settings.layouts[0][Control::Chat as usize].center = [0.25, 0.2];
    assert_eq!(touch.placement(Control::Chat).center, [0.25, 0.2]);
}

#[test]
fn editor_add_row_creates_a_button_without_closing_the_editor() {
    let mut touch = TouchControls::default();
    touch.viewport(1920, 1080);
    touch.open_editor();
    assert!(!touch.edit_touch(1, [0.15, 0.165], true, true));
    assert!(!touch.edit_touch(2, [0.5, 0.875], true, true));
    assert!(touch.editor.is_some());
    assert_eq!(touch.settings.custom_buttons.len(), 1);
    assert_eq!(touch.settings.custom_buttons[0].key, "key.keyboard.unknown");
    touch.cancel_editor();
    assert!(touch.settings.custom_buttons.is_empty());
}

#[test]
fn keypad_utility_overlay_keeps_buttons_without_movement_targets() {
    let mut t = TouchControls::default();
    t.viewport(1920, 1080);
    t.auxiliary_only = true;
    assert!(!t.visible(Control::Move));
    assert!(!t.visible(Control::Jump));
    assert!(t.visible(Control::Debug));
    let at = t.placement(Control::Debug).center;
    assert!(t.auxiliary_at(at));
    t.update(77, at, true, false);
    assert!(t.debug_requested);
    t.update(77, at, false, true);
    assert!(!t.owns_pointer(77));
}
#[test]
fn attack_gauge_survives_tap_modes_and_respects_off() {
    assert_eq!(attack_indicator(0, false), 0);
    assert_eq!(attack_indicator(1, false), 2);
    assert_eq!(attack_indicator(1, true), 1);
    assert_eq!(attack_indicator(2, false), 2);
}
#[test]
fn shortcuts_consume_their_finger_and_preserve_movement() {
    let mut touch = TouchControls::default();
    touch.update(1, [0.13, 0.76], true, false);
    touch.update(1, [0.13, 0.68], false, false);
    let moving = touch.poll_state().left;
    assert_ne!(moving, [0.0; 2]);
    for (id, control) in [(2, Control::Debug), (3, Control::Perspective)] {
        let point = touch.settings.placement(control).center;
        touch.update(id, point, true, false);
        assert!(matches!(touch.fingers[&id].0, Role::Ignore));
        assert_eq!(touch.poll_state().left, moving);
        assert_eq!(touch.poll_state().triggers, [0.0; 2]);
        touch.update(id, point, false, true);
    }
    assert!(touch.debug_requested && touch.perspective_requested);
    touch.clear();
    assert!(!touch.debug_requested && !touch.perspective_requested);
}
#[test]
fn mining_indicator_grows_outwards_with_bounded_radius() {
    assert_eq!(mining_radius(16.0, 0.0), 0.0);
    assert_eq!(mining_radius(16.0, 0.5), 7.25);
    assert_eq!(mining_radius(16.0, 1.0), 14.5);
    assert_eq!(mining_radius(16.0, 2.0), 14.5);
    assert_eq!(mining_radius(16.0, -1.0), 0.0);
    assert_eq!(mining_radius(16.0, f32::NAN), 0.0);
}
#[test]
fn contextual_button_is_opt_in_targeted_and_owned_until_release() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.context_use = true;
    assert!(!t.visible(Control::Use));
    t.settings.contextual_interact = true;
    assert!(t.visible(Control::Use));
    t.update(1, [0.92, 0.52], true, false);
    t.context_use = false;
    assert!(t.visible(Control::Use));
    assert_eq!(t.poll_state().triggers[0], 1.0);
    t.update(1, [0.92, 0.52], false, true);
    assert!(!t.visible(Control::Use));
    t.settings.mode = Mode::Crosshair;
    assert!(t.visible(Control::Use));
}
#[test]
fn looking_with_food_does_not_start_eating_and_use_button_never_disappears() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.hold_uses_item = true;
    t.update(1, [0.6, 0.4], true, false);
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    t.update(1, [0.7, 0.4], false, false);
    if let Some((Role::Look { since, .. }, _)) = t.fingers.get_mut(&1) {
        *since = Instant::now() - Duration::from_secs(1);
    }
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    assert!(!t.visible(Control::Use));
    assert_eq!(t.pointer(), Some([0.7, 0.4]));
}
#[test]
fn all_four_schemes_are_selectable_and_large_icons_remain_inside_the_surface() {
    let mut t = TouchControls::default();
    for (i, (mode, split, _)) in Mode::SCHEMES.into_iter().enumerate() {
        t.settings.mode = mode;
        t.settings.split_controls = split;
        assert_eq!(t.settings.mode, mode);
        assert_eq!(t.settings.split_controls, split);
        assert_eq!(t.settings.crosshair(), i == 0 || i == 3);
    }
    t.cancel_editor();
    t.settings.layouts[0][Control::Jump as usize].center = [0.99, 0.99];
    t.settings.layouts[0][Control::Jump as usize].size = 0.4;
    let r = t.placement(Control::Jump).rect(t.aspect());
    assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= 1.00001 && r[1] + r[3] <= 1.00001);
}
#[test]
fn optional_crosshair_gestures_target_center_for_both_movement_schemes() {
    for (mode, split) in [(Mode::Crosshair, false), (Mode::Dpad, true)] {
        let mut t = TouchControls::default();
        t.settings.mode = mode;
        t.settings.split_controls = split;
        t.settings.crosshair_gestures = true;
        t.update(1, [0.6, 0.4], true, false);
        t.update(1, [0.6, 0.4], false, true);
        assert_eq!(t.poll_state().triggers, [0.0; 2]);
        assert_eq!(t.take_tap(), Some(false));
        assert_eq!(t.aim, Some([0.5, 0.5]));
        t.inventory_mode = true;
        t.settings.button_visibility[Control::Attack as usize] = 2;
        assert!(!t.visible(Control::Attack));
    }
}
#[test]
fn quick_tap_survives_one_poll_and_shared_buttons_release_independently() {
    let mut t = TouchControls::default();
    t.update(1, [0.92, 0.78], true, false);
    t.update(1, [0.92, 0.78], false, true);
    assert!(t.poll_state().held(Button::South));
    assert!(!t.poll_state().held(Button::South));
    t.update(2, [0.80, 0.64], true, false);
    t.update(3, [0.80, 0.64], true, false);
    t.update(2, [0.80, 0.64], false, true);
    assert_eq!(t.state().triggers[1], 1.0);
    t.update(3, [0.80, 0.64], false, true);
    assert_eq!(t.state().triggers[1], 0.0);
}
#[test]
fn joystick_camera_and_jump_have_independent_owners() {
    let mut t = TouchControls::default();
    t.update(1, [0.13, 0.76], true, false);
    t.update(1, [0.20, 0.64], false, false);
    t.update(2, [0.6, 0.4], true, false);
    let d = t.update(2, [0.65, 0.45], false, false);
    assert!((d[0] - 0.05).abs() < 0.0001);
    t.update(3, [0.92, 0.78], true, false);
    assert!(t.state().held(Button::South));
    assert!(t.state().left[0] > 0.6);
    t.update(4, [0.09, 0.76], true, false);
    t.update(4, [0.02, 0.70], false, false);
    assert!(t.state().left[0] > 0.6);
    t.clear();
    assert_eq!(t.poll_state(), PadState::default());
}
#[test]
fn auto_sprint_starts_outside_and_stops_inside_the_joystick() {
    let mut t = TouchControls::default();
    t.settings.sprint_with_joystick = true;
    t.update(1, [0.13, 0.76], true, false);
    t.update(1, [0.13, 0.62], false, false);
    assert!(!t.state().held(Button::LeftStick));
    t.update(1, [0.13, 0.58], false, false);
    assert!(t.state().held(Button::LeftStick));
    t.update(1, [0.13, 0.62], false, false);
    assert!(!t.state().held(Button::LeftStick));
}
#[test]
fn tap_aim_and_camera_drag_are_distinct() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.update(1, [0.6, 0.4], true, false);
    t.update(1, [0.6, 0.4], false, true);
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    assert_eq!(t.take_tap(), Some(false));
    assert_eq!(t.aim, Some([0.6, 0.4]));
    assert_eq!(t.poll_state().triggers[0], 0.0);
    assert_eq!(t.aim, None);
    t.update(2, [0.6, 0.4], true, false);
    t.update(2, [0.7, 0.4], false, false);
    t.update(2, [0.7, 0.4], false, true);
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
}

#[test]
fn long_hold_keeps_mining_while_the_camera_turns() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.update(1, [0.6, 0.4], true, false);
    if let Some((Role::Look { since, .. }, _)) = t.fingers.get_mut(&1) {
        *since = Instant::now() - Duration::from_millis(400);
    }
    assert_eq!(t.poll_state().triggers[1], 1.0);
    assert_eq!(t.aim, Some([0.6, 0.4]));
    assert_ne!(t.update(1, [0.7, 0.4], false, false), [0.0; 2]);
    assert_eq!(t.poll_state().triggers[1], 1.0);
    assert_eq!(t.aim, Some([0.7, 0.4]));
    t.update(1, [0.7, 0.4], false, true);
    assert_eq!(t.poll_state().triggers[1], 0.0);
}
#[test]
fn held_food_survives_drift_and_releases_with_its_owner() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.hold_uses_item = true;
    t.update(1, [0.6, 0.4], true, false);
    if let Some((Role::Look { since, .. }, _)) = t.fingers.get_mut(&1) {
        *since = Instant::now() - Duration::from_millis(400);
    }
    assert_eq!(t.poll_state().triggers, [1.0, 0.0]);
    assert!(t.holding_use);
    assert_ne!(t.update(1, [0.66, 0.46], false, false), [0.0; 2]);
    t.hold_uses_item = false; // consuming the last item cannot become mining
    assert_eq!(t.poll_state().triggers, [1.0, 0.0]);
    assert_eq!(t.aim, Some([0.66, 0.46]));
    t.update(2, [0.4, 0.3], true, false);
    assert_ne!(t.update(2, [0.45, 0.3], false, false), [0.0; 2]);
    assert_eq!(t.poll_state().triggers, [1.0, 0.0]);
    t.update(1, [0.66, 0.46], false, true);
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    assert!(!t.holding_use);
}
#[test]
fn contextual_hold_can_latch_use_without_turning_short_taps_into_holds() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.update(1, [0.6, 0.4], true, false);
    t.poll_state();
    assert!(!t.holding);
    if let Some((Role::Look { since, .. }, _)) = t.fingers.get_mut(&1) {
        *since = Instant::now() - Duration::from_millis(400);
    }
    assert_eq!(t.poll_state().triggers, [0.0, 1.0]);
    assert!(t.holding);
    t.latch_use();
    assert_eq!(t.poll_state().triggers, [1.0, 0.0]);
    assert_ne!(t.update(1, [0.66, 0.4], false, false), [0.0; 2]);
    t.update(1, [0.66, 0.4], false, true);
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    assert!(!t.holding);
    t.update(2, [0.6, 0.4], true, false);
    t.update(2, [0.6, 0.4], false, true);
    assert_eq!(t.poll_state().triggers, [0.0, 0.0]);
    assert_eq!(t.take_tap(), Some(false));
    assert!(!t.holding);
}
#[test]
fn touch_marker_follows_camera_drag_without_starting_an_action() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    assert_eq!(t.pointer(), None);
    t.update(1, [0.6, 0.4], true, false);
    assert_eq!(t.pointer(), Some([0.6, 0.4]));
    assert_eq!(t.poll_state().triggers, [0.0; 2]);
    assert_eq!(t.aim, Some([0.6, 0.4]));
    t.update(1, [0.7, 0.4], false, false);
    t.poll_state();
    assert_eq!(t.pointer(), Some([0.7, 0.4]));
    assert_eq!(t.aim, None);
    t.clear();
    t.settings.mode = Mode::Crosshair;
    t.update(1, [0.6, 0.4], true, false);
    assert_eq!(t.pointer(), None);
}
#[test]
fn cancel_layout_preserves_already_applied_touch_options() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.settings.left_handed = true;
    let original = t.settings.placement(Control::Move).center;
    t.open_editor();
    t.edit_touch(1, original, true, false);
    t.edit_touch(1, [0.7, 0.7], false, true);
    assert_ne!(t.settings.placement(Control::Move).center, original);
    t.cancel_editor();
    assert_eq!(t.settings.placement(Control::Move).center, original);
    assert_eq!(t.settings.mode, Mode::Tap);
    assert!(t.settings.left_handed);
}
#[test]
fn dpad_can_slide_between_directions_and_mirror() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Dpad;
    t.update(1, [0.13, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
    t.update(1, [0.20, 0.76], false, false);
    assert_eq!(t.state().left, [1.0, 0.0]);
    t.clear();
    t.update(10, [0.13, 0.76], true, false);
    assert!(!t.poll_state().held(Button::RightStick));
    t.update(10, [0.13, 0.76], false, true);
    t.update(12, [0.13, 0.76], true, false);
    assert!(t.poll_state().held(Button::RightStick));
    t.clear();
    t.settings.swap_jump_sneak = true;
    t.update(11, [0.13, 0.76], true, false);
    assert!(t.state().held(Button::South));
    t.clear();
    t.settings.left_handed = true;
    t.update(2, [0.87, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
}
#[test]
fn editing_owns_input_and_cancel_restores_layouts() {
    let mut t = TouchControls::default();
    t.open_editor();
    t.edit_touch(1, [0.13, 0.76], true, false);
    t.edit_touch(1, [0.18, 0.70], false, true);
    assert!((t.settings.placement(Control::Move).center[0] - 0.18).abs() < 0.001);
    assert_eq!(t.poll_state(), PadState::default());
    t.cancel_editor();
    assert_eq!(t.settings.placement(Control::Move).center, [0.13, 0.76]);
    t.settings.mode = Mode::Tap;
    t.open_editor();
    assert_eq!(t.settings.mode, Mode::Tap);
    t.edit_touch(3, [0.13, 0.76], true, true);
    t.edit_touch(4, [0.6, 0.82], true, true);
    t.edit_touch(5, [0.65, 0.73], true, true);
    assert!(t.settings.placement(Control::Move).size > 0.4);
    t.cancel_editor();
    assert_eq!(t.settings.mode, Mode::Tap);
}
#[test]
fn editor_deselects_and_can_apply_opacity_to_the_current_mode() {
    let mut t = TouchControls::default();
    t.open_editor();
    t.edit_touch(1, [0.13, 0.76], true, true);
    t.edit_touch(2, [0.4, 0.82], true, true); // Opacity
    t.edit_touch(3, [0.5, 0.64], true, true); // Apply to all
    t.edit_touch(4, [0.5, 0.73], true, true);
    for p in t.settings.layouts[0] {
        assert!((p.opacity - 0.5).abs() < 0.001);
    }
    assert_eq!(t.settings.layouts[1][0].opacity, 0.65);
    t.edit_touch(5, [0.7, 0.4], true, true);
    assert!(t.editor.as_ref().unwrap().selected.is_none());
    t.cancel_editor();
    assert_eq!(t.settings.layouts[0][0].opacity, 0.65);
}
#[test]
fn layouts_roundtrip_and_reject_invalid_sizes() {
    let mut s = Settings::default();
    s.mode = Mode::Dpad;
    s.layouts[2][2].center = [0.4, 0.5];
    let bytes = serde_json::to_vec(&s).unwrap();
    let mut r: Settings = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(r.mode, Mode::Dpad);
    assert_eq!(r.layouts[2][2].center, [0.4, 0.5]);
    r.layouts[2][2].size = f32::NAN;
    r.sensitivity = f32::INFINITY;
    r.sanitize();
    assert!(r.layouts[2][2].size.is_finite());
    assert_eq!(r.sensitivity, 1.0);
    let rect = r.placement(Control::Jump).rect(20.0 / 9.0);
    assert!((rect[2] * (20.0 / 9.0) - rect[3]).abs() < 0.0001);
}
#[test]
fn projection_targets_screen_point_without_turning_camera() {
    let forward = [0.0, 0.0, 1.0];
    let up = [0.0, 1.0, 0.0];
    assert_eq!(
        screen_direction(forward, up, [0.5, 0.5], 2.0, 90.0),
        forward
    );
    let ray = screen_direction(forward, up, [0.75, 0.25], 2.0, 90.0);
    assert!(ray[0] < 0.0 && ray[1] > 0.0 && ray[2] > 0.0);
    assert!((ray.iter().map(|x| x * x).sum::<f64>() - 1.0).abs() < 1e-10);
}
#[test]
fn hotbar_all_nine_slots() {
    for i in 0..9 {
        assert_eq!(
            hotbar_slot(427.0, 240.0, [133.5 + i as f32 * 20.0, 230.0]),
            Some(i)
        );
    }
    assert_eq!(hotbar_slot(427.0, 240.0, [320.0, 230.0]), None);
}
#[test]
fn old_profiles_keep_custom_layouts_and_new_preferences_have_safe_defaults() {
    let mut settings: Settings =
        serde_json::from_str(r#"{"mode":"tap","sensitivity":1.5}"#).unwrap();
    settings.sanitize();
    assert_eq!(settings.mode, Mode::Tap);
    assert_eq!(settings.sensitivity, 1.5);
    assert_eq!(settings.circle_size, 32.0);
    assert!(!settings.analog_movement);
    assert!(settings.sprint_button);
    settings.circle_size = f32::NAN;
    settings.sanitize();
    assert_eq!(settings.circle_size, 32.0);
    settings.sprint_button = false;
    let center = settings.placement(Control::Sprint).center;
    let mut touch = TouchControls {
        settings,
        ..Default::default()
    };
    touch.update(1, center, true, false);
    assert!(!touch.poll_state().held(Button::LeftStick));
    touch.update(1, center, false, true);
    touch.settings.sprint_button = true;
    touch.update(2, center, true, false);
    assert!(touch.poll_state().held(Button::LeftStick));
    touch.update(2, center, false, true);
    touch.poll_state(); // consume the queued tap
    assert!(!touch.poll_state().held(Button::LeftStick));
}

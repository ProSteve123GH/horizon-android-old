# Zalith-style Android controls

Horizon Android now uses the default Zalith Launcher 2 control arrangement for
new touch profiles. The normalized positions are ported from Zalith's
`default_layout.json`, including its left joystick and lower-right action
cluster:

- an analog joystick on the lower-left;
- independent Attack and Use buttons on the lower-right;
- Jump above the action cluster and Sneak beside it;
- Sprint near the movement control;
- utility buttons kept along the top and bottom margins.

The layout is still stored in Horizon's `horizon-touch.json` format. This keeps
the existing per-profile editor, left-handed mirroring, opacity, size, dynamic
joystick option, sprint toggle, keyboard, mouse, controller, and multi-touch
handling. Existing customized layouts are not overwritten.

The control renderer remains native Rust because Horizon's game loop and
`NativeActivity` do not run Zalith's Kotlin/Compose layer. The event mapping is
kept equivalent: joystick movement, primary/secondary actions, jump, sneak,
sprint, F3, F5, inventory, chat, pause, and multi-touch ownership all route to
the same Horizon actions. Zalith Launcher 2 is GPL-3.0; its original notices
remain applicable to any Kotlin/Compose source copied into this fork, while the
existing Horizon source retains its original MIT notice.

For a fresh profile, open **Options > Controls > Touch Controls > Customize
layout** to move, resize, hide, or restore the factory positions. Existing
profiles can use the same screen and are intentionally left unchanged.

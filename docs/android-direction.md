# Android architecture and compatibility

Horizon Android adds native packaging, installation, touch input and Android
platform integration to [Horizon's Rust engine](https://github.com/ludovic111/horizon).
World simulation, the renderer, Java protocol and shared menus remain upstream
components. [Integration records](android-upstream.json) identify reviewed merges.

## Compatibility

| Content | Support |
| --- | --- |
| Minecraft versions | Java 26.3 / protocol 777; other JAR versions are not launch targets |
| Vanilla-compatible Java servers | Multiplayer client implemented; compatibility depends on server, proxy and anticheat |
| Authenticated servers | Xbox/XSTS and Minecraft session handoff implemented; project-owned production app approval remains pending |
| Native `.hmod` | Wasmtime components on 64-bit builds, subject to the versioned SDK and declared capabilities |
| Resource packs | Supported within the renderer's implemented formats |
| Fabric/Forge/NeoForge and Mixins | Java code does not execute; native ports are required |
| Java modpacks | Not exposed by the Android launcher |
| Custom modded server protocols | Require explicit protocol and gameplay implementations |

See [the feature matrix](android-features.md), [mod SDK](mods.md),
[account integration](microsoft-account.md), [Android build guide](android.md),
[Minecraft installation import](android-minecraft-import.md) and
[the fork notes](android-fork.md).

## Platform boundaries

Gameplay, simulation and rendering are Rust. The first screen is a Kotlin/Jetpack
Compose launcher in the main process; it talks to the Rust controller over JNI.
The game runs in a separate `:game` process as a stock `NativeActivity`, so the
launcher survives the game lifecycle and there is no second winit event loop.
Rust/JNI uses the system text editor and IME on Android's UI thread. The APK
contains one engine library, not a second copy for the launcher.

Android graphics choices select system-driver Vulkan or GLES paths. Game
resolution, draw distance, quality and FPS controls remain game options.
Performance changes should use comparable frame-time measurements with the
same device, settings, world and camera path.

## Remaining platform work

- Storage Access Framework world import/export and log sharing (Minecraft
  installation import is implemented; player-data import/export is separate).
- Production account registration and broader authenticated-server validation.
- Hardware gamepad axes, controller rumble and complete TV navigation.
- Low-memory and legacy GPU testing, lifecycle recovery and thermal profiling.
- Native mod ports backed by explicit SDK capabilities and compatibility tests.

A different brand does not change Java-mod compatibility. Running unchanged
Java mods requires a Java execution architecture beyond this native client.

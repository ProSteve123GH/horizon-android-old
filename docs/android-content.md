# Android add-on files

Use **Mods**, **Packs** or **Shaders → Import file**, or open **Installed** and
press Import beside the desired group. Select a file in Android's document picker;
the picker reads a file from another app or from Downloads and copies it into
Horizon. Horizon no longer appears as a folder in the system Files app.

Player files are browsed in the launcher's **Game files** screen
(Settings → Storage & Files → Browse game files). Instance files are under
`Instances/Default` or `Instances/<named instance>` and are identified by their
stable `pN` marker. Launcher touch-layout snapshots are under `Controls/<pN>/`,
while the live `horizon-touch.json` remains in the instance. Files stay in the
active game-files root; no extra copy is made. Account files, private launcher
state and prepared game resources are not exposed. Screenshots continue to
appear in **Pictures/HorizonAndroid**.

The game settings in the active instance include `options.txt`,
`horizon-graphics.txt`, `horizon-touch.json` and `horizon-keypad.json`. These are
the existing engine settings, not copied or relocated files. The launcher
snapshot file `Controls/<pN>/android-touch-layouts.json` is launcher data, not
the live touch configuration. Return to the launcher before editing settings or
worlds. Changes apply when the game starts again. Keep a backup before manual
edits; invalid settings may be reset by their loader.

To download a `.hmod` from a link, paste its HTTPS URL in the Mods search field
and press **Import .hmod** (the Search button changes when a URL is entered).
The URL's suffix is not trusted to decide the type: the archive's contents do.
A `.hmod` may be content-only or content plus native Rust source; a native
bundle is detected by `horizon-native.json` and is routed to the native mods
folder. Native source bundles are stored for inspection only on Android: the
APK does not compile or execute them. A pasted HTTPS link gets no registry
hash or signature check unless its URL exactly matches a known Horizon registry
file, so paste only links you trust. File import and link import are both
available.

In **Shaders**, choose **Horizon** for native `.hshader` packs or **Modrinth**
for classic shader ZIPs. The selection is saved and each source has its own search
cache. Modrinth compatibility is experimental: downloading a pack does not mean
every effect is supported. Installation prefers a ZIP version listing the current
Minecraft version, otherwise the newest downloadable ZIP. Source checksums and
archive structure are checked before installation. An existing file belonging to
another project or imported manually is not replaced. Successful downloads appear
in **Installed > Shaders**, where they can be selected or removed.

Successful imports and catalog installs in the launcher open Installed with the
matching group expanded. Errors and cancellation keep the current page. Import
results appear in the status line; progress and conflict dialogs use the bundled font.

Catalog searches run separately from installation jobs, so switching tabs does
not wait for a network response. At most one search runs at once; rapid changes
replace the pending request, and replies for an old tab cannot replace the current
list. The last eight query pages are cached for 30 seconds; Search explicitly
refreshes its query. Changing tabs does not unpack installed `.hmod` WASM files.

Supported files:

- Mods: `.hmod` archives, which may be content-only or content plus native
  Rust source. A package with WASM runs in the Wasmtime sandbox on 64-bit
  builds (32-bit builds omit that runtime but can still manage the files).
  A native part is detected by `horizon-native.json`, is **not sandboxed** on
  hosts that run it, and remains source-only on Android: it is not compiled or
  executed by the APK. The Installed page labels content/WASM and native
  archives separately; see [android-native-mods.md](android-native-mods.md).
  Java `.jar` mods are not accepted.
- Resource packs: ZIPs with valid `pack.mcmeta` at the archive root.
- Shaders: native `.hshader` packs, native shader ZIPs with `horizon-shaders.toml`
  (stored as `.hshader`), or classic ZIPs with shader sources under `shaders/`.
  A classic ZIP's structure being valid does not guarantee support for all of its
  shader features; the existing game loader reports compatibility errors.

Open Installed to expand the three groups. The list includes regular imported and
downloaded archive files in the current instance. Selection applies at the next
game launch. Removing an add-on requires confirmation and removes only that file,
its browser record and its enabled selection; worlds are not touched.

Imports copy the granted content URI to a temporary Android cache file and validate
it before committing. Existing files are never silently replaced: **Keep both**
chooses a new filename, **Replace** explicitly replaces the selected collision,
and **Cancel** leaves the existing file intact. A Horizon mod with the same manifest
ID must be replaced rather than loaded twice. Cancel is checked between copy and
validation blocks; a document provider may delay an in-flight read.

Limits: 256 MiB input, 512 MiB declared expansion, 32,768 entries and 1,024-byte
entry paths. Native mods are limited to 32 MiB input and 64 MiB expanded data.
Archive entry paths, duplicates, links, CRC/decompression errors and resource-pack
metadata are checked. Archives are kept as files, not extracted into arbitrary
filesystem paths. Invalid imports and interrupted temporary files do not replace
existing content. Original URI grants are not retained after copying.

The old Horizon DocumentsProvider was removed. Only Android's own document
pickers (and the private screenshot provider used for sharing) remain; they read
files the user selects and never expose Horizon's account data or prepared
resources. Game-data browsing, copying and deletion happen in the launcher's
in-app **Game files** screen, which works without a documents permission.
World and configuration files must not be changed while the game is running.

Copying directly with a file manager bypasses the Import file validator; wait for
the copy to finish before starting the game, and use the game's pack selection to
reload changes. Import file remains the validated route with progress/cancellation
and explicit conflict choices. A system file manager controls its own copy and
overwrite dialogs.

Developer checks: `python tools/test_android_content.py` covers the shared import
validator, streamed copy cancellation/limits and upload scheduling without starting
the game. Test the complete APK on Android for picker cancellation, duplicate-name
choices, deletion and keyboard/background recovery.
`python tools/test_android_storage.py --serial DEVICE` runs the Bionic file-lock
and canonical instance-layout probe in a separate small app with fresh data.

For a native shader smoke test, package Horizon's included Golden Hour example:

```sh
cargo run -p horizon-shader-api --bin horizon-shader -- pack crates/horizon-shader-api/packs/golden_hour --out .local/golden_hour-1.0.0.hshader
```

Transfer the resulting file to the phone, import it under Shaders, then enable it
in Installed before Play, or in the game's Shader Packs settings. It changes the
sky and adds two post-processing passes; it is a functional example, not an
Android performance benchmark.

The Horizon catalog also lists native mods so their availability is visible.
A unified `.hmod` can carry both content and native source: the native part
is detected by `horizon-native.json` and stored in `horizon-native-mods/`,
while its content part is mirrored to `horizon-mods/<id>.hmod` so this
installer and the game's mod list see it. The content works on its own; the
native source remains stored but is not compiled or run on Android. A legacy
`.hnmod` is native-only. All of these can be imported here — file and HTTPS
imports remain available — and none of the native code is executed or tested
on the phone during import.

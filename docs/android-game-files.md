# Game files location

Horizon can keep the player-facing game files in a folder on shared storage
instead of app storage. This page documents what moves, what stays private,
how the two modes work, and how recovery behaves on failure.

## The two locations

| | App storage (SAF) | Internal storage (recommended) |
| --- | --- | --- |
| Path | `getFilesDir()` | `/storage/emulated/0/Horizon-Android` (all builds) |
| File manager access | Through the documents provider (**Files → Horizon Android**) | Any file manager, directly |
| Permission | None | All files access (Android 11+), or the legacy storage permission (Android 9–10) |
| Marker file | — | `shared-storage.json` in app storage |

The user chooses in **Settings → Storage → Game files location**, a two-option
chooser; the internal storage option is recommended and marked as the default.
On first launch — or whenever the game is still on app storage — Horizon also
asks once, with a **"Game files on internal storage?"** prompt: **Grant access
and move** sends the user to the access Android shows and starts the move;
**Not now** dismisses the prompt for good (a private `android-storage-prompt.json`
marker in app storage records the choice) and app storage keeps working fully.
Nothing moves without the user's explicit grant or action.

## What moves

Everything the game and the instances need and the player owns:

- `saves/` (worlds), in the default instance and in every `profiles/profile-N`
  instance folder, including `android-profile-name.txt` and
  `android-profile-played.txt` (the instance's display name and last-played
  time).
- `horizon-mods/`, `resourcepacks/`, `shaderpacks/`, `options.txt`,
  `horizon-graphics.txt`, `horizon-touch.json`, `horizon-keypad.json`,
  `screenshots/`, `servers.dat`, `horizon-player.txt`, and any files
  with those roots that the game creates in the game folder.
- The shareable reports: `horizon-diagnostics-*.zip` and
  `horizon-performance-*.jsonl` from `performance/`. These are exactly what the
  documents provider exposes as **Reports**.

## What stays private

Never leaves app storage, on either move, enforced by a top-level denylist
and a nested one (android-* and dotfiles also never match):

- The engine build and the verified, prepared Minecraft resources
  (`engine-data*`, `.local/`).
- Account data: the Keystore-wrapped tokens, `account-secrets/`, and the
  per-launch `sessions/` files (always written to app storage, never moved),
  and every `sessions/` folder inside instances.
- Downloaded Minecraft files and the download cache (`.local/`, cache dir).
- Logs and crash reports (`logs/`, `crash-reports/`) and the raw diagnostics
  with potentially sensitive content (`diagnostic.log`, `*-session.json`,
  `android-exits.json`, exit traces, previous run logs).
- The launcher's own state (`android-*` files), the SAF marker, control
  sockets, the migration journal (`.storage-migration.json`), and any
  dotfile.

## Behavior

- **Grant:** Android 11+ shows the all-files-access system page; approving it
  returns to the launcher and the move starts by itself. Android 9 and
  10 ask for the storage permission in place. Android 10 also opts into legacy
  storage (`requestLegacyExternalStorage`).
- **Move back:** the same chooser, reversed; **Move back to app storage** is
  offered whenever internal storage is selected or expected. Restoring deletes
  the shared folder when it is empty.
- **Failure safety:** files are copied to the destination first, then
  re-read and compared byte for byte; only then are the sources deleted.
  Any error rolls back everything the run created; the source is untouched.
  The destination must be empty (or absent) and is refused if not. A
  free-space preflight leaves 64 MiB headroom.
- **Interrupted moves:** before anything is copied, the move records every
  file (source and destination), the directories it will create and the
  phase (`copying` | `deleting`) in a private journal
  (`.storage-migration.json`). On the next launcher start — or at the top of
  the next migration — an orphaned journal rolls back the partial copy
  (phase `copying`: exactly the recorded destinations are removed, sources
  untouched) or completes the move (phase `deleting`: sources are removed
  and the marker recorded, since the copies were verified). The journal is
  validated to describe only paths under the two roots it records; a
  journal that fails validation changes nothing.
- **Game running:** the move refuses while a game is playing, using the same
  `game.lock` the documents provider uses to refuse access mid-game.
- **Marker:** `shared-storage.json` in app storage records the choice and the
  shared folder's absolute path. Every reader (game, launcher, documents
  provider, screenshot gallery) resolves through it. If the shared folder is
  deleted, readers fall back to app storage and the launcher blocks Play with
  a message instead of opening an empty world list.
- **Missing folder:** Settings → Storage → Game files location shows the
  internal-storage option as *Folder missing*, never silently re-creates
  it, and offers explicit actions instead: **Re-create empty folder**
  (a clearly labelled empty folder; your deleted files are not restored by
  this — restore them from the file manager's Trash first) or **Move back to
  app storage** (works once the folder is readable again, e.g. after an
  accidental re-create or a re-mounted volume). Play stays blocked until the
  folder is readable or the marker is moved back.
- **Reports:** the recorder and the diagnostics exporter write to the active
  Reports location, so Files shows the same reports after a move.
- **SAF provider:** unchanged and still allowlisted. It serves the active
  game-files location (the recorded shared folder, or app storage); the
  shareable-reports subset is what it exposes as Reports.
- **Both locations holding files:** adopting an empty app storage onto an
  existing shared folder (the reinstall path) refuses when anything is left
  to lose in app storage, and so does moving; nothing is overwritten and the
  player must move or remove one side first.

## Revoking the access

If all-files access (or the legacy permission) is revoked, the shared game
files become unreadable to Horizon. The marker stays, the launcher blocks
Play, and **Game files location** reports the folder as missing. Grant the
access again in system settings (preferred), or re-create the folder from the
chooser (or from a file manager if it was deleted, from inside its Trash if
possible). Moving back to app storage also requires the access, since Horizon
must read the shared files to move them — and **Move back to app storage**
stays available from the same chooser.

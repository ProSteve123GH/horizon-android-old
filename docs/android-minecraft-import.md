# Reusing an existing Minecraft installation

Horizon Android can import the Minecraft 26.3 files that another launcher
already downloaded on the phone. The import is local: nothing is uploaded. Files
that are missing from the source are downloaded from Mojang during preparation,
using the same pinned hashes as **Download Minecraft 26.3**. Horizon does not
bundle or redistribute Mojang jars or assets.

## What is needed

The source must contain a version JSON published for this engine. The usual
launcher layout is:

```text
.minecraft/
├── versions/26.3/26.3.json       # required
├── versions/26.3/26.3.jar        # or versions/26.3/client.jar
├── assets/indexes/34.json        # optional; downloaded when absent
└── assets/objects/aa/<sha1>      # optional; downloaded when absent
```

The importer also accepts these roots:

- the folder itself (`.minecraft` selected directly);
- a parent folder containing `.minecraft/`, `minecraft/` or `Minecraft/`;
- a ZIP with any of those layouts, including entries named `./...` and Windows
  ZIPs using backslashes.

The version JSON may also be at `versions/26.3.json`. A launcher that stores the
client JAR as `versions/26.3/client.jar` is supported.

## What is not imported

Java libraries, native libraries, LWJGL, Java runtimes and launcher metadata are
not used by Horizon. They are ignored, not required and never downloaded. In
particular, an Android Java launcher that supplies its own LWJGL build can have
no `libraries/` folder at all; that does not prevent a Horizon import.

Player data and Java-only content are also skipped:

```text
saves/  logs/  screenshots/  resourcepacks/  shaderpacks/  mods/
config/  crash-reports/  backups/  libraries/  downloads/  cache/
```

Saves and profiles are not copied into Horizon. Use the Android world
import/export paths for player data.

## Importing from a folder

1. Open **Play** in the Compose launcher.
2. Choose **Import folder**.
3. Pick the Minecraft folder with Android's folder picker.
4. Keep Horizon open while it indexes the files, copies the required ones and
   downloads anything missing.

The picker grants read access to the selected tree. Horizon persists that grant
so a retry can reuse the same folder. If the grant is revoked, pick the folder
again.

Android 11 and later restrict access to other apps' private directories,
including many `Android/data/<package>/` folders. If the launcher's `.minecraft`
folder cannot be selected, export it as a ZIP from that launcher and use
**Import ZIP**, or use **Download Minecraft 26.3**.

Folder enumeration is read-only and has limits: 50,000 indexed files, 24 levels
of nesting and 32 MiB per metadata file. Directories that are never imported are
skipped before indexing. A provider that refuses traversal produces a readable
error instead of an empty import.

## Importing from a ZIP

**Import ZIP** opens the document picker for a ZIP. Horizon reads the archive
through a file descriptor; it does not unpack the ZIP to a second copy and does
not load the archive into RAM. The provider must return a seekable file
descriptor. A normal Downloads/DocumentsProvider ZIP does.

Duplicate entries are counted; the last indexed entry for a path is the one read.
Entries containing `..` are rejected and never read. A corrupt or truncated archive is
rejected before the prepared installation is touched.

## Verification and missing files

The importer checks the same pins as the official installer:

- the version JSON against the accepted Mojang manifest hashes;
- the client JAR against the engine's pinned client hash and size;
- the asset index against the accepted index hashes;
- every copied asset object against its SHA-1 and size.

Mojang has republished the 26.3 version JSON several times; only the asset
index changed. Horizon accepts the pinned manifest, the current launcher-meta
manifest and their asset indexes, so an existing launcher can keep whichever
one it downloaded. A version JSON rewritten by a launcher is not accepted unless
its hash is one of those Mojang manifests; use the official 26.3 metadata or
**Download Minecraft 26.3** in that case.

A source file that is present but has the wrong size or hash is an error: the
import stops and reports which file is corrupt. An existing Horizon installation
is not modified. A source file that is absent is different: it is counted as
missing, and the normal installer downloads it immediately after the copy phase.
This is why a folder without assets, without the client JAR, or without
`assets/indexes/34.json` can still finish.

The import record in
`.local/horizon-client/26.3/minecraft-import.json` keeps the source label and
counts. It is not player data and is removed by **Remove resources**.

## Storage and interruption

A full Minecraft 26.3 installation is roughly 500 MiB. An import needs room for
the copied cache and the prepared staging directory, so the launcher checks for
about twice the source/install size plus a 128 MiB reserve. The exact check is
shown in the UI as free space.

**Cancel** stops after the current file. Verified cache entries remain, so a
retry reuses them. Preparation is not cancellable: it verifies and atomically
promotes the prepared directory, and cancelling it could leave a half-written
installation. `.partial` files left by a killed process are removed at the next
import.

If preparation fails, the previous prepared installation is left in place. On
the next attempt the installer restores `26.3.previous` if it was interrupted
between renames.

## Repairing an installed copy

When Minecraft is installed, **Repair missing files** on the Play screen:

1. checks the prepared asset index and the size of every prepared asset;
2. reports that everything is present without downloading when the installation
   is complete;
3. otherwise hard-links usable prepared assets into the installer cache and runs
   the normal installer, which hashes the cache and downloads missing or damaged
   objects plus the client JAR;
4. atomically replaces the prepared directory and preserves the import record.

This repairs Horizon's own prepared resources. It does not add LWJGL to the
source launcher's folder, because Horizon never uses LWJGL.

## Remove resources

**Remove resources** deletes only:

```text
.local/horizon-client/26.3
.local/horizon-client/26.3.previous
.local/horizon-client/26.3.partial
.local/horizon-client/official.partial
cacheDir/minecraft-downloads/26.3
```

Saves, profiles, resource packs, mods, engine tables and the selected source
folder are kept. The source is never deleted or modified.

## Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| “No Minecraft 26.3 installation was found” | The folder has no accepted `versions/26.3/26.3.json`, or a launcher rewrote it. Point the picker at `.minecraft`, use official metadata, or use **Download**. |
| “This installation is not Minecraft 26.3” | The source is another Minecraft version. Horizon only launches 26.3. |
| “Minecraft game files differ from this engine's pinned version” | The client JAR is not the pinned 26.3 build. Use **Download**. |
| “Asset index does not match...” | The index is from another version or was modified. Remove it from the source and retry; the installer downloads the official index. |
| “Installation file is corrupt” | A source file has the wrong size/hash. The error names it. Replace it or use **Download**. |
| Picker cannot open a folder | Android 11+ blocks other apps' private directories. Export a ZIP or use **Download**. |
| Import stops with a provider error | The document provider denied a read or a persisted grant was revoked. Pick the folder again. |
| Not enough free storage | Free space in Android Files. The UI reports the estimate before preparation. |
| Repair says resources are complete | All indexed prepared assets are present. It does not re-download LWJGL or Java libraries. |

## Test status

The platform-independent importer has 15 Rust unit tests for layout discovery,
pinned metadata, cache reuse, cancellation, corruption, ZIP traversal and
record restore. The Android glue and Compose screens are compiled into the
arm64 dev APK by the remote workflow. CI run #4 (commit `128b45605`) passed APK
signing, package identity, ABI and payload checks; its artifact SHA-256 was
verified locally. The APK has not been installed on a physical Android device
in this workspace, so SAF provider behavior, ZIP file descriptors, the repair
flow and the installer still need device validation.

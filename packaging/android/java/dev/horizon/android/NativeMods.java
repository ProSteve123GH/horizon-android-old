package dev.horizon.android;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.os.Build;
import android.util.Log;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.Arrays;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Chooses the game library for this process on every start and loads it.
 *
 * <p>`files/native/current.json` (written by the launcher when a compiled modded
 * build is installed) names one `libhorizon_android.so` under `files/native/builds/`.
 * If it parses and passes every check, this start runs that library; otherwise the
 * stock game from the APK does. Either way `files/native/active.json` is written so
 * the native trampoline (`android.app.lib_name`) knows what to load, and the chosen
 * library is loaded here, before `super.onCreate`, so Java's JNI bindings resolve
 * against the same library the game runs.
 *
 * <p>Crash guard: while a modded build is unproven, every start leaves an empty
 * `pending_crash` marker behind. The game deletes it on its first presented frame
 * (the trampoline exports the path as `HORIZON_NATIVE_PENDING`) and writes a
 * universal `files/native/first-frame` marker that the splash overlay polls. If
 * the marker survives, the build crashed before its first frame; two survivals in
 * a row set the build aside (`current.json` moves to `current.json.rejected`,
 * `last_rejected` records why, `last_failed_mod` records the mod ids that were
 * active) and a healthy build recorded in `last_known_good.json` is restored.
 *
 * <p>Compatibility: `current.json` may carry `engine_commit`, `api_level` and
 * `min_apk_version`. Older bundles omit them and are accepted; when present a
 * mismatch is refused with a reason the launcher shows. `abi`, `versionCode` and
 * `sha256` are always checked.
 */
final class NativeMods {
    private static final String TAG = "HorizonNativeMods";

    // TODO(android-build): replace with the APK's build-time engine commit
    // (generated BuildConfig field or resource) once the build exposes one.
    // Until then any bundle that declares engine_commit is refused, which is
    // the safe side of the gate: absence is still accepted for old bundles.
    private static final String ENGINE_COMMIT = "unknown";
    /** The native mod ABI/API this APK's engine understands. */
    private static final int NATIVE_API_LEVEL = 1;

    private NativeMods() {}

    static void prepareAndLoad(Context context) {
        File nativeDir = new File(context.getFilesDir(), "native");
        // noinspection ResultOfMethodCallIgnored
        nativeDir.mkdirs();
        String stock = context.getApplicationInfo().nativeLibraryDir + File.separator + "libhorizon_android.so";
        // A first-frame marker from a previous process must not hide this run's splash.
        //noinspection ResultOfMethodCallIgnored
        new File(nativeDir, "first-frame").delete();
        String chosen = select(context, nativeDir);
        writeActive(nativeDir, chosen == null ? stock : chosen, stock);
        if (chosen != null) {
            // Replace an existing marker atomically: this start has not proven itself yet.
            writeFileAtomic(new File(nativeDir, "pending_crash"), "");
        } else {
            // A stale marker from a crashed modded start must not count as a
            // crash against a future build.
            //noinspection ResultOfMethodCallIgnored
            new File(nativeDir, "pending_crash").delete();
            //noinspection ResultOfMethodCallIgnored
            new File(nativeDir, "crash_streak").delete();
        }
        System.load(chosen == null ? stock : chosen);
    }

    /** Returns the chosen library's absolute path, or {@code null} for the stock game. */
    private static String select(Context context, File nativeDir) {
        File current = new File(nativeDir, "current.json");
        // A build that proved healthy once is the rollback target after a bad build
        // is set aside. Restore it here, before any other check, but never restore
        // the very build that was just rejected (that would loop forever).
        if (!current.isFile()) {
            File good = new File(nativeDir, "last_known_good.json");
            File rejected = new File(nativeDir, "current.json.rejected");
            if (good.isFile() && !sameLibrary(good, rejected)) {
                try {
                    copyAtomic(good, current);
                    Log.i(TAG, "Restored last_known_good.json as current.json");
                } catch (IOException error) {
                    Log.w(TAG, "Cannot restore last_known_good.json: " + error);
                }
            }
        }
        if (!current.isFile()) {
            return null;
        }
        String id;
        String version;
        String lib;
        long versionCode;
        String abi;
        String sha256;
        String engineCommit;
        String apiLevel;
        String minApkVersion;
        JSONObject json;
        try {
            json = new JSONObject(new String(Files.readAllBytes(current.toPath()), StandardCharsets.UTF_8));
            id = json.getString("id");
            version = json.getString("version");
            lib = json.getString("lib");
            versionCode = json.getLong("versionCode");
            abi = json.getString("abi");
            sha256 = json.optString("sha256", "").trim();
            // All three are optional: bundles built before the compat gate existed
            // carry none of them and keep working.
            engineCommit = json.optString("engine_commit", "").trim();
            apiLevel = json.optString("api_level", "").trim();
            minApkVersion = json.optString("min_apk_version", "").trim();
        } catch (Exception error) {
            return rejectSoft(nativeDir, current, "current.json is unreadable: " + error);
        }
        long apkVersionCode;
        try {
            PackageInfo info = context.getPackageManager().getPackageInfo(context.getPackageName(), 0);
            apkVersionCode = info.getLongVersionCode();
            if (apkVersionCode != versionCode) {
                return rejectSoft(nativeDir, current, "modded build targets versionCode " + versionCode
                        + " but this APK is " + apkVersionCode);
            }
        } catch (Exception error) {
            return rejectSoft(nativeDir, current, "cannot read this APK's versionCode: " + error);
        }
        boolean supported = false;
        for (String candidate : Build.SUPPORTED_ABIS) {
            if (candidate.equals(abi)) {
                supported = true;
                break;
            }
        }
        if (!supported) {
            return rejectSoft(nativeDir, current, "modded build abi " + abi + " is not in " + Arrays.toString(Build.SUPPORTED_ABIS));
        }
        // Compatibility gate. Absent fields are fine; a present field that does
        // not match is a hard rejection with a reason the launcher can display.
        if (!engineCommit.isEmpty() && !engineCommit.equalsIgnoreCase(ENGINE_COMMIT)) {
            return rejectHard(nativeDir, current, "Mod requires engine commit " + engineCommit
                    + " but this APK has " + ENGINE_COMMIT + " (rebuild required)");
        }
        if (!apiLevel.isEmpty()) {
            int level;
            try {
                level = Integer.parseInt(apiLevel);
            } catch (NumberFormatException error) {
                return rejectHard(nativeDir, current, "Mod declares an invalid api_level: " + apiLevel);
            }
            if (level != NATIVE_API_LEVEL) {
                return rejectHard(nativeDir, current, "Mod requires native api_level " + level
                        + " but this APK supports " + NATIVE_API_LEVEL + " (rebuild required)");
            }
        }
        if (!minApkVersion.isEmpty()) {
            long minimum;
            try {
                minimum = Long.parseLong(minApkVersion);
            } catch (NumberFormatException error) {
                return rejectHard(nativeDir, current, "Mod declares an invalid min_apk_version: " + minApkVersion);
            }
            if (apkVersionCode < minimum) {
                return rejectHard(nativeDir, current, "Mod needs APK version " + minimum
                        + " or newer but this APK is " + apkVersionCode + " (update Horizon first)");
            }
        }
        File libFile = new File(lib);
        if (!libFile.isFile()) {
            return rejectSoft(nativeDir, current, "modded build library is missing: " + lib);
        }
        if (!sha256.isEmpty() && !sha256Matches(libFile, sha256)) {
            return rejectSoft(nativeDir, current, "modded build library sha256 does not match current.json");
        }
        // Crash guard: a marker from an earlier start means the game never reached
        // its first frame. Two markers in a row set the build aside.
        File pending = new File(nativeDir, "pending_crash");
        if (pending.isFile()) {
            int streak = readStreak(new File(nativeDir, "crash_streak")) + 1;
            if (streak >= 2) {
                String ids = failedModIds(json, id);
                writeFileAtomic(new File(nativeDir, "last_failed_mod"), ids);
                String reason = "A modded build (" + id + " " + version + "; mods: " + ids
                        + ") was set aside after crashing before its first frame two starts in a row. The stock game is used.";
                File rejectedFile = new File(nativeDir, "current.json.rejected");
                try {
                    moveAtomic(current.toPath(), rejectedFile.toPath());
                } catch (IOException error) {
                    Log.w(TAG, "Cannot move current.json aside: " + error);
                }
                writeFileAtomic(new File(nativeDir, "last_rejected"), reason);
                Log.w(TAG, reason);
                //noinspection ResultOfMethodCallIgnored
                pending.delete();
                //noinspection ResultOfMethodCallIgnored
                new File(nativeDir, "crash_streak").delete();
                return null;
            }
            writeFileAtomic(new File(nativeDir, "crash_streak"), Integer.toString(streak));
        }
        return lib;
    }

    /** The mod ids that were active when a build failed, for crash attribution. */
    private static String failedModIds(JSONObject current, String fallbackId) {
        try {
            JSONArray mods = current.optJSONArray("mods");
            if (mods != null && mods.length() > 0) {
                StringBuilder joined = new StringBuilder();
                for (int i = 0; i < mods.length(); i++) {
                    JSONObject entry = mods.optJSONObject(i);
                    String id = entry == null ? null : entry.optString("id", "").trim();
                    if (id == null || id.isEmpty()) {
                        continue;
                    }
                    if (joined.length() > 0) {
                        joined.append(',');
                    }
                    joined.append(id);
                }
                if (joined.length() > 0) {
                    return joined.toString();
                }
            }
        } catch (Exception ignored) {
            // Fall back to the build id.
        }
        return fallbackId == null ? "" : fallbackId;
    }

    /** True when two manifests name the same library, so rollback cannot loop. */
    private static boolean sameLibrary(File a, File b) {
        if (!a.isFile() || !b.isFile()) {
            return false;
        }
        try {
            String left = new JSONObject(new String(Files.readAllBytes(a.toPath()), StandardCharsets.UTF_8))
                    .optString("lib", "");
            String right = new JSONObject(new String(Files.readAllBytes(b.toPath()), StandardCharsets.UTF_8))
                    .optString("lib", "");
            return !left.isEmpty() && left.equals(right);
        } catch (Exception ignored) {
            return false;
        }
    }

    /** Log a reason the launcher can show and fall back without rejecting the build. */
    private static String rejectSoft(File nativeDir, File current, String reason) {
        Log.w(TAG, reason + "; playing the stock game");
        writeFileAtomic(new File(nativeDir, "last_rejected"), reason);
        return null;
    }

    /** Log, record why, and set the build aside. One hard mismatch is enough. */
    private static String rejectHard(File nativeDir, File current, String reason) {
        Log.w(TAG, reason + "; playing the stock game");
        try {
            moveAtomic(current.toPath(), new File(nativeDir, "current.json.rejected").toPath());
        } catch (IOException error) {
            Log.w(TAG, "Cannot move current.json aside: " + error);
        }
        writeFileAtomic(new File(nativeDir, "last_rejected"), reason);
        return null;
    }

    private static int readStreak(File file) {
        try {
            return Integer.parseInt(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8).trim());
        } catch (Exception ignored) {
            return 0;
        }
    }

    private static boolean sha256Matches(File file, String expectedHex) {
        try (InputStream in = new FileInputStream(file)) {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            byte[] buffer = new byte[1024 * 1024];
            int read;
            while ((read = in.read(buffer)) != -1) {
                digest.update(buffer, 0, read);
            }
            byte[] actual = digest.digest();
            StringBuilder hex = new StringBuilder(actual.length * 2);
            for (byte b : actual) {
                hex.append(Character.forDigit((b >> 4) & 0xF, 16));
                hex.append(Character.forDigit(b & 0xF, 16));
            }
            return hex.toString().equalsIgnoreCase(expectedHex);
        } catch (Exception error) {
            Log.w(TAG, "Cannot hash " + file + ": " + error);
            return false;
        }
    }

    private static void writeActive(File nativeDir, String lib, String stock) {
        final String json;
        try {
            json = new JSONObject().put("lib", lib).put("stock", stock).toString();
        } catch (org.json.JSONException error) {
            Log.w(TAG, "Cannot encode active.json: " + error);
            return;
        }
        writeFileAtomic(new File(nativeDir, "active.json"), json);
    }

    /** Write a file through a sibling temp file so readers never see a partial body. */
    private static void writeFileAtomic(File file, String content) {
        File temp = new File(file.getParentFile(), file.getName() + ".tmp");
        try {
            Files.write(temp.toPath(), content.getBytes(StandardCharsets.UTF_8));
            moveAtomic(temp.toPath(), file.toPath());
        } catch (IOException error) {
            Log.w(TAG, "Cannot write " + file + ": " + error);
            //noinspection ResultOfMethodCallIgnored
            temp.delete();
        }
    }

    /** Copy a file through a sibling temp file (used when restoring a known-good build). */
    private static void copyAtomic(File from, File to) throws IOException {
        File temp = new File(to.getParentFile(), to.getName() + ".tmp");
        try {
            Files.copy(from.toPath(), temp.toPath(), StandardCopyOption.REPLACE_EXISTING);
            moveAtomic(temp.toPath(), to.toPath());
        } catch (IOException error) {
            //noinspection ResultOfMethodCallIgnored
            temp.delete();
            throw error;
        }
    }

    private static void moveAtomic(java.nio.file.Path from, java.nio.file.Path to) throws IOException {
        try {
            Files.move(from, to, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } catch (AtomicMoveNotSupportedException unsupported) {
            Files.move(from, to, StandardCopyOption.REPLACE_EXISTING);
        }
    }
}

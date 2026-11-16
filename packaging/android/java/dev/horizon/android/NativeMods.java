package dev.horizon.android;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.os.Build;
import android.util.Log;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.Arrays;
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
 * `pending_crash` marker behind. If the game reaches its first frame it deletes the
 * marker and resets `crash_streak`. If the marker survives, the build crashed before
 * its first frame; two survivals in a row set the build aside (`current.json` moves
 * to `current.json.rejected`, `last_rejected` records why) and the stock game runs.
 */
final class NativeMods {
    private static final String TAG = "HorizonNativeMods";

    private NativeMods() {}

    static void prepareAndLoad(Context context) {
        File nativeDir = new File(context.getFilesDir(), "native");
        // noinspection ResultOfMethodCallIgnored
        nativeDir.mkdirs();
        String stock = context.getApplicationInfo().nativeLibraryDir + File.separator + "libhorizon_android.so";
        String chosen = select(context, nativeDir);
        writeActive(nativeDir, chosen == null ? stock : chosen);
        if (chosen != null) {
            // Truncate an existing marker: this start has not proven itself yet.
            try (java.io.FileOutputStream out = new java.io.FileOutputStream(new File(nativeDir, "pending_crash"), false)) {
                // create or truncate; the content is meaningless
            } catch (java.io.IOException error) {
                Log.w(TAG, "Cannot mark pending_crash: " + error);
            }
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
        if (!current.isFile()) {
            return null;
        }
        String id;
        String version;
        String lib;
        long versionCode;
        String abi;
        String sha256;
        try {
            JSONObject json = new JSONObject(new String(Files.readAllBytes(current.toPath()), StandardCharsets.UTF_8));
            id = json.getString("id");
            version = json.getString("version");
            lib = json.getString("lib");
            versionCode = json.getLong("versionCode");
            abi = json.getString("abi");
            sha256 = json.optString("sha256", "").trim();
        } catch (Exception error) {
            return rejected("current.json is unreadable: " + error);
        }
        try {
            PackageInfo info = context.getPackageManager().getPackageInfo(context.getPackageName(), 0);
            if (info.getLongVersionCode() != versionCode) {
                return rejected("modded build targets versionCode " + versionCode
                        + " but this APK is " + info.getLongVersionCode());
            }
        } catch (Exception error) {
            return rejected("cannot read this APK's versionCode: " + error);
        }
        boolean supported = false;
        for (String candidate : Build.SUPPORTED_ABIS) {
            if (candidate.equals(abi)) {
                supported = true;
                break;
            }
        }
        if (!supported) {
            return rejected("modded build abi " + abi + " is not in " + Arrays.toString(Build.SUPPORTED_ABIS));
        }
        File libFile = new File(lib);
        if (!libFile.isFile()) {
            return rejected("modded build library is missing: " + lib);
        }
        if (!sha256.isEmpty() && !sha256Matches(libFile, sha256)) {
            return rejected("modded build library sha256 does not match current.json");
        }
        // Crash guard: a marker from an earlier start means the game never reached
        // its first frame. Two markers in a row set the build aside.
        File pending = new File(nativeDir, "pending_crash");
        if (pending.isFile()) {
            int streak = readStreak(new File(nativeDir, "crash_streak")) + 1;
            if (streak >= 2) {
                String reason = "A modded build (" + id + " " + version
                        + ") was set aside after crashing before its first frame two starts in a row. The stock game is used.";
                File rejectedFile = new File(nativeDir, "current.json.rejected");
                try {
                    moveAtomic(current.toPath(), rejectedFile.toPath());
                } catch (java.io.IOException error) {
                    Log.w(TAG, "Cannot move current.json aside: " + error);
                }
                writeFile(new File(nativeDir, "last_rejected"), reason);
                Log.w(TAG, reason);
                // noinspection ResultOfMethodCallIgnored
                pending.delete();
                // noinspection ResultOfMethodCallIgnored
                new File(nativeDir, "crash_streak").delete();
                return null;
            }
            writeFile(new File(nativeDir, "crash_streak"), Integer.toString(streak));
        }
        return lib;
    }

    /** Log one line and fall back to the stock game. Does not move current.json: a single bad start is not proof. */
    private static String rejected(String reason) {
        Log.w(TAG, reason + "; playing the stock game");
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

    private static void writeActive(File nativeDir, String lib) {
        final String json;
        try {
            json = new JSONObject().put("lib", lib).toString();
        } catch (org.json.JSONException error) {
            Log.w(TAG, "Cannot encode active.json: " + error);
            return;
        }
        File target = new File(nativeDir, "active.json");
        File temp = new File(nativeDir, "active.json.tmp");
        writeFile(temp, json);
        try {
            moveAtomic(temp.toPath(), target.toPath());
        } catch (java.io.IOException error) {
            Log.w(TAG, "Cannot publish active.json: " + error);
        }
    }

    private static void writeFile(File file, String content) {
        try {
            Files.write(file.toPath(), content.getBytes(StandardCharsets.UTF_8));
        } catch (java.io.IOException error) {
            Log.w(TAG, "Cannot write " + file + ": " + error);
        }
    }

    private static void moveAtomic(java.nio.file.Path from, java.nio.file.Path to) throws java.io.IOException {
        try {
            Files.move(from, to, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } catch (AtomicMoveNotSupportedException unsupported) {
            Files.move(from, to, StandardCopyOption.REPLACE_EXISTING);
        }
    }
}

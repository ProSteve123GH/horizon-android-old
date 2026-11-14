package dev.horizon.android;

/**
 * The game's activity: the APK's own library, or the native-mod build the launcher chose
 * ({@link NativeMods}). The UI and file operations stay Rust.
 */
public final class HorizonActivity extends android.app.NativeActivity {
    /** The library to run instead of the APK's ({@code android_main.rs}'s {@code horizon_on_create}). */
    private static final String GAME_LIBRARY = "HORIZON_GAME_LIBRARY";
    private native void nativeDocumentResult(int request, int result, String uri);

    /**
     * Loads the library this start runs, before NativeActivity's {@code onCreate} calls the
     * native entry point (the manifest's {@code horizon_on_create}).
     */
    private void loadGame() {
        java.io.File build = NativeMods.INSTANCE.chosen(this, versionName());
        if (build != null) {
            try {
                android.system.Os.setenv(GAME_LIBRARY, build.getPath(), true);
                System.load(build.getPath());
                return;
            } catch (Throwable error) {
                try {
                    android.system.Os.unsetenv(GAME_LIBRARY);
                } catch (Throwable ignored) {
                }
                NativeMods.INSTANCE.failed(this, error.getMessage());
            }
        }
        System.loadLibrary("horizon_android");
    }

    /** The game drew its first frame ({@code platform::first_frame}): the build works. */
    public void firstFrame() {
        NativeMods.INSTANCE.started(this);
    }

    @Override protected void onCreate(android.os.Bundle state) {
        loadGame();
        super.onCreate(state);
    }

    private String versionName() {
        try {
            return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Throwable error) {
            return null;
        }
    }

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101) return;
        nativeDocumentResult(request, result,
                data == null || data.getData() == null ? null : data.getData().toString());
    }
}

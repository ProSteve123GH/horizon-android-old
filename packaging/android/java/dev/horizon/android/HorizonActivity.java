package dev.horizon.android;

import android.os.Bundle;

/**
 * The game's activity. Chooses and loads the game library for this process
 * (stock, or a modded build if the player installed one) before `NativeActivity`
 * starts, so the trampoline and Java's JNI bindings resolve against the same library.
 * The in-game document flow is a SAF result callback.
 */
public final class HorizonActivity extends android.app.NativeActivity {
    private native void nativeDocumentResult(int request, int result, String uri);

    @Override protected void onCreate(final Bundle savedInstanceState) {
        NativeMods.prepareAndLoad(this);
        super.onCreate(savedInstanceState);
    }

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101) return;
        nativeDocumentResult(request, result,
                data == null || data.getData() == null ? null : data.getData().toString());
    }
}

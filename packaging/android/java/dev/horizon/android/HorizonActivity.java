package dev.horizon.android;

import android.graphics.Color;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewParent;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.TextView;

/**
 * The game's activity. Chooses and loads the game library for this process
 * (stock, or a modded build if the player installed one) before `NativeActivity`
 * starts, so the trampoline and Java's JNI bindings resolve against the same library.
 * The in-game document flow is a SAF result callback.
 *
 * <p>A lightweight Java splash covers the native surface until the engine writes
 * `files/native/first-frame`, hiding the black gap while the library loads and
 * shaders compile. A 30s fallback makes sure it can never stick permanently.
 */
public final class HorizonActivity extends android.app.NativeActivity {
    private native void nativeDocumentResult(int request, int result, String uri);

    private Handler handler;
    private Runnable poll;
    private View splash;

    @Override protected void onCreate(final Bundle savedInstanceState) {
        NativeMods.prepareAndLoad(this);
        super.onCreate(savedInstanceState);
        installSplash();
    }

    /** Adds the splash above the native surface and starts watching for the first frame. */
    private void installSplash() {
        FrameLayout overlay = new FrameLayout(this);
        overlay.setBackgroundColor(Color.rgb(0x14, 0x12, 0x18));

        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.horizon_splash);
        logo.setScaleType(ImageView.ScaleType.FIT_CENTER);
        overlay.addView(logo, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        TextView label = new TextView(this);
        label.setText(R.string.splash_starting);
        label.setTextColor(Color.WHITE);
        label.setTextSize(16f);
        label.setGravity(Gravity.CENTER);
        FrameLayout.LayoutParams labelParams = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        labelParams.bottomMargin = (int) (56 * getResources().getDisplayMetrics().density);
        overlay.addView(label, labelParams);

        addContentView(overlay, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        splash = overlay;

        final java.io.File marker = new java.io.File(new java.io.File(getFilesDir(), "native"), "first-frame");
        handler = new Handler(Looper.getMainLooper());
        poll = new Runnable() {
            @Override public void run() {
                if (marker.isFile()) {
                    hideSplash();
                } else {
                    handler.postDelayed(this, 100);
                }
            }
        };
        handler.postDelayed(poll, 100);
        handler.postDelayed(new Runnable() {
            @Override public void run() { hideSplash(); }
        }, 30000);
    }

    /** Fades and removes the overlay once, whether the marker or the timeout won. */
    private void hideSplash() {
        final View view = splash;
        if (view == null) return;
        splash = null;
        if (handler != null && poll != null) handler.removeCallbacks(poll);
        view.animate().alpha(0f).setDuration(250).withEndAction(new Runnable() {
            @Override public void run() {
                ViewParent parent = view.getParent();
                if (parent instanceof ViewGroup) ((ViewGroup) parent).removeView(view);
            }
        }).start();
    }

    @Override protected void onDestroy() {
        if (handler != null && poll != null) handler.removeCallbacks(poll);
        splash = null;
        super.onDestroy();
    }

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101) return;
        nativeDocumentResult(request, result,
                data == null || data.getData() == null ? null : data.getData().toString());
    }
}

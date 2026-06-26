package com.kzahel.mclone.xr;

import android.app.NativeActivity;
import android.content.Intent;
import android.graphics.Color;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.util.Log;
import android.view.Gravity;
import android.view.WindowInsets;
import android.widget.EditText;
import android.widget.FrameLayout;

public class McloneXrActivity extends NativeActivity {
    private static final String TAG = "McloneXrActivity";
    private static final String EXTRA_STARTUP_ARGV = "mclone.startup.argv";
    private EditText readinessEditText;
    private Boolean lastImeVisible;

    static {
        System.loadLibrary("mclone_android_xr_client");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Log.i(TAG, "McloneXrActivity created");
        ensureReadinessEditText();
        logStartupArgv(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        Log.i(TAG, "McloneXrActivity received new intent");
        logStartupArgv(intent);
    }

    public String getMcloneStartupArgvJson() {
        Intent intent = getIntent();
        if (intent == null) {
            return null;
        }
        return intent.getStringExtra(EXTRA_STARTUP_ARGV);
    }

    private EditText ensureReadinessEditText() {
        if (readinessEditText != null) {
            return readinessEditText;
        }
        readinessEditText = new EditText(this);
        readinessEditText.setSingleLine(true);
        readinessEditText.setInputType(
                InputType.TYPE_CLASS_TEXT
                        | InputType.TYPE_TEXT_VARIATION_NORMAL
                        | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        readinessEditText.setFocusable(true);
        readinessEditText.setFocusableInTouchMode(true);
        readinessEditText.setBackgroundColor(Color.TRANSPARENT);
        readinessEditText.setTextColor(Color.TRANSPARENT);
        readinessEditText.setHintTextColor(Color.TRANSPARENT);
        readinessEditText.setCursorVisible(false);
        readinessEditText.setAlpha(0.01f);
        readinessEditText.setWidth(1);
        readinessEditText.setHeight(1);
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(1, 1);
        params.gravity = Gravity.START | Gravity.TOP;
        addContentView(readinessEditText, params);
        getWindow()
                .getDecorView()
                .setOnApplyWindowInsetsListener(
                        (view, insets) -> {
                            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                                boolean visible = insets.isVisible(WindowInsets.Type.ime());
                                if (lastImeVisible == null || lastImeVisible != visible) {
                                    lastImeVisible = visible;
                                    Log.i(TAG, "IME visible=" + visible);
                                }
                            }
                            return insets;
                        });
        Log.i(TAG, "hidden readiness EditText attached");
        return readinessEditText;
    }

    private void logStartupArgv(Intent intent) {
        if (intent == null) {
            Log.i(TAG, "startup argv intent extra unavailable: no intent");
            return;
        }
        String startupArgv = intent.getStringExtra(EXTRA_STARTUP_ARGV);
        if (startupArgv == null || startupArgv.isEmpty()) {
            Log.i(TAG, "startup argv intent extra: <none>");
        } else {
            Log.i(TAG, "startup argv intent extra: " + startupArgv);
        }
    }
}

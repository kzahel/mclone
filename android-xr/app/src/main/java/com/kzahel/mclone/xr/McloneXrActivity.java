package com.kzahel.mclone.xr;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.NativeActivity;
import android.content.Context;
import android.content.Intent;
import android.graphics.Color;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.InputType;
import android.util.Log;
import android.view.Gravity;
import android.view.WindowInsets;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import com.kzahel.mclone.controller.ControllerInputBridge;

public class McloneXrActivity extends NativeActivity {
    private static final String TAG = "McloneXrActivity";
    private static final String EXTRA_STARTUP_ARGV = "mclone.startup.argv";
    private static final String FAILURE_CHANNEL_ID = "mclone_xr_startup_failures";
    private static final int FAILURE_NOTIFICATION_ID = 1701;
    private static final long FAILURE_FINISH_DELAY_MS = 1500L;
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private EditText readinessEditText;
    private Boolean lastImeVisible;
    private boolean nativeFailureReported;
    private ControllerInputBridge controllerInput;

    static {
        System.loadLibrary("mclone_android_xr_client");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        controllerInput = new ControllerInputBridge(this);
        controllerInput.start();
        Log.i(TAG, "McloneXrActivity created");
        ensureReadinessEditText();
        logStartupArgv(getIntent());
    }

    @Override
    protected void onDestroy() {
        if (controllerInput != null) {
            controllerInput.stop();
        }
        super.onDestroy();
    }

    @Override
    public boolean dispatchKeyEvent(android.view.KeyEvent event) {
        return controllerInput != null && controllerInput.dispatchKeyEvent(event)
                || super.dispatchKeyEvent(event);
    }

    @Override
    public boolean dispatchGenericMotionEvent(android.view.MotionEvent event) {
        return controllerInput != null && controllerInput.dispatchGenericMotionEvent(event)
                || super.dispatchGenericMotionEvent(event);
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

    public void reportMcloneNativeFailure(String message) {
        final String safeMessage = sanitizeFailureMessage(message);
        synchronized (this) {
            if (nativeFailureReported) {
                return;
            }
            nativeFailureReported = true;
        }
        Log.e(TAG, "native startup failure: " + safeMessage);
        showFailureNotification(safeMessage);
        Log.e(TAG, "MCLONE_ANDROID_XR_FAILURE_SURFACED");
        mainHandler.post(() -> showNativeFailureUi(safeMessage));
        mainHandler.postDelayed(this::finishAfterNativeFailure, FAILURE_FINISH_DELAY_MS);
        startFailureExitFallback();
    }

    private void showNativeFailureUi(String message) {
        showFailureOverlay(message);
        showFailureToast(message);
    }

    private void startFailureExitFallback() {
        Thread fallback =
                new Thread(
                        () -> {
                            try {
                                Thread.sleep(FAILURE_FINISH_DELAY_MS + 1000L);
                            } catch (InterruptedException ignored) {
                                Thread.currentThread().interrupt();
                            }
                            Log.e(TAG, "MCLONE_ANDROID_XR_FAILURE_EXITING");
                            android.os.Process.killProcess(android.os.Process.myPid());
                        },
                        "mclone-xr-failure-exit");
        fallback.setDaemon(true);
        fallback.start();
    }

    private void showFailureOverlay(String message) {
        TextView text = new TextView(this);
        text.setText(
                "Mclone XR failed to start\n\n"
                        + message
                        + "\n\nThe app will close shortly. See logcat for MCLONE_ANDROID_XR_FAILURE.");
        text.setTextColor(Color.WHITE);
        text.setTextSize(18.0f);
        text.setPadding(40, 40, 40, 40);
        text.setGravity(Gravity.START);

        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Color.rgb(24, 24, 24));
        scroll.addView(
                text,
                new ScrollView.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        setContentView(scroll);
    }

    private void showFailureToast(String message) {
        Toast.makeText(
                        this,
                        "Mclone XR failed to start: " + shortFailureMessage(message),
                        Toast.LENGTH_LONG)
                .show();
    }

    private void showFailureNotification(String message) {
        try {
            NotificationManager manager =
                    (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
            if (manager == null) {
                return;
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                NotificationChannel channel =
                        new NotificationChannel(
                                FAILURE_CHANNEL_ID,
                                "Mclone XR startup failures",
                                NotificationManager.IMPORTANCE_HIGH);
                channel.setDescription("Mclone XR launch failure diagnostics");
                manager.createNotificationChannel(channel);
            }

            Notification.Builder builder =
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                            ? new Notification.Builder(this, FAILURE_CHANNEL_ID)
                            : new Notification.Builder(this);
            Notification notification =
                    builder.setSmallIcon(android.R.drawable.stat_notify_error)
                            .setContentTitle("Mclone XR failed to start")
                            .setContentText(shortFailureMessage(message))
                            .setStyle(new Notification.BigTextStyle().bigText(message))
                            .setAutoCancel(true)
                            .setShowWhen(true)
                            .build();
            manager.notify(FAILURE_NOTIFICATION_ID, notification);
        } catch (RuntimeException error) {
            Log.w(TAG, "failed to post native failure notification", error);
        }
    }

    private void finishAfterNativeFailure() {
        try {
            finishAndRemoveTask();
        } catch (RuntimeException error) {
            Log.w(TAG, "finishAndRemoveTask failed after native startup failure", error);
            finish();
        }
    }

    private String sanitizeFailureMessage(String message) {
        if (message == null) {
            return "unknown native startup failure";
        }
        String sanitized = message.replace('\0', ' ').trim();
        if (sanitized.isEmpty()) {
            return "unknown native startup failure";
        }
        if (sanitized.length() > 4000) {
            return sanitized.substring(0, 4000) + "\n... truncated ...";
        }
        return sanitized;
    }

    private String shortFailureMessage(String message) {
        String firstLine = message.split("\\R", 2)[0].trim();
        if (firstLine.length() > 160) {
            return firstLine.substring(0, 157) + "...";
        }
        return firstLine;
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

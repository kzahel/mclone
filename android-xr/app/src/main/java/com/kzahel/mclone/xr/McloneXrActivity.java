package com.kzahel.mclone.xr;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Bundle;
import android.util.Log;

public class McloneXrActivity extends NativeActivity {
    private static final String TAG = "McloneXrActivity";
    private static final String EXTRA_STARTUP_ARGV = "mclone.startup.argv";

    static {
        System.loadLibrary("mclone_android_xr_client");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Log.i(TAG, "McloneXrActivity created");
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

package com.kzahel.mclone;

import android.app.NativeActivity;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.MotionEvent;
import com.kzahel.mclone.controller.ControllerInputBridge;

/** NativeActivity shell with a source-aware controller seam ahead of winit. */
public class McloneActivity extends NativeActivity {
    private ControllerInputBridge controllerInput;

    static {
        System.loadLibrary("mclone_android_client");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        controllerInput = new ControllerInputBridge(this);
        controllerInput.start();
    }

    @Override
    protected void onDestroy() {
        if (controllerInput != null) {
            controllerInput.stop();
        }
        super.onDestroy();
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        return controllerInput != null && controllerInput.dispatchKeyEvent(event)
                || super.dispatchKeyEvent(event);
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent event) {
        return controllerInput != null && controllerInput.dispatchGenericMotionEvent(event)
                || super.dispatchGenericMotionEvent(event);
    }
}

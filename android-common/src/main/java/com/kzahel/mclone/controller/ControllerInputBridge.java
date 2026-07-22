package com.kzahel.mclone.controller;

import android.app.Activity;
import android.content.Context;
import android.hardware.input.InputManager;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;

/** Mechanical Android controller bridge shared by the flat and XR activities. */
public final class ControllerInputBridge implements InputManager.InputDeviceListener {
    private static final int DEVICE_ADDED = 0;
    private static final int DEVICE_CHANGED = 1;
    private static final int DEVICE_REMOVED = 2;

    private static final int AXIS_X = 1 << 0;
    private static final int AXIS_Y = 1 << 1;
    private static final int AXIS_Z = 1 << 2;
    private static final int AXIS_RZ = 1 << 3;
    private static final int AXIS_RX = 1 << 4;
    private static final int AXIS_RY = 1 << 5;
    private static final int AXIS_HAT_X = 1 << 6;
    private static final int AXIS_HAT_Y = 1 << 7;
    private static final int AXIS_LTRIGGER = 1 << 8;
    private static final int AXIS_RTRIGGER = 1 << 9;
    private static final int AXIS_BRAKE = 1 << 10;
    private static final int AXIS_GAS = 1 << 11;

    private final InputManager inputManager;
    private boolean started;

    public ControllerInputBridge(Activity activity) {
        inputManager = (InputManager) activity.getSystemService(Context.INPUT_SERVICE);
    }

    public void start() {
        if (started || inputManager == null) {
            return;
        }
        started = true;
        inputManager.registerInputDeviceListener(this, null);
        for (int deviceId : InputDevice.getDeviceIds()) {
            reportDevice(DEVICE_ADDED, deviceId);
        }
    }

    public void stop() {
        if (!started || inputManager == null) {
            return;
        }
        started = false;
        inputManager.unregisterInputDeviceListener(this);
    }

    public boolean dispatchKeyEvent(KeyEvent event) {
        if (!isControllerSource(event.getSource()) || !isStandardControllerKey(event.getKeyCode())) {
            return false;
        }
        reportDevice(DEVICE_CHANGED, event.getDeviceId());
        nativeControllerKey(
                event.getDeviceId(),
                event.getSource(),
                event.getKeyCode(),
                event.getAction(),
                event.getRepeatCount());
        return true;
    }

    public boolean dispatchGenericMotionEvent(MotionEvent event) {
        if (event.getActionMasked() != MotionEvent.ACTION_MOVE
                || !isControllerSource(event.getSource())) {
            return false;
        }
        reportDevice(DEVICE_CHANGED, event.getDeviceId());
        nativeControllerMotion(
                event.getDeviceId(),
                event.getSource(),
                event.getAxisValue(MotionEvent.AXIS_X),
                event.getAxisValue(MotionEvent.AXIS_Y),
                event.getAxisValue(MotionEvent.AXIS_Z),
                event.getAxisValue(MotionEvent.AXIS_RZ),
                event.getAxisValue(MotionEvent.AXIS_RX),
                event.getAxisValue(MotionEvent.AXIS_RY),
                event.getAxisValue(MotionEvent.AXIS_HAT_X),
                event.getAxisValue(MotionEvent.AXIS_HAT_Y),
                event.getAxisValue(MotionEvent.AXIS_LTRIGGER),
                event.getAxisValue(MotionEvent.AXIS_RTRIGGER),
                event.getAxisValue(MotionEvent.AXIS_BRAKE),
                event.getAxisValue(MotionEvent.AXIS_GAS));
        return true;
    }

    @Override
    public void onInputDeviceAdded(int deviceId) {
        reportDevice(DEVICE_ADDED, deviceId);
    }

    @Override
    public void onInputDeviceChanged(int deviceId) {
        reportDevice(DEVICE_CHANGED, deviceId);
    }

    @Override
    public void onInputDeviceRemoved(int deviceId) {
        nativeControllerDevice(DEVICE_REMOVED, deviceId, 0, 0, 0, 0, "");
    }

    private void reportDevice(int change, int deviceId) {
        InputDevice device = InputDevice.getDevice(deviceId);
        if (device == null || !isControllerSource(device.getSources())) {
            return;
        }
        nativeControllerDevice(
                change,
                deviceId,
                device.getSources(),
                device.getVendorId(),
                device.getProductId(),
                axisSupport(device),
                device.getName());
    }

    private static int axisSupport(InputDevice device) {
        int support = 0;
        support |= hasAxis(device, MotionEvent.AXIS_X) ? AXIS_X : 0;
        support |= hasAxis(device, MotionEvent.AXIS_Y) ? AXIS_Y : 0;
        support |= hasAxis(device, MotionEvent.AXIS_Z) ? AXIS_Z : 0;
        support |= hasAxis(device, MotionEvent.AXIS_RZ) ? AXIS_RZ : 0;
        support |= hasAxis(device, MotionEvent.AXIS_RX) ? AXIS_RX : 0;
        support |= hasAxis(device, MotionEvent.AXIS_RY) ? AXIS_RY : 0;
        support |= hasAxis(device, MotionEvent.AXIS_HAT_X) ? AXIS_HAT_X : 0;
        support |= hasAxis(device, MotionEvent.AXIS_HAT_Y) ? AXIS_HAT_Y : 0;
        support |= hasAxis(device, MotionEvent.AXIS_LTRIGGER) ? AXIS_LTRIGGER : 0;
        support |= hasAxis(device, MotionEvent.AXIS_RTRIGGER) ? AXIS_RTRIGGER : 0;
        support |= hasAxis(device, MotionEvent.AXIS_BRAKE) ? AXIS_BRAKE : 0;
        support |= hasAxis(device, MotionEvent.AXIS_GAS) ? AXIS_GAS : 0;
        return support;
    }

    private static boolean hasAxis(InputDevice device, int axis) {
        return device.getMotionRange(axis) != null;
    }

    private static boolean isControllerSource(int source) {
        return (source & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (source & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK
                || (source & InputDevice.SOURCE_DPAD) == InputDevice.SOURCE_DPAD;
    }

    private static boolean isStandardControllerKey(int keyCode) {
        return switch (keyCode) {
            case KeyEvent.KEYCODE_DPAD_UP,
                    KeyEvent.KEYCODE_DPAD_DOWN,
                    KeyEvent.KEYCODE_DPAD_LEFT,
                    KeyEvent.KEYCODE_DPAD_RIGHT,
                    KeyEvent.KEYCODE_BUTTON_A,
                    KeyEvent.KEYCODE_BUTTON_B,
                    KeyEvent.KEYCODE_BUTTON_X,
                    KeyEvent.KEYCODE_BUTTON_Y,
                    KeyEvent.KEYCODE_BUTTON_L1,
                    KeyEvent.KEYCODE_BUTTON_R1,
                    KeyEvent.KEYCODE_BUTTON_L2,
                    KeyEvent.KEYCODE_BUTTON_R2,
                    KeyEvent.KEYCODE_BUTTON_THUMBL,
                    KeyEvent.KEYCODE_BUTTON_THUMBR,
                    KeyEvent.KEYCODE_BUTTON_START,
                    KeyEvent.KEYCODE_BUTTON_SELECT,
                    KeyEvent.KEYCODE_BUTTON_MODE -> true;
            default -> false;
        };
    }

    private static native void nativeControllerDevice(
            int change,
            int deviceId,
            int sources,
            int vendorId,
            int productId,
            int axisSupport,
            String displayLabel);

    private static native void nativeControllerKey(
            int deviceId, int source, int keyCode, int action, int repeatCount);

    private static native void nativeControllerMotion(
            int deviceId,
            int source,
            float x,
            float y,
            float z,
            float rz,
            float rx,
            float ry,
            float hatX,
            float hatY,
            float leftTrigger,
            float rightTrigger,
            float brake,
            float gas);
}

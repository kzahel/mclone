//! Desktop companion window for the real `--desktop-xr` run verb.
//!
//! This is app-local desktop platform glue (winit window/event-loop ownership),
//! not engine policy: it exists so the operator has a desktop presence and a
//! non-headset way to quit. Per tactical 164 Slice 3 it is a **status surface
//! only** — no headset mirror, no per-view render path. The live status is
//! carried on the window title, and the OS window-close control (the title-bar
//! close button / Cmd-W / Alt-F4) is the "Close" affordance that drives a
//! graceful shutdown of the XR session.
//!
//! The XR frame loop owns timing, so the window is polled non-blockingly with
//! winit's `pump_app_events` (timeout `Duration::ZERO`) once per XR frame rather
//! than running its own blocking event loop.

use std::time::Duration;

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{Window, WindowId};

/// A small always-on status window for the desktop XR run mode.
pub(super) struct CompanionWindow {
    event_loop: EventLoop<()>,
    app: CompanionApp,
}

#[derive(Default)]
struct CompanionApp {
    window: Option<Window>,
    title: String,
    close_requested: bool,
}

impl ApplicationHandler for CompanionApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(460.0, 140.0))
            .with_resizable(true);
        match event_loop.create_window(attributes) {
            Ok(window) => self.window = Some(window),
            Err(err) => println!("desktop XR companion window unavailable: {err}"),
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.close_requested = true;
        }
    }
}

impl CompanionWindow {
    /// Create the event loop, spawn the window, and pump once so it is mapped
    /// before the first XR frame. Must be called on the process main thread
    /// (winit requirement on macOS); the desktop XR path already runs there.
    pub(super) fn spawn(initial_status: &str) -> Result<Self> {
        let event_loop =
            EventLoop::new().context("create desktop XR companion window event loop")?;
        // The XR frame loop drives cadence; never block inside the window pump.
        event_loop.set_control_flow(ControlFlow::Poll);
        let app = CompanionApp {
            title: initial_status.to_owned(),
            ..Default::default()
        };
        let mut window = CompanionWindow { event_loop, app };
        window.pump();
        Ok(window)
    }

    /// Update the status line shown on the window title. No-op if unchanged so
    /// we do not thrash the OS title bar every XR frame.
    pub(super) fn set_status(&mut self, status: &str) {
        if self.app.title == status {
            return;
        }
        self.app.title = status.to_owned();
        if let Some(window) = &self.app.window {
            window.set_title(status);
        }
    }

    /// Drain pending window events without blocking. Returns `true` once the
    /// user has asked to close the window (title-bar close / Cmd-W / Alt-F4).
    pub(super) fn pump(&mut self) -> bool {
        let _ = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.app);
        self.app.close_requested
    }
}

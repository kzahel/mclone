use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ViewPoint, ViewportMetrics, WorldViewIntent,
    WorldViewReducer, WorldViewSignal, WorldViewState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerrainLabNavigationSignal {
    None,
    Tap,
    DoubleTap,
    ContactsCancelled,
}

impl TerrainLabNavigationSignal {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Tap => "tap",
            Self::DoubleTap => "double-tap",
            Self::ContactsCancelled => "contacts-cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TerrainLabNavigationSnapshot {
    pub(crate) center_x: i32,
    pub(crate) center_z: i32,
    pub(crate) blocks_across: u32,
    pub(crate) yaw_radians: f64,
    pub(crate) pitch_radians: f64,
    pub(crate) view_changed: bool,
    pub(crate) signal: TerrainLabNavigationSignal,
}

pub(crate) struct TerrainLabNavigationController {
    reducer: WorldViewReducer,
    gestures: ContactGestureReducer,
    state: WorldViewState,
}

impl Default for TerrainLabNavigationController {
    fn default() -> Self {
        Self {
            reducer: WorldViewReducer::default(),
            gestures: ContactGestureReducer::default(),
            state: WorldViewState::default(),
        }
    }
}

impl TerrainLabNavigationController {
    pub(crate) fn sync(&mut self, state: WorldViewState) {
        self.state = self.reducer.normalize(state);
    }

    pub(crate) fn contact(&mut self, event: ContactEvent) -> TerrainLabNavigationSnapshot {
        let before = self.state;
        let intents = self.gestures.handle(self.state, event);
        let mut signal = TerrainLabNavigationSignal::None;
        for intent in intents {
            let reduction = self.reducer.reduce(self.state, intent);
            self.state = reduction.state;
            if let Some(next_signal) = reduction.signal {
                signal = navigation_signal(next_signal);
            }
        }
        self.snapshot(before != self.state, signal)
    }

    pub(crate) fn apply(&mut self, intent: WorldViewIntent) -> TerrainLabNavigationSnapshot {
        let before = self.state;
        let reduction = self.reducer.reduce(self.state, intent);
        self.state = reduction.state;
        self.snapshot(
            before != self.state,
            reduction
                .signal
                .map_or(TerrainLabNavigationSignal::None, navigation_signal),
        )
    }

    pub(crate) fn arrow(&mut self, key: &str) -> TerrainLabNavigationSnapshot {
        let distance = self.state.blocks_across / 8.0;
        let intent = match key {
            "ArrowUp" => Some(WorldViewIntent::PanWorld {
                delta_x: 0.0,
                delta_z: -distance,
            }),
            "ArrowDown" => Some(WorldViewIntent::PanWorld {
                delta_x: 0.0,
                delta_z: distance,
            }),
            "ArrowLeft" => Some(WorldViewIntent::PanWorld {
                delta_x: -distance,
                delta_z: 0.0,
            }),
            "ArrowRight" => Some(WorldViewIntent::PanWorld {
                delta_x: distance,
                delta_z: 0.0,
            }),
            _ => None,
        };
        match intent {
            Some(intent) => self.apply(intent),
            None => self.snapshot(false, TerrainLabNavigationSignal::None),
        }
    }

    pub(crate) fn pan_fraction(
        &mut self,
        horizontal: f64,
        depth: f64,
        fraction: f64,
    ) -> TerrainLabNavigationSnapshot {
        let distance = self.state.blocks_across * fraction;
        self.apply(WorldViewIntent::PanWorld {
            delta_x: horizontal * distance,
            delta_z: depth * distance,
        })
    }

    pub(crate) fn zoom_factor(
        &mut self,
        factor: f64,
        normalized_anchor: ViewPoint,
        viewport: ViewportMetrics,
    ) -> TerrainLabNavigationSnapshot {
        self.apply(WorldViewIntent::AnchoredZoom {
            log_delta: factor.ln(),
            normalized_anchor,
            viewport,
        })
    }

    fn snapshot(
        &self,
        view_changed: bool,
        signal: TerrainLabNavigationSignal,
    ) -> TerrainLabNavigationSnapshot {
        TerrainLabNavigationSnapshot {
            center_x: self.state.center_x_i32(),
            center_z: self.state.center_z_i32(),
            blocks_across: self.state.blocks_across_u32(),
            yaw_radians: self.state.yaw_radians,
            pitch_radians: self.state.pitch_radians,
            view_changed,
            signal,
        }
    }
}

fn navigation_signal(signal: WorldViewSignal) -> TerrainLabNavigationSignal {
    match signal {
        WorldViewSignal::Tap { .. } => TerrainLabNavigationSignal::Tap,
        WorldViewSignal::DoubleTap { .. } => TerrainLabNavigationSignal::DoubleTap,
        WorldViewSignal::ContactsCancelled => TerrainLabNavigationSignal::ContactsCancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_view_control::{WorldViewMode, WorldViewProjection};

    fn lab_state(mode: WorldViewMode) -> WorldViewState {
        WorldViewState {
            mode,
            focus_x: -304.0,
            focus_z: 336.0,
            blocks_across: 512.0,
            yaw_radians: std::f64::consts::FRAC_PI_4,
            pitch_radians: 0.48,
            projection: WorldViewProjection::Orthographic,
        }
    }

    #[test]
    fn ui_pan_and_arrow_use_shared_world_view_reducer() {
        let mut navigation = TerrainLabNavigationController::default();
        navigation.sync(lab_state(WorldViewMode::Orbit));
        let panned = navigation.pan_fraction(1.0, -1.0, 0.25);
        assert_eq!(panned.center_x, -176);
        assert_eq!(panned.center_z, 208);

        navigation.sync(lab_state(WorldViewMode::Orbit));
        let arrowed = navigation.arrow("ArrowRight");
        assert_eq!(arrowed.center_x, -240);
        assert_eq!(arrowed.center_z, 336);
        assert!(!navigation.arrow("Enter").view_changed);
    }

    #[test]
    fn map_zoom_preserves_terrain_lab_integer_facing_contract() {
        let mut navigation = TerrainLabNavigationController::default();
        navigation.sync(lab_state(WorldViewMode::Map));
        let zoomed = navigation.zoom_factor(
            0.5,
            ViewPoint::new(0.5, -0.5),
            ViewportMetrics::new(800.0, 400.0),
        );
        assert_eq!(zoomed.blocks_across, 256);
        assert_eq!(zoomed.center_x, -176);
        assert_eq!(zoomed.center_z, 272);
    }

    #[test]
    fn contact_signals_come_from_shared_gesture_reducer() {
        let mut navigation = TerrainLabNavigationController::default();
        navigation.sync(lab_state(WorldViewMode::Map));
        let viewport = ViewportMetrics::new(800.0, 600.0);
        navigation.contact(ContactEvent::Down {
            id: 7,
            position: ViewPoint::new(10.0, 10.0),
            purpose: mclone_view_control::ContactPurpose::ViewDefault,
            time_seconds: 0.0,
            viewport,
        });
        let tap = navigation.contact(ContactEvent::Up {
            id: 7,
            position: ViewPoint::new(12.0, 10.0),
            time_seconds: 0.1,
            viewport,
        });
        assert_eq!(tap.signal, TerrainLabNavigationSignal::Tap);
        assert!(!tap.view_changed);
    }
}

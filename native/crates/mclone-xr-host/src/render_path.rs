use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum XrRenderMode {
    #[default]
    DualPerEye,
    ArrayPerEye,
    ArrayMultiview,
}

impl XrRenderMode {
    pub const ALL: [Self; 3] = [Self::DualPerEye, Self::ArrayPerEye, Self::ArrayMultiview];

    pub const fn topology(self) -> XrTargetTopology {
        match self {
            Self::DualPerEye => XrTargetTopology::DualEye,
            Self::ArrayPerEye | Self::ArrayMultiview => XrTargetTopology::StereoArray,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::DualPerEye => "dual-per-eye",
            Self::ArrayPerEye => "array-per-eye",
            Self::ArrayMultiview => "array-multiview",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dual" | "dual-eye" | "dual-per-eye" | "per-eye" => Ok(Self::DualPerEye),
            "array" | "array-eye" | "array-per-eye" => Ok(Self::ArrayPerEye),
            "multiview" | "array-multiview" => Ok(Self::ArrayMultiview),
            _ => bail!(
                "unknown XR render mode {value:?}; expected dual-per-eye, array-per-eye, or array-multiview"
            ),
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::DualPerEye => 1 << 0,
            Self::ArrayPerEye => 1 << 1,
            Self::ArrayMultiview => 1 << 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XrTargetTopology {
    DualEye,
    StereoArray,
}

impl XrTargetTopology {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DualEye => "dual-eye",
            Self::StereoArray => "stereo-array",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrRenderModeSet(u8);

impl XrRenderModeSet {
    pub const NONE: Self = Self(0);
    pub const DUAL_PER_EYE: Self = Self(XrRenderMode::DualPerEye.bit());
    pub const ARRAY_PER_EYE: Self = Self(XrRenderMode::ArrayPerEye.bit());
    pub const ARRAY_MULTIVIEW: Self = Self(XrRenderMode::ArrayMultiview.bit());
    pub const ALL: Self = Self(
        XrRenderMode::DualPerEye.bit()
            | XrRenderMode::ArrayPerEye.bit()
            | XrRenderMode::ArrayMultiview.bit(),
    );

    pub const fn contains(self, mode: XrRenderMode) -> bool {
        self.0 & mode.bit() != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn iter(self) -> impl Iterator<Item = XrRenderMode> {
        XrRenderMode::ALL
            .into_iter()
            .filter(move |mode| self.contains(*mode))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrRenderTransitionState {
    #[default]
    Idle,
    Pending,
    Committed,
    RejectedUnsupported,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrRenderTransitionCounts {
    pub committed: u64,
    pub failed: u64,
    pub rejected: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XrRenderPathSnapshot {
    pub supported_modes: XrRenderModeSet,
    pub requested_mode: XrRenderMode,
    pub pending_mode: Option<XrRenderMode>,
    pub active_mode: XrRenderMode,
    pub active_topology: XrTargetTopology,
    pub transition_state: XrRenderTransitionState,
    pub counts: XrRenderTransitionCounts,
    pub active_target_bytes: u64,
    pub peak_transition_bytes: u64,
    pub outstanding_images: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrRenderModeRequest {
    Pending,
    AlreadyActive,
    RejectedUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XrRenderTransition {
    pub previous_mode: XrRenderMode,
    pub active_mode: XrRenderMode,
    pub topology_recreated: bool,
    pub retired_target_bytes: u64,
    pub active_target_bytes: u64,
    pub peak_transition_bytes: u64,
}

pub trait XrTargetFamily {
    fn topology(&self) -> XrTargetTopology;
    fn estimated_owned_bytes(&self) -> u64;

    fn outstanding_image_count(&self) -> u32 {
        0
    }
}

pub struct XrTargetManager<T> {
    active_target: Option<T>,
    supported_modes: XrRenderModeSet,
    requested_mode: XrRenderMode,
    pending_mode: Option<XrRenderMode>,
    active_mode: XrRenderMode,
    transition_state: XrRenderTransitionState,
    counts: XrRenderTransitionCounts,
    peak_transition_bytes: u64,
}

impl<T> XrTargetManager<T>
where
    T: XrTargetFamily,
{
    pub fn new(
        active_target: T,
        active_mode: XrRenderMode,
        supported_modes: XrRenderModeSet,
    ) -> Result<Self> {
        if !supported_modes.contains(active_mode) {
            bail!(
                "active XR render mode {} is absent from supported set",
                active_mode.label()
            );
        }
        if active_target.topology() != active_mode.topology() {
            bail!(
                "active XR target topology {} does not match mode {} topology {}",
                active_target.topology().label(),
                active_mode.label(),
                active_mode.topology().label()
            );
        }
        let peak_transition_bytes = active_target.estimated_owned_bytes();
        Ok(Self {
            active_target: Some(active_target),
            supported_modes,
            requested_mode: active_mode,
            pending_mode: None,
            active_mode,
            transition_state: XrRenderTransitionState::Idle,
            counts: XrRenderTransitionCounts::default(),
            peak_transition_bytes,
        })
    }

    pub const fn active_mode(&self) -> XrRenderMode {
        self.active_mode
    }

    pub fn active_target(&self) -> &T {
        self.active_target
            .as_ref()
            .expect("XR target manager has no active target")
    }

    pub fn active_target_mut(&mut self) -> &mut T {
        self.active_target
            .as_mut()
            .expect("XR target manager has no active target")
    }

    pub fn has_active_target(&self) -> bool {
        self.active_target.is_some()
    }

    pub fn snapshot(&self) -> XrRenderPathSnapshot {
        let active_target = self.active_target();
        XrRenderPathSnapshot {
            supported_modes: self.supported_modes,
            requested_mode: self.requested_mode,
            pending_mode: self.pending_mode,
            active_mode: self.active_mode,
            active_topology: active_target.topology(),
            transition_state: self.transition_state,
            counts: self.counts,
            active_target_bytes: active_target.estimated_owned_bytes(),
            peak_transition_bytes: self.peak_transition_bytes,
            outstanding_images: active_target.outstanding_image_count(),
        }
    }

    pub fn request_mode(&mut self, mode: XrRenderMode) -> XrRenderModeRequest {
        if !self.supported_modes.contains(mode) {
            self.requested_mode = self.active_mode;
            self.pending_mode = None;
            self.transition_state = XrRenderTransitionState::RejectedUnsupported;
            self.counts.rejected += 1;
            return XrRenderModeRequest::RejectedUnsupported;
        }
        self.requested_mode = mode;
        if mode == self.active_mode {
            self.pending_mode = None;
            self.transition_state = XrRenderTransitionState::Idle;
            XrRenderModeRequest::AlreadyActive
        } else {
            self.pending_mode = Some(mode);
            self.transition_state = XrRenderTransitionState::Pending;
            XrRenderModeRequest::Pending
        }
    }

    pub fn cancel_pending(&mut self) {
        self.requested_mode = self.active_mode;
        self.pending_mode = None;
        self.transition_state = XrRenderTransitionState::Idle;
    }

    pub fn apply_pending<F>(&mut self, create_target: F) -> Result<Option<XrRenderTransition>>
    where
        F: FnOnce(XrTargetTopology) -> Result<T>,
    {
        let Some(requested_mode) = self.pending_mode else {
            return Ok(None);
        };
        let previous_mode = self.active_mode;
        let requested_topology = requested_mode.topology();
        let active_topology = self.active_target().topology();
        if self.active_target().outstanding_image_count() != 0 {
            return self.fail_transition(anyhow::anyhow!(
                "cannot switch XR render mode with {} outstanding target image(s)",
                self.active_target().outstanding_image_count()
            ));
        }

        let mut topology_recreated = false;
        let mut retired_target_bytes = 0;
        let mut transition_peak_bytes = self.active_target().estimated_owned_bytes();
        if requested_topology != active_topology {
            let replacement = match create_target(requested_topology) {
                Ok(target) => target,
                Err(error) => return self.fail_transition(error),
            };
            if replacement.topology() != requested_topology {
                return self.fail_transition(anyhow::anyhow!(
                    "replacement XR target topology {} does not match requested {}",
                    replacement.topology().label(),
                    requested_topology.label()
                ));
            }
            if replacement.outstanding_image_count() != 0 {
                return self.fail_transition(anyhow::anyhow!(
                    "replacement XR target begins with {} outstanding image(s)",
                    replacement.outstanding_image_count()
                ));
            }
            retired_target_bytes = self.active_target().estimated_owned_bytes();
            transition_peak_bytes =
                retired_target_bytes.saturating_add(replacement.estimated_owned_bytes());
            let retired = self
                .active_target
                .replace(replacement)
                .expect("XR target manager replacement requires an active target");
            drop(retired);
            topology_recreated = true;
        }

        self.active_mode = requested_mode;
        self.requested_mode = requested_mode;
        self.pending_mode = None;
        self.transition_state = XrRenderTransitionState::Committed;
        self.counts.committed += 1;
        self.peak_transition_bytes = self.peak_transition_bytes.max(transition_peak_bytes);
        Ok(Some(XrRenderTransition {
            previous_mode,
            active_mode: requested_mode,
            topology_recreated,
            retired_target_bytes,
            active_target_bytes: self.active_target().estimated_owned_bytes(),
            peak_transition_bytes: transition_peak_bytes,
        }))
    }

    /// Apply a topology transition by retiring the current target before
    /// creating its replacement.
    ///
    /// Some OpenXR runtimes cannot safely keep two swapchain families alive at
    /// once. This path lowers peak residency and, if replacement creation
    /// fails, recreates the previous topology before returning the error.
    pub fn apply_pending_retire_first<F>(
        &mut self,
        mut create_target: F,
    ) -> Result<Option<XrRenderTransition>>
    where
        F: FnMut(XrTargetTopology) -> Result<T>,
    {
        let Some(requested_mode) = self.pending_mode else {
            return Ok(None);
        };
        let previous_mode = self.active_mode;
        let previous_topology = self.active_target().topology();
        let requested_topology = requested_mode.topology();
        if requested_topology == previous_topology {
            return self.apply_pending(create_target);
        }
        if self.active_target().outstanding_image_count() != 0 {
            return self.fail_transition(anyhow::anyhow!(
                "cannot retire XR target with {} outstanding image(s)",
                self.active_target().outstanding_image_count()
            ));
        }

        let retired = self
            .active_target
            .take()
            .expect("retire-first XR transition requires an active target");
        let retired_target_bytes = retired.estimated_owned_bytes();
        drop(retired);

        let replacement_result = create_target(requested_topology)
            .and_then(|replacement| validate_replacement_target(replacement, requested_topology));
        match replacement_result {
            Ok(replacement) => {
                let active_target_bytes = replacement.estimated_owned_bytes();
                let transition_peak_bytes = retired_target_bytes.max(active_target_bytes);
                self.active_target = Some(replacement);
                self.active_mode = requested_mode;
                self.requested_mode = requested_mode;
                self.pending_mode = None;
                self.transition_state = XrRenderTransitionState::Committed;
                self.counts.committed += 1;
                self.peak_transition_bytes = self.peak_transition_bytes.max(transition_peak_bytes);
                Ok(Some(XrRenderTransition {
                    previous_mode,
                    active_mode: requested_mode,
                    topology_recreated: true,
                    retired_target_bytes,
                    active_target_bytes,
                    peak_transition_bytes: transition_peak_bytes,
                }))
            }
            Err(replacement_error) => {
                let recovery_result = create_target(previous_topology).and_then(|recovered| {
                    validate_replacement_target(recovered, previous_topology)
                });
                self.requested_mode = previous_mode;
                self.pending_mode = None;
                self.transition_state = XrRenderTransitionState::Failed;
                self.counts.failed += 1;
                match recovery_result {
                    Ok(recovered) => {
                        self.active_target = Some(recovered);
                        Err(replacement_error.context(
                            "replacement creation failed; previous XR topology was recovered",
                        ))
                    }
                    Err(recovery_error) => Err(anyhow::anyhow!(
                        "replacement creation failed: {replacement_error:#}; previous XR topology recovery also failed: {recovery_error:#}"
                    )),
                }
            }
        }
    }

    fn fail_transition<R>(&mut self, error: anyhow::Error) -> Result<R> {
        self.requested_mode = self.active_mode;
        self.pending_mode = None;
        self.transition_state = XrRenderTransitionState::Failed;
        self.counts.failed += 1;
        Err(error)
    }
}

fn validate_replacement_target<T>(replacement: T, requested_topology: XrTargetTopology) -> Result<T>
where
    T: XrTargetFamily,
{
    if replacement.topology() != requested_topology {
        bail!(
            "replacement XR target topology {} does not match requested {}",
            replacement.topology().label(),
            requested_topology.label()
        );
    }
    if replacement.outstanding_image_count() != 0 {
        bail!(
            "replacement XR target begins with {} outstanding image(s)",
            replacement.outstanding_image_count()
        );
    }
    Ok(replacement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct FakeTarget {
        topology: XrTargetTopology,
        bytes: u64,
        outstanding: u32,
        drops: Arc<AtomicUsize>,
    }

    impl FakeTarget {
        fn new(topology: XrTargetTopology, bytes: u64, drops: Arc<AtomicUsize>) -> Self {
            Self {
                topology,
                bytes,
                outstanding: 0,
                drops,
            }
        }
    }

    impl Drop for FakeTarget {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    impl XrTargetFamily for FakeTarget {
        fn topology(&self) -> XrTargetTopology {
            self.topology
        }

        fn estimated_owned_bytes(&self) -> u64 {
            self.bytes
        }

        fn outstanding_image_count(&self) -> u32 {
            self.outstanding
        }
    }

    #[test]
    fn same_topology_switch_reuses_target() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::StereoArray, 90, drops.clone());
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::ArrayPerEye, XrRenderModeSet::ALL).unwrap();
        assert_eq!(
            manager.request_mode(XrRenderMode::ArrayMultiview),
            XrRenderModeRequest::Pending
        );
        let transition = manager
            .apply_pending(|_| panic!("same-topology switch must not create a target"))
            .unwrap()
            .unwrap();
        assert!(!transition.topology_recreated);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        assert_eq!(manager.active_mode(), XrRenderMode::ArrayMultiview);
    }

    #[test]
    fn topology_switch_retires_old_family_before_return() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops.clone());
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        let transition = manager
            .apply_pending(|topology| Ok(FakeTarget::new(topology, 92, drops.clone())))
            .unwrap()
            .unwrap();
        assert!(transition.topology_recreated);
        assert_eq!(transition.peak_transition_bytes, 182);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(
            manager.snapshot().active_topology,
            XrTargetTopology::StereoArray
        );
    }

    #[test]
    fn creation_failure_preserves_old_family() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops.clone());
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayMultiview);
        assert!(
            manager
                .apply_pending(|_| anyhow::bail!("injected failure"))
                .is_err()
        );
        let snapshot = manager.snapshot();
        assert_eq!(snapshot.active_mode, XrRenderMode::DualPerEye);
        assert_eq!(snapshot.requested_mode, XrRenderMode::DualPerEye);
        assert_eq!(snapshot.counts.failed, 1);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn latest_request_coalesces_and_active_request_cancels() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops);
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        manager.request_mode(XrRenderMode::ArrayMultiview);
        assert_eq!(
            manager.snapshot().pending_mode,
            Some(XrRenderMode::ArrayMultiview)
        );
        assert_eq!(
            manager.request_mode(XrRenderMode::DualPerEye),
            XrRenderModeRequest::AlreadyActive
        );
        assert_eq!(manager.snapshot().pending_mode, None);
    }

    #[test]
    fn unsupported_request_is_rejected_before_creation() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops);
        let mut manager = XrTargetManager::new(
            target,
            XrRenderMode::DualPerEye,
            XrRenderModeSet::DUAL_PER_EYE,
        )
        .unwrap();
        assert_eq!(
            manager.request_mode(XrRenderMode::ArrayPerEye),
            XrRenderModeRequest::RejectedUnsupported
        );
        assert!(manager.apply_pending(|_| unreachable!()).unwrap().is_none());
        assert_eq!(manager.snapshot().counts.rejected, 1);
    }

    #[test]
    fn outstanding_image_blocks_topology_replacement() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops);
        target.outstanding = 1;
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        assert!(manager.apply_pending(|_| unreachable!()).is_err());
        assert_eq!(manager.snapshot().counts.failed, 1);
    }

    #[test]
    fn retire_first_drops_old_family_before_creating_replacement() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops.clone());
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        let transition = manager
            .apply_pending_retire_first(|topology| {
                assert_eq!(drops.load(Ordering::Relaxed), 1);
                Ok(FakeTarget::new(topology, 92, drops.clone()))
            })
            .unwrap()
            .unwrap();
        assert!(transition.topology_recreated);
        assert_eq!(transition.peak_transition_bytes, 92);
        assert_eq!(manager.active_mode(), XrRenderMode::ArrayPerEye);
    }

    #[test]
    fn retire_first_failure_recovers_previous_topology() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops.clone());
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        assert!(
            manager
                .apply_pending_retire_first(|topology| {
                    if topology == XrTargetTopology::StereoArray {
                        anyhow::bail!("injected replacement failure");
                    }
                    Ok(FakeTarget::new(topology, 90, drops.clone()))
                })
                .is_err()
        );
        assert!(manager.has_active_target());
        assert_eq!(manager.active_mode(), XrRenderMode::DualPerEye);
        assert_eq!(manager.snapshot().counts.failed, 1);
    }

    #[test]
    fn retire_first_reports_unrecoverable_empty_state() {
        let drops = Arc::new(AtomicUsize::new(0));
        let target = FakeTarget::new(XrTargetTopology::DualEye, 90, drops);
        let mut manager =
            XrTargetManager::new(target, XrRenderMode::DualPerEye, XrRenderModeSet::ALL).unwrap();
        manager.request_mode(XrRenderMode::ArrayPerEye);
        assert!(
            manager
                .apply_pending_retire_first(|_| anyhow::bail!("injected creation failure"))
                .is_err()
        );
        assert!(!manager.has_active_target());
    }
}

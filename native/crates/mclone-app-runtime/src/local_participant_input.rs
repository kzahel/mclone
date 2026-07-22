use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    time::Duration,
};

use mclone_input::{
    ControllerInputError, ControllerInputPreferences, ControllerInputSession,
    DEFAULT_INPUT_RECONNECT_GRACE, GamepadSourceSampleReceipt, InputContext, InputSourceDescriptor,
    InputSourceId, LocalGamepadAssignmentReducer, LocalInputAssignmentError, LocalParticipantSlot,
    PlayerActionFrame, SourceConnectOutcome, SourceDisconnectReceipt, SourceJoinOutcome,
    SourceReservationExpiry, StandardGamepadSnapshot,
};

use crate::local_participant::{
    LocalParticipantAdmission, LocalParticipantGroup, LocalParticipantGroupError,
    LocalParticipantId,
};

#[derive(Clone, Debug)]
struct LocalParticipantInputState {
    controller: ControllerInputSession,
}

impl LocalParticipantInputState {
    fn new(preferences: &ControllerInputPreferences) -> Self {
        Self {
            controller: ControllerInputSession::with_preferences(preferences),
        }
    }
}

/// One participant's semantic input at a shared presentation boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalParticipantActionFrame {
    pub participant: LocalParticipantAdmission,
    /// Assigned source, including one temporarily disconnected but reserved.
    pub assigned_source: Option<InputSourceId>,
    pub actions: PlayerActionFrame,
}

/// Explicit source-assignment and participant-action facts from one sample.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalParticipantInputFrame {
    pub source_receipts: Vec<GamepadSourceSampleReceipt>,
    pub participant_actions: Vec<LocalParticipantActionFrame>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalParticipantInputError {
    Assignment(LocalInputAssignmentError),
    Participant(LocalParticipantGroupError),
    Controller(ControllerInputError),
    MissingParticipant(LocalParticipantSlot),
}

impl fmt::Display for LocalParticipantInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assignment(error) => {
                write!(formatter, "local input assignment failed: {error:?}")
            }
            Self::Participant(error) => {
                write!(formatter, "participant admission failed: {error:?}")
            }
            Self::Controller(error) => write!(formatter, "participant input failed: {error}"),
            Self::MissingParticipant(slot) => {
                write!(
                    formatter,
                    "assigned slot {} has no participant",
                    slot.index()
                )
            }
        }
    }
}

impl Error for LocalParticipantInputError {}

impl From<LocalInputAssignmentError> for LocalParticipantInputError {
    fn from(error: LocalInputAssignmentError) -> Self {
        Self::Assignment(error)
    }
}

impl From<LocalParticipantGroupError> for LocalParticipantInputError {
    fn from(error: LocalParticipantGroupError) -> Self {
        Self::Participant(error)
    }
}

impl From<ControllerInputError> for LocalParticipantInputError {
    fn from(error: ControllerInputError) -> Self {
        Self::Controller(error)
    }
}

/// Shared source-to-participant assignment and semantic input owner.
///
/// A source's first join edge admits the participant for that stable slot. The
/// edge and every other held control remain suppressed until that source
/// produces a fully neutral sample. Participant records survive source
/// reservation expiry so a later source can occupy the vacant input slot
/// without manufacturing a new participant identity.
#[derive(Clone, Debug)]
pub struct LocalParticipantInputGroup {
    preferences: ControllerInputPreferences,
    assignments: LocalGamepadAssignmentReducer,
    participants: LocalParticipantGroup<LocalParticipantInputState>,
    awaiting_neutral: BTreeSet<InputSourceId>,
}

impl Default for LocalParticipantInputGroup {
    fn default() -> Self {
        Self::new(&ControllerInputPreferences::default())
    }
}

impl LocalParticipantInputGroup {
    pub fn new(preferences: &ControllerInputPreferences) -> Self {
        Self::with_reconnect_grace(preferences, DEFAULT_INPUT_RECONNECT_GRACE)
    }

    pub fn with_reconnect_grace(
        preferences: &ControllerInputPreferences,
        reconnect_grace: Duration,
    ) -> Self {
        Self {
            preferences: preferences.clone(),
            assignments: LocalGamepadAssignmentReducer::new(reconnect_grace),
            participants: LocalParticipantGroup::new(),
            awaiting_neutral: BTreeSet::new(),
        }
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn participant_at(&self, slot: LocalParticipantSlot) -> Option<LocalParticipantAdmission> {
        self.participants
            .id_at(slot)
            .map(|id| LocalParticipantAdmission { id, slot })
    }

    pub fn participant_for_source(
        &self,
        source_id: InputSourceId,
    ) -> Option<LocalParticipantAdmission> {
        self.assignments
            .slot_for_source(source_id)
            .and_then(|slot| self.participant_at(slot))
    }

    pub fn source_for_participant(
        &self,
        participant_id: LocalParticipantId,
    ) -> Option<InputSourceId> {
        self.participants
            .slot_of(participant_id)
            .and_then(|slot| self.assignments.source_for_slot(slot))
    }

    pub fn connect_source(
        &mut self,
        source_id: InputSourceId,
        descriptor: InputSourceDescriptor,
        now: Duration,
    ) -> Result<SourceConnectOutcome, LocalParticipantInputError> {
        let outcome = self.assignments.connect_source(source_id, descriptor, now);
        if let SourceConnectOutcome::Reconnected { slot: Some(slot) } = outcome {
            self.connect_assigned_source(slot, source_id)?;
            self.awaiting_neutral.insert(source_id);
        }
        Ok(outcome)
    }

    pub fn disconnect_source(
        &mut self,
        source_id: InputSourceId,
        now: Duration,
    ) -> Result<SourceDisconnectReceipt, LocalParticipantInputError> {
        let receipt = self.assignments.disconnect_source(source_id, now)?;
        self.awaiting_neutral.remove(&source_id);
        if let Some(slot) = receipt.reserved_slot {
            let participant = self
                .participants
                .get_at_mut(slot)
                .ok_or(LocalParticipantInputError::MissingParticipant(slot))?;
            match participant.controller.disconnect_source(source_id) {
                Ok(_) | Err(ControllerInputError::UnknownSource(_))
                    if !receipt.held_state_cleared => {}
                Ok(_) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(receipt)
    }

    pub fn expire_reconnect_grace(&mut self, now: Duration) -> Vec<SourceReservationExpiry> {
        self.assignments.expire_reconnect_grace(now)
    }

    pub fn set_context(
        &mut self,
        participant_id: LocalParticipantId,
        context: InputContext,
    ) -> bool {
        let Some(participant) = self.participants.get_mut(participant_id) else {
            return false;
        };
        participant.controller.set_context(context);
        true
    }

    pub fn apply_preferences(&mut self, preferences: &ControllerInputPreferences) {
        self.preferences = preferences.clone();
        let ids = self
            .participants
            .iter()
            .map(|(admission, _)| admission.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.participants
                .get_mut(id)
                .expect("collected participant remains present")
                .controller
                .apply_preferences(preferences);
        }
    }

    pub fn remove_participant(&mut self, participant_id: LocalParticipantId) -> bool {
        let Some(slot) = self.participants.slot_of(participant_id) else {
            return false;
        };
        if let Some(source_id) = self.assignments.source_for_slot(slot) {
            self.assignments.forget_source(source_id);
            self.awaiting_neutral.remove(&source_id);
        }
        self.participants.remove(participant_id).is_some()
    }

    pub fn sample_frame(
        &mut self,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
    ) -> Result<LocalParticipantInputFrame, LocalParticipantInputError> {
        let samples = samples.into_iter().collect::<Vec<_>>();
        let receipts = self.assignments.sample_frame(samples.iter().copied())?;

        for receipt in &receipts {
            if let SourceJoinOutcome::Assigned(slot) = receipt.join_outcome {
                if self.participants.id_at(slot).is_none() {
                    self.participants
                        .admit_at(slot, LocalParticipantInputState::new(&self.preferences))?;
                }
                self.connect_assigned_source(slot, receipt.source_id)?;
                self.awaiting_neutral.insert(receipt.source_id);
            }
        }

        let sampled = samples.into_iter().collect::<BTreeMap<_, _>>();
        let admissions = self
            .participants
            .iter()
            .map(|(admission, _)| admission)
            .collect::<Vec<_>>();
        let mut participant_actions = Vec::with_capacity(admissions.len());
        for admission in admissions {
            let assigned_source = self.assignments.source_for_slot(admission.slot);
            let sample = assigned_source.and_then(|source_id| {
                sampled
                    .get(&source_id)
                    .copied()
                    .map(|snapshot| (source_id, snapshot))
            });
            let samples_for_participant = match sample {
                Some((source_id, snapshot)) if self.awaiting_neutral.contains(&source_id) => {
                    if snapshot.has_held_state() {
                        Vec::new()
                    } else {
                        self.awaiting_neutral.remove(&source_id);
                        vec![(source_id, snapshot)]
                    }
                }
                Some(sample) => vec![sample],
                None => Vec::new(),
            };
            let actions = self
                .participants
                .get_mut(admission.id)
                .expect("collected participant remains present")
                .controller
                .sample_frame(now, samples_for_participant)?;
            participant_actions.push(LocalParticipantActionFrame {
                participant: admission,
                assigned_source,
                actions,
            });
        }

        Ok(LocalParticipantInputFrame {
            source_receipts: receipts,
            participant_actions,
        })
    }

    fn connect_assigned_source(
        &mut self,
        slot: LocalParticipantSlot,
        source_id: InputSourceId,
    ) -> Result<(), LocalParticipantInputError> {
        let descriptor = self
            .assignments
            .source_descriptor(source_id)
            .cloned()
            .ok_or(LocalInputAssignmentError::UnknownSource(source_id))?;
        let participant = self
            .participants
            .get_at_mut(slot)
            .ok_or(LocalParticipantInputError::MissingParticipant(slot))?;
        participant.controller.connect_source(source_id, descriptor);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;
    use mclone_input::{
        InputSourceIdAllocator, MAX_LOCAL_PARTICIPANTS, PlayerAction, StandardGamepadButtonState,
        StandardGamepadButtons,
    };

    use super::*;

    fn join_snapshot() -> StandardGamepadSnapshot {
        StandardGamepadSnapshot {
            left_stick: Vec2::new(0.75, 0.0),
            buttons: StandardGamepadButtons {
                start: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        }
    }

    fn connect_sources(group: &mut LocalParticipantInputGroup, count: usize) -> Vec<InputSourceId> {
        let mut ids = InputSourceIdAllocator::new();
        (0..count)
            .map(|index| {
                let source_id = ids.allocate().unwrap();
                assert_eq!(
                    group
                        .connect_source(
                            source_id,
                            InputSourceDescriptor::scripted_gamepad(format!("pad {index}")),
                            Duration::ZERO,
                        )
                        .unwrap(),
                    SourceConnectOutcome::Connected
                );
                source_id
            })
            .collect()
    }

    fn actions_for(
        frame: &LocalParticipantInputFrame,
        participant_id: LocalParticipantId,
    ) -> &PlayerActionFrame {
        &frame
            .participant_actions
            .iter()
            .find(|frame| frame.participant.id == participant_id)
            .expect("participant action frame")
            .actions
    }

    #[test]
    fn two_reordered_sources_route_to_isolated_semantic_sessions() {
        let mut group = LocalParticipantInputGroup::default();
        let sources = connect_sources(&mut group, 2);
        let joined = group
            .sample_frame(
                Duration::ZERO,
                [(sources[1], join_snapshot()), (sources[0], join_snapshot())],
            )
            .unwrap();
        assert_eq!(group.participant_count(), 2);
        assert!(
            joined
                .participant_actions
                .iter()
                .all(|participant| participant.actions.is_idle())
        );
        let first = group.participant_for_source(sources[0]).unwrap();
        let second = group.participant_for_source(sources[1]).unwrap();
        assert_eq!(first.slot.index(), 0);
        assert_eq!(second.slot.index(), 1);

        let still_held = group
            .sample_frame(
                Duration::from_millis(1),
                [(sources[0], join_snapshot()), (sources[1], join_snapshot())],
            )
            .unwrap();
        assert!(
            still_held
                .participant_actions
                .iter()
                .all(|participant| participant.actions.is_idle())
        );
        group
            .sample_frame(
                Duration::from_millis(2),
                [
                    (sources[1], StandardGamepadSnapshot::default()),
                    (sources[0], StandardGamepadSnapshot::default()),
                ],
            )
            .unwrap();

        assert!(group.set_context(second.id, InputContext::Menu));
        let frame = group
            .sample_frame(
                Duration::from_millis(3),
                [
                    (
                        sources[1],
                        StandardGamepadSnapshot {
                            buttons: StandardGamepadButtons {
                                dpad_up: StandardGamepadButtonState::pressed(),
                                ..StandardGamepadButtons::default()
                            },
                            ..StandardGamepadSnapshot::default()
                        },
                    ),
                    (
                        sources[0],
                        StandardGamepadSnapshot {
                            left_stick: Vec2::new(0.8, 0.0),
                            ..StandardGamepadSnapshot::default()
                        },
                    ),
                ],
            )
            .unwrap();
        assert_ne!(actions_for(&frame, first.id).movement.left, 0.0);
        assert!(
            !actions_for(&frame, first.id)
                .held
                .contains(&PlayerAction::UiNavigateUp)
        );
        assert_eq!(actions_for(&frame, second.id).movement.left, 0.0);
        assert!(
            actions_for(&frame, second.id)
                .pressed
                .contains(&PlayerAction::UiNavigateUp)
        );
    }

    #[test]
    fn four_sources_are_bounded_and_do_not_leak_held_edges() {
        let mut group = LocalParticipantInputGroup::default();
        let sources = connect_sources(&mut group, MAX_LOCAL_PARTICIPANTS + 1);
        group
            .sample_frame(
                Duration::ZERO,
                sources[..MAX_LOCAL_PARTICIPANTS]
                    .iter()
                    .rev()
                    .copied()
                    .map(|source| (source, join_snapshot())),
            )
            .unwrap();
        group
            .sample_frame(
                Duration::from_millis(1),
                sources[..MAX_LOCAL_PARTICIPANTS]
                    .iter()
                    .copied()
                    .map(|source| (source, StandardGamepadSnapshot::default())),
            )
            .unwrap();
        assert_eq!(group.participant_count(), MAX_LOCAL_PARTICIPANTS);

        let full = group
            .sample_frame(Duration::from_millis(2), [(sources[4], join_snapshot())])
            .unwrap();
        assert_eq!(
            full.source_receipts[0].join_outcome,
            SourceJoinOutcome::Full
        );

        let owner = group.participant_for_source(sources[2]).unwrap();
        let held = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                right_trigger: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        let frame = group
            .sample_frame(Duration::from_millis(3), [(sources[2], held)])
            .unwrap();
        for participant in &frame.participant_actions {
            assert_eq!(
                participant.actions.held.contains(&PlayerAction::Attack),
                participant.participant.id == owner.id
            );
            assert_eq!(
                participant.actions.pressed.contains(&PlayerAction::Attack),
                participant.participant.id == owner.id
            );
        }
    }

    #[test]
    fn disconnect_releases_only_owner_and_reconnect_preserves_participant() {
        let mut group = LocalParticipantInputGroup::with_reconnect_grace(
            &ControllerInputPreferences::default(),
            Duration::from_secs(5),
        );
        let sources = connect_sources(&mut group, 3);
        group
            .sample_frame(
                Duration::ZERO,
                sources[..2]
                    .iter()
                    .copied()
                    .map(|source| (source, join_snapshot())),
            )
            .unwrap();
        group
            .sample_frame(
                Duration::from_millis(1),
                sources[..2]
                    .iter()
                    .copied()
                    .map(|source| (source, StandardGamepadSnapshot::default())),
            )
            .unwrap();
        let first = group.participant_for_source(sources[0]).unwrap();
        let second = group.participant_for_source(sources[1]).unwrap();

        let held = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                right_trigger: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        group
            .sample_frame(Duration::from_secs(1), [(sources[0], held)])
            .unwrap();
        let disconnected = group
            .disconnect_source(sources[0], Duration::from_secs(2))
            .unwrap();
        assert_eq!(disconnected.reserved_slot, Some(first.slot));
        let released = group
            .sample_frame(Duration::from_secs(2), std::iter::empty())
            .unwrap();
        assert!(
            actions_for(&released, first.id)
                .released
                .contains(&PlayerAction::Attack)
        );
        assert!(actions_for(&released, second.id).released.is_empty());

        assert_eq!(
            group
                .connect_source(
                    sources[0],
                    InputSourceDescriptor::scripted_gamepad("reconnected"),
                    Duration::from_secs(4),
                )
                .unwrap(),
            SourceConnectOutcome::Reconnected {
                slot: Some(first.slot)
            }
        );
        assert_eq!(group.participant_for_source(sources[0]), Some(first));
        let suppressed = group
            .sample_frame(Duration::from_secs(4), [(sources[0], held)])
            .unwrap();
        assert!(actions_for(&suppressed, first.id).is_idle());
        group
            .sample_frame(
                Duration::from_secs(5),
                [(sources[0], StandardGamepadSnapshot::default())],
            )
            .unwrap();
        let resumed = group
            .sample_frame(Duration::from_secs(6), [(sources[0], held)])
            .unwrap();
        assert!(
            actions_for(&resumed, first.id)
                .pressed
                .contains(&PlayerAction::Attack)
        );

        group
            .disconnect_source(sources[0], Duration::from_secs(7))
            .unwrap();
        group
            .sample_frame(Duration::from_secs(7), std::iter::empty())
            .unwrap();
        let expired = group.expire_reconnect_grace(Duration::from_secs(12));
        assert_eq!(expired[0].released_slot, Some(first.slot));
        let replacement = group
            .sample_frame(Duration::from_secs(13), [(sources[2], join_snapshot())])
            .unwrap();
        assert_eq!(
            replacement.source_receipts[0].join_outcome,
            SourceJoinOutcome::Assigned(first.slot)
        );
        assert_eq!(group.participant_for_source(sources[2]), Some(first));
        assert_eq!(group.participant_count(), 2);
    }
}

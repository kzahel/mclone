#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderWorkerPriority {
    Active,
    Standby,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenderWorkerRequest {
    pub(crate) generation: u64,
    pub(crate) broker_request_id: u64,
    pub(crate) client_request_id: u32,
    pub(crate) world_instance_id: u64,
    pub(crate) priority: RenderWorkerPriority,
    pub(crate) enqueued_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderWorkerLifecycle {
    Starting,
    Ready,
    Failed,
    Terminated,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RenderWorkerActions {
    pub(crate) dispatch: Option<RenderWorkerRequest>,
    pub(crate) failed: Vec<RenderWorkerRequest>,
    pub(crate) release_worlds: Vec<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RenderWorkerCompletion {
    pub(crate) accepted: bool,
    pub(crate) stale: bool,
    pub(crate) actions: RenderWorkerActions,
}

#[derive(Debug)]
pub(crate) struct RenderWorkerCoordinatorState {
    generation: u64,
    lifecycle: RenderWorkerLifecycle,
    next_broker_request_id: u64,
    active: Option<RenderWorkerRequest>,
    queued: VecDeque<RenderWorkerRequest>,
    pending_world_releases: BTreeSet<u64>,
    stale_completion_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenderWorkerGeneration {
    pub(crate) asset_epoch: u64,
    pub(crate) worker_generation: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RenderWorkerGenerationActions {
    pub(crate) activated: Option<RenderWorkerGeneration>,
    pub(crate) restored: Option<RenderWorkerGeneration>,
    pub(crate) terminate_generation: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct RenderWorkerAssetSwapState {
    active: RenderWorkerGeneration,
    candidate: Option<(RenderWorkerGeneration, bool)>,
    previous: Option<RenderWorkerGeneration>,
    next_worker_generation: u64,
}

impl RenderWorkerAssetSwapState {
    pub(crate) fn new(asset_epoch: u64, worker_generation: u64) -> Self {
        assert!(
            worker_generation > 0,
            "render worker generation must be nonzero"
        );
        Self {
            active: RenderWorkerGeneration {
                asset_epoch,
                worker_generation,
            },
            candidate: None,
            previous: None,
            next_worker_generation: worker_generation.wrapping_add(1).max(1),
        }
    }

    pub(crate) const fn active(&self) -> RenderWorkerGeneration {
        self.active
    }

    pub(crate) fn begin_candidate(&mut self, asset_epoch: u64) -> RenderWorkerGeneration {
        assert!(
            self.candidate.is_none() && self.previous.is_none(),
            "render worker asset replacement already in progress"
        );
        assert!(
            asset_epoch > self.active.asset_epoch,
            "render worker candidate epoch must advance"
        );
        let generation = RenderWorkerGeneration {
            asset_epoch,
            worker_generation: self.allocate_worker_generation(),
        };
        self.candidate = Some((generation, false));
        generation
    }

    pub(crate) fn mark_candidate_ready(&mut self, worker_generation: u64) -> bool {
        let Some((generation, ready)) = self.candidate.as_mut() else {
            return false;
        };
        if generation.worker_generation != worker_generation {
            return false;
        }
        *ready = true;
        true
    }

    pub(crate) fn activate_candidate(&mut self, asset_epoch: u64) -> RenderWorkerGenerationActions {
        let Some((candidate, ready)) = self.candidate else {
            return RenderWorkerGenerationActions::default();
        };
        if !ready || candidate.asset_epoch != asset_epoch {
            return RenderWorkerGenerationActions::default();
        }
        self.candidate = None;
        self.previous = Some(self.active);
        self.active = candidate;
        RenderWorkerGenerationActions {
            activated: Some(candidate),
            ..RenderWorkerGenerationActions::default()
        }
    }

    pub(crate) fn commit(&mut self, asset_epoch: u64) -> RenderWorkerGenerationActions {
        if self.active.asset_epoch != asset_epoch {
            return RenderWorkerGenerationActions::default();
        }
        RenderWorkerGenerationActions {
            terminate_generation: self
                .previous
                .take()
                .map(|generation| generation.worker_generation),
            ..RenderWorkerGenerationActions::default()
        }
    }

    pub(crate) fn rollback(&mut self, asset_epoch: u64) -> RenderWorkerGenerationActions {
        if let Some((candidate, _)) = self.candidate
            && candidate.asset_epoch == asset_epoch
        {
            self.candidate = None;
            return RenderWorkerGenerationActions {
                terminate_generation: Some(candidate.worker_generation),
                ..RenderWorkerGenerationActions::default()
            };
        }
        if self.active.asset_epoch != asset_epoch {
            return RenderWorkerGenerationActions::default();
        }
        let Some(previous) = self.previous.take() else {
            return RenderWorkerGenerationActions::default();
        };
        let failed = self.active;
        self.active = previous;
        RenderWorkerGenerationActions {
            restored: Some(previous),
            terminate_generation: Some(failed.worker_generation),
            ..RenderWorkerGenerationActions::default()
        }
    }

    fn allocate_worker_generation(&mut self) -> u64 {
        let generation = self.next_worker_generation;
        self.next_worker_generation = self.next_worker_generation.wrapping_add(1).max(1);
        generation
    }
}

impl RenderWorkerCoordinatorState {
    pub(crate) fn new(generation: u64) -> Self {
        assert!(generation > 0, "render worker generation must be nonzero");
        Self {
            generation,
            lifecycle: RenderWorkerLifecycle::Starting,
            next_broker_request_id: 1,
            active: None,
            queued: VecDeque::new(),
            pending_world_releases: BTreeSet::new(),
            stale_completion_count: 0,
        }
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn lifecycle(&self) -> RenderWorkerLifecycle {
        self.lifecycle
    }

    pub(crate) fn enqueue(
        &mut self,
        client_request_id: u32,
        world_instance_id: u64,
        priority: RenderWorkerPriority,
        enqueued_at_ms: u64,
    ) -> (RenderWorkerRequest, RenderWorkerActions) {
        assert!(client_request_id > 0, "client request id must be nonzero");
        assert!(world_instance_id > 0, "world instance id must be nonzero");
        assert!(
            !matches!(
                self.lifecycle,
                RenderWorkerLifecycle::Failed | RenderWorkerLifecycle::Terminated
            ),
            "cannot enqueue work on an unavailable render worker"
        );
        let request = RenderWorkerRequest {
            generation: self.generation,
            broker_request_id: self.allocate_broker_request_id(),
            client_request_id,
            world_instance_id,
            priority,
            enqueued_at_ms,
        };
        self.queued.push_back(request.clone());
        let actions = self.pump();
        (request, actions)
    }

    pub(crate) fn mark_ready(&mut self, generation: u64) -> RenderWorkerActions {
        if generation != self.generation || self.lifecycle != RenderWorkerLifecycle::Starting {
            self.stale_completion_count = self.stale_completion_count.saturating_add(1);
            return RenderWorkerActions::default();
        }
        self.lifecycle = RenderWorkerLifecycle::Ready;
        self.pump()
    }

    pub(crate) fn complete(
        &mut self,
        generation: u64,
        broker_request_id: u64,
    ) -> RenderWorkerCompletion {
        if generation != self.generation
            || self
                .active
                .as_ref()
                .map(|request| request.broker_request_id)
                != Some(broker_request_id)
        {
            self.stale_completion_count = self.stale_completion_count.saturating_add(1);
            return RenderWorkerCompletion {
                stale: true,
                ..RenderWorkerCompletion::default()
            };
        }
        self.active = None;
        let mut actions = self.pump();
        actions.release_worlds.extend(self.take_releasable_worlds());
        RenderWorkerCompletion {
            accepted: true,
            stale: false,
            actions,
        }
    }

    pub(crate) fn request_world_release(&mut self, world_instance_id: u64) -> Vec<u64> {
        assert!(world_instance_id > 0, "world instance id must be nonzero");
        self.pending_world_releases.insert(world_instance_id);
        self.take_releasable_worlds()
    }

    pub(crate) fn set_world_priority(
        &mut self,
        world_instance_id: u64,
        priority: RenderWorkerPriority,
    ) {
        for request in &mut self.queued {
            if request.world_instance_id == world_instance_id {
                request.priority = priority;
            }
        }
    }

    pub(crate) fn fail(&mut self, generation: u64) -> RenderWorkerActions {
        if generation != self.generation
            || matches!(
                self.lifecycle,
                RenderWorkerLifecycle::Failed | RenderWorkerLifecycle::Terminated
            )
        {
            self.stale_completion_count = self.stale_completion_count.saturating_add(1);
            return RenderWorkerActions::default();
        }
        self.lifecycle = RenderWorkerLifecycle::Failed;
        self.drain_unavailable()
    }

    pub(crate) fn terminate(&mut self) -> RenderWorkerActions {
        if self.lifecycle == RenderWorkerLifecycle::Terminated {
            return RenderWorkerActions::default();
        }
        self.lifecycle = RenderWorkerLifecycle::Terminated;
        self.drain_unavailable()
    }

    pub(crate) fn timed_out(&self, now_ms: u64, timeout_ms: u64) -> bool {
        self.active
            .iter()
            .chain(self.queued.iter())
            .any(|request| now_ms.saturating_sub(request.enqueued_at_ms) >= timeout_ms)
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.active.is_none() && self.queued.is_empty()
    }

    pub(crate) fn pending_request_count(&self) -> usize {
        usize::from(self.active.is_some()).saturating_add(self.queued.len())
    }

    pub(crate) const fn stale_completion_count(&self) -> usize {
        self.stale_completion_count
    }

    fn allocate_broker_request_id(&mut self) -> u64 {
        let id = self.next_broker_request_id;
        self.next_broker_request_id = self.next_broker_request_id.wrapping_add(1).max(1);
        id
    }

    fn pump(&mut self) -> RenderWorkerActions {
        if self.lifecycle != RenderWorkerLifecycle::Ready
            || self.active.is_some()
            || self.queued.is_empty()
        {
            return RenderWorkerActions::default();
        }
        let index = self
            .queued
            .iter()
            .position(|request| request.priority == RenderWorkerPriority::Active)
            .unwrap_or(0);
        let request = self
            .queued
            .remove(index)
            .expect("selected render request exists");
        self.active = Some(request.clone());
        RenderWorkerActions {
            dispatch: Some(request),
            ..RenderWorkerActions::default()
        }
    }

    fn drain_unavailable(&mut self) -> RenderWorkerActions {
        let mut failed = self.active.take().into_iter().collect::<Vec<_>>();
        failed.extend(self.queued.drain(..));
        let release_worlds = self.pending_world_releases.iter().copied().collect();
        self.pending_world_releases.clear();
        RenderWorkerActions {
            failed,
            release_worlds,
            ..RenderWorkerActions::default()
        }
    }

    fn take_releasable_worlds(&mut self) -> Vec<u64> {
        let mut releasable = Vec::new();
        for world in self.pending_world_releases.iter().copied() {
            let active = self
                .active
                .as_ref()
                .is_some_and(|request| request.world_instance_id == world);
            let queued = self
                .queued
                .iter()
                .any(|request| request.world_instance_id == world);
            if !active && !queued {
                releasable.push(world);
            }
        }
        for world in &releasable {
            self.pending_world_releases.remove(world);
        }
        releasable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enqueue(
        state: &mut RenderWorkerCoordinatorState,
        client: u32,
        world: u64,
        priority: RenderWorkerPriority,
        time: u64,
    ) -> RenderWorkerRequest {
        state.enqueue(client, world, priority, time).0
    }

    #[test]
    fn ready_worker_prefers_active_over_queued_standby_work() {
        let mut state = RenderWorkerCoordinatorState::new(7);
        let standby = enqueue(&mut state, 1, 2, RenderWorkerPriority::Standby, 0);
        let active = enqueue(&mut state, 1, 1, RenderWorkerPriority::Active, 1);

        let ready = state.mark_ready(7);

        assert_eq!(ready.dispatch, Some(active.clone()));
        let completed = state.complete(7, active.broker_request_id);
        assert!(completed.accepted);
        assert_eq!(completed.actions.dispatch, Some(standby));
    }

    #[test]
    fn stale_generation_and_request_completions_do_not_advance_queue() {
        let mut state = RenderWorkerCoordinatorState::new(3);
        state.mark_ready(3);
        let active = state
            .enqueue(1, 1, RenderWorkerPriority::Active, 0)
            .1
            .dispatch
            .unwrap();
        let queued = enqueue(&mut state, 2, 1, RenderWorkerPriority::Active, 1);

        assert!(state.complete(2, active.broker_request_id).stale);
        assert!(state.complete(3, queued.broker_request_id).stale);
        assert_eq!(state.pending_request_count(), 2);
        assert_eq!(state.stale_completion_count(), 2);
        assert_eq!(
            state.complete(3, active.broker_request_id).actions.dispatch,
            Some(queued)
        );
    }

    #[test]
    fn world_release_waits_for_active_and_queued_requests() {
        let mut state = RenderWorkerCoordinatorState::new(1);
        state.mark_ready(1);
        let first = state
            .enqueue(1, 9, RenderWorkerPriority::Active, 0)
            .1
            .dispatch
            .unwrap();
        let second = enqueue(&mut state, 2, 9, RenderWorkerPriority::Active, 1);

        assert!(state.request_world_release(9).is_empty());
        assert!(
            state
                .complete(1, first.broker_request_id)
                .actions
                .release_worlds
                .is_empty()
        );
        assert_eq!(
            state
                .complete(1, second.broker_request_id)
                .actions
                .release_worlds,
            vec![9]
        );
    }

    #[test]
    fn worker_failure_fails_every_request_and_unblocks_releases() {
        let mut state = RenderWorkerCoordinatorState::new(4);
        state.mark_ready(4);
        let first = state.enqueue(1, 1, RenderWorkerPriority::Active, 0).0;
        let second = state.enqueue(2, 2, RenderWorkerPriority::Standby, 1).0;
        assert!(state.request_world_release(1).is_empty());

        let actions = state.fail(4);

        assert_eq!(actions.failed, vec![first, second]);
        assert_eq!(actions.release_worlds, vec![1]);
        assert!(state.is_idle());
        assert_eq!(state.lifecycle(), RenderWorkerLifecycle::Failed);
    }

    #[test]
    fn queued_priority_tracks_world_role_changes() {
        let mut state = RenderWorkerCoordinatorState::new(1);
        let world_two = enqueue(&mut state, 1, 2, RenderWorkerPriority::Standby, 0);
        let world_one = enqueue(&mut state, 1, 1, RenderWorkerPriority::Active, 1);
        state.set_world_priority(1, RenderWorkerPriority::Standby);
        state.set_world_priority(2, RenderWorkerPriority::Active);

        assert_eq!(
            state
                .mark_ready(1)
                .dispatch
                .map(|request| request.broker_request_id),
            Some(world_two.broker_request_id)
        );
        assert_ne!(world_two.broker_request_id, world_one.broker_request_id);
    }

    #[test]
    fn timeout_covers_active_and_queued_age_without_mutating_state() {
        let mut state = RenderWorkerCoordinatorState::new(1);
        enqueue(&mut state, 1, 1, RenderWorkerPriority::Active, 100);

        assert!(!state.timed_out(20_099, 20_000));
        assert!(state.timed_out(20_100, 20_000));
        assert_eq!(state.lifecycle(), RenderWorkerLifecycle::Starting);
        assert_eq!(state.pending_request_count(), 1);
    }

    #[test]
    fn terminate_is_idempotent_after_draining_work() {
        let mut state = RenderWorkerCoordinatorState::new(1);
        enqueue(&mut state, 1, 1, RenderWorkerPriority::Active, 0);
        assert_eq!(state.generation(), 1);

        assert_eq!(state.terminate().failed.len(), 1);
        assert!(state.terminate().failed.is_empty());
        assert_eq!(state.lifecycle(), RenderWorkerLifecycle::Terminated);
    }

    #[test]
    fn asset_candidate_must_be_ready_before_atomic_activation() {
        let mut state = RenderWorkerAssetSwapState::new(3, 7);
        let candidate = state.begin_candidate(4);

        assert_eq!(candidate.worker_generation, 8);
        assert_eq!(
            state.activate_candidate(4),
            RenderWorkerGenerationActions::default()
        );
        assert!(state.mark_candidate_ready(candidate.worker_generation));
        assert_eq!(state.activate_candidate(4).activated, Some(candidate));
        assert_eq!(state.active(), candidate);
        assert_eq!(state.commit(4).terminate_generation, Some(7));
    }

    #[test]
    fn asset_failure_restores_previous_generation_or_drops_candidate() {
        let mut state = RenderWorkerAssetSwapState::new(3, 7);
        let candidate = state.begin_candidate(4);
        assert_eq!(
            state.rollback(4).terminate_generation,
            Some(candidate.worker_generation)
        );
        assert_eq!(state.active().worker_generation, 7);

        let candidate = state.begin_candidate(5);
        assert!(state.mark_candidate_ready(candidate.worker_generation));
        state.activate_candidate(5);
        let rollback = state.rollback(5);
        assert_eq!(
            rollback.terminate_generation,
            Some(candidate.worker_generation)
        );
        assert_eq!(
            rollback.restored.map(|value| value.worker_generation),
            Some(7)
        );
        assert_eq!(state.active().worker_generation, 7);
    }
}

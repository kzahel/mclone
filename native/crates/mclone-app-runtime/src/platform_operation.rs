//! Target-neutral identity and lifetime tracking for platform operations.
//!
//! Browser adapters may execute an operation through a promise or worker while
//! native adapters may complete it immediately. This ledger keeps the policy
//! contract identical in both cases: request IDs are unique for the host
//! lifetime, epochs reject superseded completions, failures return the state
//! needed by the policy owner to restore a retryable state, and teardown
//! invalidates every outstanding request without retaining platform objects.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

/// Platform-owned execution behind the shared token/epoch policy.
///
/// Native adapters commonly make a completion available during `submit`;
/// browser adapters may make it available after a promise or worker wakes.
pub trait PlatformOperationExecutor<K, T, E>: std::fmt::Debug {
    fn submit(&mut self, operation: PlatformOperation<K>);
    fn try_recv_completion(&mut self) -> Option<PlatformOperationCompletion<T, E>>;
}

/// Cloneable platform-side endpoint for a deferred executor.
///
/// The shared service owns the executor half. A browser adapter retains this
/// handle, takes typed operations when it is ready to start a promise/worker,
/// and later submits typed completions. No `JsValue`, promise, worker, or
/// socket enters the shared ledger.
pub struct DeferredPlatformOperationHandle<K, T, E> {
    submitted: Rc<RefCell<VecDeque<PlatformOperation<K>>>>,
    completions: Rc<RefCell<VecDeque<PlatformOperationCompletion<T, E>>>>,
}

impl<K, T, E> Clone for DeferredPlatformOperationHandle<K, T, E> {
    fn clone(&self) -> Self {
        Self {
            submitted: Rc::clone(&self.submitted),
            completions: Rc::clone(&self.completions),
        }
    }
}

impl<K, T, E> std::fmt::Debug for DeferredPlatformOperationHandle<K, T, E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredPlatformOperationHandle")
            .field("submitted", &self.submitted.borrow().len())
            .field("completions", &self.completions.borrow().len())
            .finish()
    }
}

impl<K, T, E> DeferredPlatformOperationHandle<K, T, E> {
    pub fn take_submitted(&self) -> Option<PlatformOperation<K>> {
        self.submitted.borrow_mut().pop_front()
    }

    pub fn submit_completion(&self, completion: PlatformOperationCompletion<T, E>) {
        self.completions.borrow_mut().push_back(completion);
    }

    pub fn submitted_len(&self) -> usize {
        self.submitted.borrow().len()
    }

    pub fn completion_len(&self) -> usize {
        self.completions.borrow().len()
    }
}

#[derive(Debug)]
pub struct DeferredPlatformOperationExecutor<K, T, E> {
    handle: DeferredPlatformOperationHandle<K, T, E>,
}

impl<K, T, E> PlatformOperationExecutor<K, T, E> for DeferredPlatformOperationExecutor<K, T, E>
where
    K: std::fmt::Debug,
    T: std::fmt::Debug,
    E: std::fmt::Debug,
{
    fn submit(&mut self, operation: PlatformOperation<K>) {
        self.handle.submitted.borrow_mut().push_back(operation);
    }

    fn try_recv_completion(&mut self) -> Option<PlatformOperationCompletion<T, E>> {
        self.handle.completions.borrow_mut().pop_front()
    }
}

pub fn deferred_platform_operation_executor<K, T, E>() -> (
    DeferredPlatformOperationExecutor<K, T, E>,
    DeferredPlatformOperationHandle<K, T, E>,
) {
    let handle = DeferredPlatformOperationHandle {
        submitted: Rc::new(RefCell::new(VecDeque::new())),
        completions: Rc::new(RefCell::new(VecDeque::new())),
    };
    (
        DeferredPlatformOperationExecutor {
            handle: handle.clone(),
        },
        handle,
    )
}

#[derive(Debug)]
pub struct PlatformOperationService<K, R, T, E> {
    ledger: PlatformOperationLedger<K, R>,
    executor: Box<dyn PlatformOperationExecutor<K, T, E>>,
}

impl<K, R, T, E> PlatformOperationService<K, R, T, E> {
    pub fn new(executor: Box<dyn PlatformOperationExecutor<K, T, E>>) -> Self {
        Self {
            ledger: PlatformOperationLedger::new(),
            executor,
        }
    }

    pub fn issue(&mut self, kind: K, failure_restore: R) -> PlatformOperationToken
    where
        K: Clone,
    {
        let operation = self.ledger.issue(kind, failure_restore);
        let token = operation.token;
        self.executor.submit(operation);
        token
    }

    pub fn poll(&mut self) -> Vec<PlatformOperationResolution<K, R, T, E>> {
        let mut resolutions = Vec::new();
        while let Some(completion) = self.executor.try_recv_completion() {
            resolutions.push(self.ledger.complete(completion));
        }
        resolutions
    }

    pub fn pending_len(&self) -> usize {
        self.ledger.pending_len()
    }

    pub fn begin_epoch(&mut self) -> Vec<CancelledPlatformOperation<K, R>> {
        self.ledger.teardown()
    }

    /// Replace the concrete executor at an epoch boundary. Pending neutral
    /// restore state is returned, and late completions retained by the old
    /// adapter cannot enter the new service.
    pub fn replace_executor(
        &mut self,
        executor: Box<dyn PlatformOperationExecutor<K, T, E>>,
    ) -> Vec<CancelledPlatformOperation<K, R>> {
        let cancelled = self.ledger.teardown();
        self.executor = executor;
        cancelled
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlatformOperationEpoch(u64);

impl PlatformOperationEpoch {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlatformOperationRequestId(u64);

impl PlatformOperationRequestId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlatformOperationToken {
    pub epoch: PlatformOperationEpoch,
    pub request_id: PlatformOperationRequestId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformOperation<K> {
    pub token: PlatformOperationToken,
    pub kind: K,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformOperationCompletion<T, E> {
    pub token: PlatformOperationToken,
    pub result: Result<T, E>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelledPlatformOperation<K, R> {
    pub token: PlatformOperationToken,
    pub kind: K,
    pub failure_restore: R,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformOperationResolution<K, R, T, E> {
    Applied {
        token: PlatformOperationToken,
        kind: K,
        value: T,
    },
    Failed {
        token: PlatformOperationToken,
        kind: K,
        error: E,
        failure_restore: R,
    },
    Stale(PlatformOperationCompletion<T, E>),
    Duplicate(PlatformOperationCompletion<T, E>),
    Unknown(PlatformOperationCompletion<T, E>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingPlatformOperation<K, R> {
    kind: K,
    failure_restore: R,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformOperationLedger<K, R> {
    epoch: PlatformOperationEpoch,
    next_request_id: u64,
    pending: HashMap<PlatformOperationToken, PendingPlatformOperation<K, R>>,
    completed: HashSet<PlatformOperationToken>,
}

impl<K, R> Default for PlatformOperationLedger<K, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, R> PlatformOperationLedger<K, R> {
    pub fn new() -> Self {
        Self {
            epoch: PlatformOperationEpoch(1),
            next_request_id: 1,
            pending: HashMap::new(),
            completed: HashSet::new(),
        }
    }

    pub const fn epoch(&self) -> PlatformOperationEpoch {
        self.epoch
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn issue(&mut self, kind: K, failure_restore: R) -> PlatformOperation<K>
    where
        K: Clone,
    {
        let token = PlatformOperationToken {
            epoch: self.epoch,
            request_id: PlatformOperationRequestId(self.next_request_id),
        };
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .expect("platform operation request ID space exhausted");
        let previous = self.pending.insert(
            token,
            PendingPlatformOperation {
                kind: kind.clone(),
                failure_restore,
            },
        );
        debug_assert!(previous.is_none(), "platform operation token reused");
        PlatformOperation { token, kind }
    }

    pub fn complete<T, E>(
        &mut self,
        completion: PlatformOperationCompletion<T, E>,
    ) -> PlatformOperationResolution<K, R, T, E> {
        if completion.token.epoch != self.epoch {
            return PlatformOperationResolution::Stale(completion);
        }

        let Some(pending) = self.pending.remove(&completion.token) else {
            if self.completed.contains(&completion.token) {
                return PlatformOperationResolution::Duplicate(completion);
            }
            return PlatformOperationResolution::Unknown(completion);
        };
        self.completed.insert(completion.token);

        match completion.result {
            Ok(value) => PlatformOperationResolution::Applied {
                token: completion.token,
                kind: pending.kind,
                value,
            },
            Err(error) => PlatformOperationResolution::Failed {
                token: completion.token,
                kind: pending.kind,
                error,
                failure_restore: pending.failure_restore,
            },
        }
    }

    /// Invalidates all outstanding operations and begins a new resource/session
    /// epoch. Concrete promise, worker, socket, or database objects remain in the
    /// platform adapter; the returned values are neutral policy state only.
    pub fn teardown(&mut self) -> Vec<CancelledPlatformOperation<K, R>> {
        let mut cancelled = self
            .pending
            .drain()
            .map(|(token, pending)| CancelledPlatformOperation {
                token,
                kind: pending.kind,
                failure_restore: pending.failure_restore,
            })
            .collect::<Vec<_>>();
        cancelled.sort_by_key(|operation| operation.token.request_id);
        self.completed.clear();
        self.epoch = PlatformOperationEpoch(
            self.epoch
                .0
                .checked_add(1)
                .expect("platform operation epoch space exhausted"),
        );
        cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum TestOperation {
        StartLocal,
        OpenCatalog,
        Reconnect,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RestoreState(&'static str);

    #[derive(Debug)]
    struct ScriptedExecutor {
        deferred: bool,
        submitted: VecDeque<PlatformOperation<TestOperation>>,
        ready: VecDeque<PlatformOperationCompletion<&'static str, &'static str>>,
    }

    impl ScriptedExecutor {
        fn immediate() -> Self {
            Self {
                deferred: false,
                submitted: VecDeque::new(),
                ready: VecDeque::new(),
            }
        }

        fn deferred() -> Self {
            Self {
                deferred: true,
                submitted: VecDeque::new(),
                ready: VecDeque::new(),
            }
        }
    }

    impl PlatformOperationExecutor<TestOperation, &'static str, &'static str> for ScriptedExecutor {
        fn submit(&mut self, operation: PlatformOperation<TestOperation>) {
            if self.deferred {
                self.submitted.push_back(operation);
            } else {
                self.ready.push_back(ok(operation.token, "applied"));
            }
        }

        fn try_recv_completion(
            &mut self,
        ) -> Option<PlatformOperationCompletion<&'static str, &'static str>> {
            if self.ready.is_empty() && self.deferred {
                if let Some(operation) = self.submitted.pop_front() {
                    self.ready.push_back(ok(operation.token, "applied"));
                }
            }
            self.ready.pop_front()
        }
    }

    fn ok(
        token: PlatformOperationToken,
        value: &'static str,
    ) -> PlatformOperationCompletion<&'static str, &'static str> {
        PlatformOperationCompletion {
            token,
            result: Ok(value),
        }
    }

    fn err(
        token: PlatformOperationToken,
        error: &'static str,
    ) -> PlatformOperationCompletion<&'static str, &'static str> {
        PlatformOperationCompletion {
            token,
            result: Err(error),
        }
    }

    #[test]
    fn request_identity_is_unique_across_operation_kinds_and_epochs() {
        let mut ledger = PlatformOperationLedger::new();
        let first = ledger.issue(TestOperation::StartLocal, RestoreState("title"));
        let second = ledger.issue(TestOperation::OpenCatalog, RestoreState("catalog"));

        assert_eq!(first.token.epoch.get(), 1);
        assert_eq!(first.token.request_id.get(), 1);
        assert_eq!(second.token.request_id.get(), 2);
        assert_ne!(first.token, second.token);

        ledger.teardown();
        let third = ledger.issue(TestOperation::Reconnect, RestoreState("disconnected"));
        assert_eq!(third.token.epoch.get(), 2);
        assert_eq!(third.token.request_id.get(), 3);
        assert_ne!(first.token.request_id, third.token.request_id);
    }

    #[test]
    fn completions_are_identity_matched_and_rejections_are_observable() {
        let mut ledger = PlatformOperationLedger::new();
        let first = ledger.issue(TestOperation::StartLocal, RestoreState("title"));
        let second = ledger.issue(TestOperation::OpenCatalog, RestoreState("catalog"));

        assert_eq!(
            ledger.complete(ok(second.token, "worlds")),
            PlatformOperationResolution::Applied {
                token: second.token,
                kind: TestOperation::OpenCatalog,
                value: "worlds",
            }
        );
        assert!(matches!(
            ledger.complete(ok(second.token, "duplicate")),
            PlatformOperationResolution::Duplicate(_)
        ));
        assert!(matches!(
            ledger.complete(ok(
                PlatformOperationToken {
                    epoch: ledger.epoch(),
                    request_id: PlatformOperationRequestId(9_999),
                },
                "unknown",
            )),
            PlatformOperationResolution::Unknown(_)
        ));

        ledger.teardown();
        assert!(matches!(
            ledger.complete(ok(first.token, "late")),
            PlatformOperationResolution::Stale(_)
        ));
    }

    #[test]
    fn failed_completion_returns_exact_retry_state() {
        let mut ledger = PlatformOperationLedger::new();
        let operation = ledger.issue(TestOperation::Reconnect, RestoreState("retryable"));

        assert_eq!(
            ledger.complete(err(operation.token, "socket refused")),
            PlatformOperationResolution::Failed {
                token: operation.token,
                kind: TestOperation::Reconnect,
                error: "socket refused",
                failure_restore: RestoreState("retryable"),
            }
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn teardown_returns_pending_policy_state_and_invalidates_late_results() {
        let mut ledger = PlatformOperationLedger::new();
        let first = ledger.issue(TestOperation::StartLocal, RestoreState("title"));
        let second = ledger.issue(TestOperation::OpenCatalog, RestoreState("catalog"));

        let cancelled = ledger.teardown();
        assert_eq!(
            cancelled,
            vec![
                CancelledPlatformOperation {
                    token: first.token,
                    kind: TestOperation::StartLocal,
                    failure_restore: RestoreState("title"),
                },
                CancelledPlatformOperation {
                    token: second.token,
                    kind: TestOperation::OpenCatalog,
                    failure_restore: RestoreState("catalog"),
                },
            ]
        );
        assert!(ledger.is_empty());
        assert_eq!(ledger.epoch().get(), 2);
        assert!(matches!(
            ledger.complete(err(second.token, "late indexeddb failure")),
            PlatformOperationResolution::Stale(_)
        ));
    }

    #[derive(Debug, Default, Eq, PartialEq)]
    struct ScriptedSessionHostState {
        local_session_active: bool,
        catalog_open: bool,
        reconnect_count: usize,
    }

    fn scripted_policy_trace(deferred: bool) -> ScriptedSessionHostState {
        let executor: Box<dyn PlatformOperationExecutor<_, _, _>> = if deferred {
            Box::new(ScriptedExecutor::deferred())
        } else {
            Box::new(ScriptedExecutor::immediate())
        };
        let mut service = PlatformOperationService::new(executor);
        service.issue(TestOperation::StartLocal, RestoreState("title"));
        service.issue(TestOperation::OpenCatalog, RestoreState("catalog"));
        service.issue(TestOperation::Reconnect, RestoreState("disconnected"));

        let mut state = ScriptedSessionHostState::default();
        for resolution in service.poll() {
            match resolution {
                PlatformOperationResolution::Applied { kind, value, .. } => {
                    assert_eq!(value, "applied");
                    match kind {
                        TestOperation::StartLocal => state.local_session_active = true,
                        TestOperation::OpenCatalog => state.catalog_open = true,
                        TestOperation::Reconnect => state.reconnect_count += 1,
                    }
                }
                resolution => panic!("unexpected resolution: {resolution:?}"),
            }
        }
        state
    }

    #[test]
    fn immediate_and_deferred_executors_have_identical_session_host_policy() {
        assert_eq!(scripted_policy_trace(false), scripted_policy_trace(true));
    }

    #[test]
    fn executor_replacement_cancels_old_epoch_work() {
        let mut service = PlatformOperationService::new(Box::new(ScriptedExecutor::deferred()));
        let token = service.issue(TestOperation::OpenCatalog, RestoreState("catalog"));
        let cancelled = service.replace_executor(Box::new(ScriptedExecutor::immediate()));
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].token, token);
        assert_eq!(service.pending_len(), 0);
    }

    #[test]
    fn service_epoch_change_rejects_old_executor_completion() {
        let mut service = PlatformOperationService::new(Box::new(ScriptedExecutor::deferred()));
        let token = service.issue(TestOperation::OpenCatalog, RestoreState("catalog"));
        let cancelled = service.begin_epoch();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].token, token);
        assert!(
            service
                .poll()
                .into_iter()
                .all(|resolution| matches!(resolution, PlatformOperationResolution::Stale(_)))
        );
    }

    #[test]
    fn deferred_handle_accepts_out_of_order_results_and_rejects_post_teardown_work() {
        let (executor, handle) = deferred_platform_operation_executor();
        let mut service = PlatformOperationService::new(Box::new(executor));
        let first = service.issue(TestOperation::StartLocal, RestoreState("title"));
        let second = service.issue(TestOperation::Reconnect, RestoreState("disconnected"));
        assert_eq!(handle.submitted_len(), 2);
        let submitted_first = handle.take_submitted().unwrap();
        let submitted_second = handle.take_submitted().unwrap();
        assert_eq!(submitted_first.token, first);
        assert_eq!(submitted_second.token, second);

        handle.submit_completion(ok(second, "reconnected"));
        handle.submit_completion(ok(first, "started"));
        let resolutions = service.poll();
        assert!(matches!(
            resolutions[0],
            PlatformOperationResolution::Applied {
                kind: TestOperation::Reconnect,
                value: "reconnected",
                ..
            }
        ));
        assert!(matches!(
            resolutions[1],
            PlatformOperationResolution::Applied {
                kind: TestOperation::StartLocal,
                value: "started",
                ..
            }
        ));

        let late = service.issue(TestOperation::OpenCatalog, RestoreState("catalog"));
        let _ = handle.take_submitted().unwrap();
        assert_eq!(service.begin_epoch().len(), 1);
        handle.submit_completion(err(late, "late"));
        assert!(matches!(
            service.poll().as_slice(),
            [PlatformOperationResolution::Stale(_)]
        ));
    }
}

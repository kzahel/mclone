//! Target-neutral identity and lifetime tracking for platform operations.
//!
//! Browser adapters may execute an operation through a promise or worker while
//! native adapters may complete it immediately. This ledger keeps the policy
//! contract identical in both cases: request IDs are unique for the host
//! lifetime, epochs reject superseded completions, failures return the state
//! needed by the policy owner to restore a retryable state, and teardown
//! invalidates every outstanding request without retaining platform objects.

use std::collections::{HashMap, HashSet};

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

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum TestOperation {
        StartLocal,
        OpenCatalog,
        Reconnect,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RestoreState(&'static str);

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
}

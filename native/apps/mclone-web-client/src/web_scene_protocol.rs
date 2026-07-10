//! Typed browser session lifecycle used by the future scene-host driver.
//!
//! Promise, Worker, WebSocket, and IndexedDB objects remain in the wasm
//! adapter. This module owns only neutral operation identity, supersession,
//! retry state, and late-completion rejection, so it is testable on every host.

use mclone_app_runtime::catalog_executor::{
    DeferredWorldCatalogOperationHandle, WorldCatalogOperation, WorldCatalogOperationService,
};
use mclone_app_runtime::monotonic::{
    MonotonicClockHandle, MonotonicInstant, ProjectedMonotonicClock,
};
use mclone_app_runtime::platform_operation::{
    PlatformOperation, PlatformOperationCompletion, PlatformOperationLedger,
    PlatformOperationResolution, PlatformOperationToken,
};
use mclone_app_runtime::session::ActiveSessionDescriptor;
use mclone_app_runtime::world_catalog::{WorldCatalogError, WorldCatalogResponse};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebSceneSessionOperation {
    StartLocal { seed: i64, world_id: Option<String> },
    ConnectRemote { url: String },
    ReconnectRemote { url: String, attempt: u32 },
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebSceneSessionOperationResult {
    Started(ActiveSessionDescriptor),
    Reconnected,
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WebSceneSessionRestore {
    Idle,
    Active(ActiveSessionDescriptor),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebSceneSessionState {
    Idle,
    Starting {
        token: PlatformOperationToken,
        operation: WebSceneSessionOperation,
    },
    Active {
        descriptor: ActiveSessionDescriptor,
    },
    Reconnecting {
        token: PlatformOperationToken,
        descriptor: ActiveSessionDescriptor,
        attempt: u32,
    },
    ShuttingDown {
        token: PlatformOperationToken,
        descriptor: Option<ActiveSessionDescriptor>,
    },
    Failed {
        operation: WebSceneSessionOperation,
        message: String,
        retryable: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSceneSessionCompletionDisposition {
    Applied,
    Failed,
    Stale,
    Duplicate,
    Unknown,
}

#[derive(Debug)]
pub struct WebSceneSessionLifecycle {
    ledger: PlatformOperationLedger<WebSceneSessionOperation, WebSceneSessionRestore>,
    state: WebSceneSessionState,
}

impl Default for WebSceneSessionLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSceneSessionLifecycle {
    pub fn new() -> Self {
        Self {
            ledger: PlatformOperationLedger::new(),
            state: WebSceneSessionState::Idle,
        }
    }

    pub fn state(&self) -> &WebSceneSessionState {
        &self.state
    }

    /// Start or supersede a connection operation. Supersession always advances
    /// the epoch before issuing the replacement, so a late promise from the old
    /// worker/socket cannot resurrect its session.
    pub fn begin_start(
        &mut self,
        operation: WebSceneSessionOperation,
    ) -> PlatformOperation<WebSceneSessionOperation> {
        assert!(
            matches!(
                operation,
                WebSceneSessionOperation::StartLocal { .. }
                    | WebSceneSessionOperation::ConnectRemote { .. }
            ),
            "begin_start requires a local or remote start operation"
        );
        let _ = self.ledger.teardown();
        let issued = self
            .ledger
            .issue(operation.clone(), WebSceneSessionRestore::Idle);
        self.state = WebSceneSessionState::Starting {
            token: issued.token,
            operation,
        };
        issued
    }

    pub fn begin_reconnect(
        &mut self,
        url: impl Into<String>,
    ) -> Option<PlatformOperation<WebSceneSessionOperation>> {
        let descriptor = match &self.state {
            WebSceneSessionState::Active { descriptor } => descriptor.clone(),
            WebSceneSessionState::Failed {
                operation: WebSceneSessionOperation::ReconnectRemote { url, .. },
                retryable: true,
                ..
            } => ActiveSessionDescriptor::Remote {
                endpoint: mclone_app_runtime::session::RemoteSessionEndpoint::new(url.clone()),
            },
            _ => return None,
        };
        let attempt = match &self.state {
            WebSceneSessionState::Reconnecting { attempt, .. }
            | WebSceneSessionState::Failed {
                operation: WebSceneSessionOperation::ReconnectRemote { attempt, .. },
                ..
            } => attempt.saturating_add(1),
            _ => 1,
        };
        let _ = self.ledger.teardown();
        let operation = WebSceneSessionOperation::ReconnectRemote {
            url: url.into(),
            attempt,
        };
        let issued = self.ledger.issue(
            operation,
            WebSceneSessionRestore::Active(descriptor.clone()),
        );
        self.state = WebSceneSessionState::Reconnecting {
            token: issued.token,
            descriptor,
            attempt,
        };
        Some(issued)
    }

    pub fn begin_shutdown(&mut self) -> PlatformOperation<WebSceneSessionOperation> {
        let descriptor = match &self.state {
            WebSceneSessionState::Active { descriptor }
            | WebSceneSessionState::Reconnecting { descriptor, .. } => Some(descriptor.clone()),
            _ => None,
        };
        let restore = descriptor
            .clone()
            .map_or(WebSceneSessionRestore::Idle, WebSceneSessionRestore::Active);
        let _ = self.ledger.teardown();
        let issued = self
            .ledger
            .issue(WebSceneSessionOperation::Shutdown, restore);
        self.state = WebSceneSessionState::ShuttingDown {
            token: issued.token,
            descriptor,
        };
        issued
    }

    pub fn complete(
        &mut self,
        completion: PlatformOperationCompletion<WebSceneSessionOperationResult, String>,
    ) -> WebSceneSessionCompletionDisposition {
        match self.ledger.complete(completion) {
            PlatformOperationResolution::Applied { kind, value, .. } => {
                match (kind, value) {
                    (
                        WebSceneSessionOperation::StartLocal { .. }
                        | WebSceneSessionOperation::ConnectRemote { .. },
                        WebSceneSessionOperationResult::Started(descriptor),
                    ) => self.state = WebSceneSessionState::Active { descriptor },
                    (
                        operation @ WebSceneSessionOperation::ReconnectRemote { .. },
                        WebSceneSessionOperationResult::Reconnected,
                    ) => {
                        let WebSceneSessionState::Reconnecting { descriptor, .. } = &self.state
                        else {
                            self.state = WebSceneSessionState::Failed {
                                operation,
                                message: "reconnect completion had no active session".to_owned(),
                                retryable: false,
                            };
                            return WebSceneSessionCompletionDisposition::Failed;
                        };
                        self.state = WebSceneSessionState::Active {
                            descriptor: descriptor.clone(),
                        };
                    }
                    (
                        WebSceneSessionOperation::Shutdown,
                        WebSceneSessionOperationResult::Shutdown,
                    ) => self.state = WebSceneSessionState::Idle,
                    (operation, _) => {
                        self.state = WebSceneSessionState::Failed {
                            operation,
                            message: "browser session completion kind mismatch".to_owned(),
                            retryable: false,
                        };
                        return WebSceneSessionCompletionDisposition::Failed;
                    }
                }
                WebSceneSessionCompletionDisposition::Applied
            }
            PlatformOperationResolution::Failed {
                kind,
                error,
                failure_restore,
                ..
            } => {
                let retryable = matches!(
                    kind,
                    WebSceneSessionOperation::StartLocal { .. }
                        | WebSceneSessionOperation::ConnectRemote { .. }
                        | WebSceneSessionOperation::ReconnectRemote { .. }
                );
                if matches!(kind, WebSceneSessionOperation::Shutdown) {
                    self.state = match failure_restore {
                        WebSceneSessionRestore::Idle => WebSceneSessionState::Idle,
                        WebSceneSessionRestore::Active(descriptor) => {
                            WebSceneSessionState::Active { descriptor }
                        }
                    };
                } else {
                    self.state = WebSceneSessionState::Failed {
                        operation: kind,
                        message: error,
                        retryable,
                    };
                }
                WebSceneSessionCompletionDisposition::Failed
            }
            PlatformOperationResolution::Stale(_) => WebSceneSessionCompletionDisposition::Stale,
            PlatformOperationResolution::Duplicate(_) => {
                WebSceneSessionCompletionDisposition::Duplicate
            }
            PlatformOperationResolution::Unknown(_) => {
                WebSceneSessionCompletionDisposition::Unknown
            }
        }
    }

    pub fn teardown(&mut self) {
        let _ = self.ledger.teardown();
        self.state = WebSceneSessionState::Idle;
    }
}

/// Browser-side handles retained outside `McloneSceneHost` while the host owns
/// the corresponding neutral clock and catalog service.
#[derive(Debug)]
pub struct WebScenePlatformServices {
    clock: ProjectedMonotonicClock,
    catalog: DeferredWorldCatalogOperationHandle,
    lifecycle: WebSceneSessionLifecycle,
}

impl WebScenePlatformServices {
    pub fn new() -> (Self, MonotonicClockHandle, WorldCatalogOperationService) {
        let clock = ProjectedMonotonicClock::default();
        let clock_handle = clock.handle();
        let (catalog_operations, catalog) = WorldCatalogOperationService::deferred();
        (
            Self {
                clock,
                catalog,
                lifecycle: WebSceneSessionLifecycle::new(),
            },
            clock_handle,
            catalog_operations,
        )
    }

    pub fn observe_frame_time_millis(&self, millis: f64) -> MonotonicInstant {
        self.clock.observe_millis(millis)
    }

    pub fn lifecycle(&self) -> &WebSceneSessionLifecycle {
        &self.lifecycle
    }

    pub fn lifecycle_mut(&mut self) -> &mut WebSceneSessionLifecycle {
        &mut self.lifecycle
    }

    pub fn take_catalog_operation(&self) -> Option<PlatformOperation<WorldCatalogOperation>> {
        self.catalog.take_submitted()
    }

    pub fn complete_catalog_operation(
        &self,
        completion: PlatformOperationCompletion<WorldCatalogResponse, WorldCatalogError>,
    ) {
        self.catalog.submit_completion(completion);
    }

    pub fn teardown(&mut self) {
        self.lifecycle.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(seed: i64) -> ActiveSessionDescriptor {
        ActiveSessionDescriptor::LocalWorld {
            seed,
            id: None,
            display_name: None,
        }
    }

    #[test]
    fn superseded_out_of_order_start_cannot_resurrect_old_session() {
        let mut lifecycle = WebSceneSessionLifecycle::new();
        let first = lifecycle.begin_start(WebSceneSessionOperation::StartLocal {
            seed: 1,
            world_id: None,
        });
        let second = lifecycle.begin_start(WebSceneSessionOperation::StartLocal {
            seed: 2,
            world_id: None,
        });

        assert_eq!(
            lifecycle.complete(PlatformOperationCompletion {
                token: first.token,
                result: Ok(WebSceneSessionOperationResult::Started(local(1))),
            }),
            WebSceneSessionCompletionDisposition::Stale
        );
        assert_eq!(
            lifecycle.complete(PlatformOperationCompletion {
                token: second.token,
                result: Ok(WebSceneSessionOperationResult::Started(local(2))),
            }),
            WebSceneSessionCompletionDisposition::Applied
        );
        assert_eq!(
            lifecycle.state(),
            &WebSceneSessionState::Active {
                descriptor: local(2)
            }
        );
    }

    #[test]
    fn remote_reconnect_has_explicit_retry_and_terminal_paths() {
        let mut lifecycle = WebSceneSessionLifecycle::new();
        let start = lifecycle.begin_start(WebSceneSessionOperation::ConnectRemote {
            url: "ws://example.invalid".to_owned(),
        });
        let descriptor = ActiveSessionDescriptor::Remote {
            endpoint: mclone_app_runtime::session::RemoteSessionEndpoint::new(
                "ws://example.invalid",
            ),
        };
        assert_eq!(
            lifecycle.complete(PlatformOperationCompletion {
                token: start.token,
                result: Ok(WebSceneSessionOperationResult::Started(descriptor.clone())),
            }),
            WebSceneSessionCompletionDisposition::Applied
        );

        let reconnect = lifecycle
            .begin_reconnect("ws://example.invalid")
            .expect("active remote session can reconnect");
        assert!(matches!(
            lifecycle.state(),
            WebSceneSessionState::Reconnecting { attempt: 1, .. }
        ));
        assert_eq!(
            lifecycle.complete(PlatformOperationCompletion {
                token: reconnect.token,
                result: Err("offline".to_owned()),
            }),
            WebSceneSessionCompletionDisposition::Failed
        );
        assert!(matches!(
            lifecycle.state(),
            WebSceneSessionState::Failed {
                operation: WebSceneSessionOperation::ReconnectRemote { attempt: 1, .. },
                retryable: true,
                ..
            }
        ));
    }

    #[test]
    fn teardown_rejects_late_promise_completion() {
        let mut lifecycle = WebSceneSessionLifecycle::new();
        let start = lifecycle.begin_start(WebSceneSessionOperation::StartLocal {
            seed: 3,
            world_id: None,
        });
        lifecycle.teardown();
        assert_eq!(
            lifecycle.complete(PlatformOperationCompletion {
                token: start.token,
                result: Ok(WebSceneSessionOperationResult::Started(local(3))),
            }),
            WebSceneSessionCompletionDisposition::Stale
        );
        assert_eq!(lifecycle.state(), &WebSceneSessionState::Idle);
    }
}

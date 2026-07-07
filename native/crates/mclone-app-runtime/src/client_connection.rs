use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use anyhow::Result;
use mclone_protocol::{ClientCommand, ServerUpdate};

use crate::host_mode::update_drain_exchange;
use crate::{RuntimeUpdatePumpBudget, RuntimeUpdatePumpReport, SingleViewRuntime, elapsed_ms};

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedServerUpdate {
    update: Option<ServerUpdate>,
    encoded_len: usize,
    queued_age: Duration,
    transport_drained: bool,
    response_sequence: Option<u64>,
    producer_read_ms: f64,
    producer_decode_ms: f64,
}

impl QueuedServerUpdate {
    pub fn single(
        update: ServerUpdate,
        encoded_len: usize,
        queued_age: Duration,
        transport_drained: bool,
    ) -> Self {
        Self {
            update: Some(update),
            encoded_len,
            queued_age,
            transport_drained,
            response_sequence: None,
            producer_read_ms: 0.0,
            producer_decode_ms: 0.0,
        }
    }

    pub fn empty(transport_drained: bool) -> Self {
        Self {
            update: None,
            encoded_len: 0,
            queued_age: Duration::ZERO,
            transport_drained,
            response_sequence: None,
            producer_read_ms: 0.0,
            producer_decode_ms: 0.0,
        }
    }

    pub fn with_remote_metadata(
        mut self,
        response_sequence: Option<u64>,
        producer_read_ms: f64,
        producer_decode_ms: f64,
    ) -> Self {
        self.response_sequence = response_sequence;
        self.producer_read_ms = producer_read_ms;
        self.producer_decode_ms = producer_decode_ms;
        self
    }

    pub const fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    fn into_updates(self) -> Vec<ServerUpdate> {
        self.update.into_iter().collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientConnectionQueueMetrics {
    update_depth: usize,
    update_bytes: usize,
}

impl ClientConnectionQueueMetrics {
    pub const fn new(update_depth: usize, update_bytes: usize) -> Self {
        Self {
            update_depth,
            update_bytes,
        }
    }

    pub const fn update_depth(&self) -> usize {
        self.update_depth
    }

    pub const fn update_bytes(&self) -> usize {
        self.update_bytes
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientConnectionDrainResult {
    update: Option<QueuedServerUpdate>,
    remaining_queue_depth: usize,
    remaining_queue_bytes: usize,
}

impl ClientConnectionDrainResult {
    pub fn with_update(
        update: QueuedServerUpdate,
        remaining_queue_depth: usize,
        remaining_queue_bytes: usize,
    ) -> Self {
        Self {
            update: Some(update),
            remaining_queue_depth,
            remaining_queue_bytes,
        }
    }

    pub const fn pending(remaining_queue_depth: usize, remaining_queue_bytes: usize) -> Self {
        Self {
            update: None,
            remaining_queue_depth,
            remaining_queue_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionUpdateDrainMode {
    Blocking,
    ReadyOnly,
}

pub trait ClientConnection {
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()>;
    fn drain_next_update(
        &mut self,
        mode: ConnectionUpdateDrainMode,
    ) -> Result<ClientConnectionDrainResult>;
    fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics>;
}

pub fn pump_client_connection_updates_report<C>(
    core: &mut SingleViewRuntime,
    connection: &mut C,
    budget: RuntimeUpdatePumpBudget,
    drain_mode: ConnectionUpdateDrainMode,
) -> Result<RuntimeUpdatePumpReport>
where
    C: ClientConnection + ?Sized,
{
    let pump_start = pump_timing_start();
    let mut report = RuntimeUpdatePumpReport::default();
    loop {
        let drain_start = pump_timing_start();
        let drain = connection.drain_next_update(drain_mode)?;
        report.drain_updates_ms += pump_elapsed_ms(drain_start);
        let Some(queued_update) = drain.update else {
            report.remaining_queue_depth = drain.remaining_queue_depth;
            report.remaining_queue_bytes = drain.remaining_queue_bytes;
            if drain.remaining_queue_depth > 0 {
                core.apply_exchange_report(update_drain_exchange(Vec::new(), false));
            }
            return Ok(report);
        };

        let encoded_len = queued_update.encoded_len;
        let queued_age = queued_update.queued_age;
        let transport_drained = queued_update.transport_drained;
        let producer_read_ms = queued_update.producer_read_ms;
        let producer_decode_ms = queued_update.producer_decode_ms;
        let response_sequence = queued_update.response_sequence;
        let exchange_report =
            core.apply_exchange_report(update_drain_exchange(queued_update.into_updates(), true));
        report.apply_report.accumulate(exchange_report.update_apply);
        report.producer_read_ms += producer_read_ms;
        report.producer_decode_ms += producer_decode_ms;
        report.producer_response_sequence =
            report.producer_response_sequence.max(response_sequence);
        report.update_bytes = report.update_bytes.saturating_add(encoded_len);
        report.oldest_applied_update_age_ms = report
            .oldest_applied_update_age_ms
            .max(elapsed_ms(queued_age));

        if budget.exhausted_after_update(pump_elapsed(pump_start), &report.apply_report) {
            let metrics = connection.pending_update_metrics()?;
            report.remaining_queue_depth = metrics.update_depth;
            report.remaining_queue_bytes = metrics.update_bytes;
            if metrics.update_depth > 0 {
                report.stalled = true;
                report.stall_count = 1;
            }
            if metrics.update_depth > 0 || !transport_drained {
                core.apply_exchange_report(update_drain_exchange(Vec::new(), false));
            }
            return Ok(report);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type PumpTimingSample = Instant;

#[cfg(target_arch = "wasm32")]
type PumpTimingSample = f64;

#[cfg(not(target_arch = "wasm32"))]
fn pump_timing_start() -> PumpTimingSample {
    Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn pump_timing_start() -> PumpTimingSample {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn pump_elapsed(start: PumpTimingSample) -> Duration {
    start.elapsed()
}

#[cfg(target_arch = "wasm32")]
fn pump_elapsed(start: PumpTimingSample) -> Duration {
    let elapsed_ms = (js_sys::Date::now() - start).max(0.0);
    if elapsed_ms.is_finite() {
        Duration::from_secs_f64(elapsed_ms / 1000.0)
    } else {
        Duration::ZERO
    }
}

fn pump_elapsed_ms(start: PumpTimingSample) -> f64 {
    elapsed_ms(pump_elapsed(start))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use anyhow::Result;
    use mclone_core::ChunkPos;
    use mclone_protocol::{ClientCommand, ServerUpdate};

    use super::{
        ClientConnection, ClientConnectionDrainResult, ClientConnectionQueueMetrics,
        ConnectionUpdateDrainMode, QueuedServerUpdate, pump_client_connection_updates_report,
    };
    use crate::{
        RuntimeUpdatePumpBudget, SingleViewRuntime, chunk_tracking_radius_for_render_distance,
    };

    #[derive(Debug)]
    struct ScriptedClientConnection {
        sent_commands: Vec<ClientCommand>,
        queued_updates: VecDeque<QueuedServerUpdate>,
        pending_depth: usize,
        pending_bytes: usize,
        drain_modes: Vec<ConnectionUpdateDrainMode>,
    }

    impl ScriptedClientConnection {
        fn new(updates: Vec<QueuedServerUpdate>) -> Self {
            Self {
                sent_commands: Vec::new(),
                queued_updates: updates.into(),
                pending_depth: 0,
                pending_bytes: 0,
                drain_modes: Vec::new(),
            }
        }

        fn with_pending_response(mut self, pending_depth: usize, pending_bytes: usize) -> Self {
            self.pending_depth = pending_depth;
            self.pending_bytes = pending_bytes;
            self
        }
    }

    impl ClientConnection for ScriptedClientConnection {
        fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
            self.sent_commands.push(command);
            Ok(())
        }

        fn drain_next_update(
            &mut self,
            mode: ConnectionUpdateDrainMode,
        ) -> Result<ClientConnectionDrainResult> {
            self.drain_modes.push(mode);
            Ok(match self.queued_updates.pop_front() {
                Some(update) => ClientConnectionDrainResult::with_update(
                    update,
                    self.queued_updates.len(),
                    self.queued_updates
                        .iter()
                        .map(QueuedServerUpdate::encoded_len)
                        .sum(),
                ),
                None if self.pending_depth > 0 => {
                    ClientConnectionDrainResult::pending(self.pending_depth, self.pending_bytes)
                }
                None => ClientConnectionDrainResult::default(),
            })
        }

        fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics> {
            Ok(ClientConnectionQueueMetrics::new(
                self.queued_updates.len(),
                self.queued_updates
                    .iter()
                    .map(QueuedServerUpdate::encoded_len)
                    .sum(),
            ))
        }
    }

    #[test]
    fn shared_connection_pump_preserves_order_across_budget_stall() {
        let center = ChunkPos::new(0, 0);
        let mut core = SingleViewRuntime::remote_dedicated(
            center,
            0,
            chunk_tracking_radius_for_render_distance(0),
        );
        let mut connection = ScriptedClientConnection::new(vec![
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 100 },
                11,
                Duration::from_millis(3),
                false,
            )
            .with_remote_metadata(Some(10), 23.0, 5.0),
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 200 },
                13,
                Duration::from_millis(5),
                false,
            )
            .with_remote_metadata(Some(11), 1.5, 0.25),
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 300 },
                17,
                Duration::from_millis(7),
                true,
            )
            .with_remote_metadata(Some(12), 2.5, 0.75),
        ]);

        let first_report = pump_client_connection_updates_report(
            &mut core,
            &mut connection,
            RuntimeUpdatePumpBudget::MaxElapsed(Duration::ZERO),
            ConnectionUpdateDrainMode::ReadyOnly,
        )
        .unwrap();

        assert_eq!(first_report.apply_report.updates, 1);
        assert_eq!(first_report.update_bytes, 11);
        assert_eq!(first_report.producer_read_ms, 23.0);
        assert_eq!(first_report.producer_decode_ms, 5.0);
        assert_eq!(first_report.producer_response_sequence, Some(10));
        assert_eq!(first_report.remaining_queue_depth, 2);
        assert!(first_report.remaining_queue_bytes > 0);
        assert!(first_report.oldest_applied_update_age_ms >= 3.0);
        assert!(first_report.stalled);
        assert_eq!(core.day_time(), 100);
        assert!(!core.transport_drained());

        let second_report = pump_client_connection_updates_report(
            &mut core,
            &mut connection,
            RuntimeUpdatePumpBudget::unlimited(),
            ConnectionUpdateDrainMode::ReadyOnly,
        )
        .unwrap();

        assert_eq!(second_report.apply_report.updates, 2);
        assert_eq!(second_report.update_bytes, 30);
        assert_eq!(second_report.producer_read_ms, 4.0);
        assert_eq!(second_report.producer_decode_ms, 1.0);
        assert_eq!(second_report.producer_response_sequence, Some(12));
        assert_eq!(second_report.remaining_queue_depth, 0);
        assert_eq!(core.day_time(), 300);
        assert!(!second_report.stalled);
    }

    #[test]
    fn shared_connection_pump_preserves_drained_state_after_complete_response() {
        let center = ChunkPos::new(0, 0);
        let mut core = SingleViewRuntime::remote_dedicated(
            center,
            0,
            chunk_tracking_radius_for_render_distance(0),
        );
        let mut connection = ScriptedClientConnection::new(vec![
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 100 },
                11,
                Duration::from_millis(3),
                false,
            ),
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 200 },
                13,
                Duration::from_millis(5),
                false,
            ),
            QueuedServerUpdate::single(
                ServerUpdate::TimeUpdate { day_time: 300 },
                17,
                Duration::from_millis(7),
                true,
            ),
        ]);

        let report = pump_client_connection_updates_report(
            &mut core,
            &mut connection,
            RuntimeUpdatePumpBudget::unlimited(),
            ConnectionUpdateDrainMode::ReadyOnly,
        )
        .unwrap();

        assert_eq!(report.apply_report.updates, 3);
        assert_eq!(report.remaining_queue_depth, 0);
        assert_eq!(core.day_time(), 300);
        assert!(core.transport_drained());
    }

    #[test]
    fn shared_connection_pump_reports_pending_response_without_blocking_ready_only_drain() {
        let center = ChunkPos::new(0, 0);
        let mut core = SingleViewRuntime::remote_dedicated(
            center,
            0,
            chunk_tracking_radius_for_render_distance(0),
        );
        let mut connection = ScriptedClientConnection::new(Vec::new()).with_pending_response(1, 99);

        let report = pump_client_connection_updates_report(
            &mut core,
            &mut connection,
            RuntimeUpdatePumpBudget::unlimited(),
            ConnectionUpdateDrainMode::ReadyOnly,
        )
        .unwrap();

        assert_eq!(
            connection.drain_modes,
            vec![ConnectionUpdateDrainMode::ReadyOnly]
        );
        assert_eq!(report.apply_report.updates, 0);
        assert_eq!(report.remaining_queue_depth, 1);
        assert_eq!(report.remaining_queue_bytes, 99);
        assert!(!report.stalled);
        assert!(!core.transport_drained());
    }
}

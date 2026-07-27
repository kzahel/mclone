use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::{
    TerrainVegetationExecutor, TerrainVegetationExecutorActor,
    TerrainVegetationExecutorDiagnostics, TerrainVegetationExecutorEvent,
    TerrainVegetationExecutorJob, TerrainVegetationExecutorKind, TerrainVegetationSubmitError,
};
use mclone_worldgen::terrain_vegetation::{
    TerrainVegetationCompilerSession, TerrainVegetationSourceIdentity,
};

struct NativeWorker {
    actor: TerrainVegetationExecutorActor,
    requests: Option<SyncSender<TerrainVegetationExecutorJob>>,
    completions: Receiver<TerrainVegetationExecutorEvent>,
    handle: Option<JoinHandle<()>>,
    retired: bool,
}

impl NativeWorker {
    fn spawn(
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    ) -> Result<Self, String> {
        let (request_sender, request_receiver) =
            mpsc::sync_channel::<TerrainVegetationExecutorJob>(1);
        let (completion_sender, completion_receiver) =
            mpsc::sync_channel::<TerrainVegetationExecutorEvent>(1);
        let handle = thread::Builder::new()
            .name("mclone-terrain-vegetation".to_owned())
            .spawn(move || {
                let mut compiler = TerrainVegetationCompilerSession::new(source);
                while let Ok(job) = request_receiver.recv() {
                    let started = Instant::now();
                    let event = match compiler.compile(job.source, job.request) {
                        Ok(product) => TerrainVegetationExecutorEvent::Completed {
                            identity: job.identity,
                            source: job.source,
                            product,
                            compile_micros: u64::try_from(started.elapsed().as_micros())
                                .unwrap_or(u64::MAX),
                        },
                        Err(error) => TerrainVegetationExecutorEvent::JobFailed {
                            identity: job.identity,
                            error: error.to_string(),
                        },
                    };
                    if completion_sender.send(event).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("failed to spawn terrain vegetation worker: {error}"))?;
        Ok(Self {
            actor,
            requests: Some(request_sender),
            completions: completion_receiver,
            handle: Some(handle),
            retired: false,
        })
    }

    fn disconnect(&mut self, retired: bool) {
        self.requests = None;
        self.retired = retired;
    }

    fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }

    fn join(&mut self) -> thread::Result<()> {
        self.handle.take().map_or(Ok(()), JoinHandle::join)
    }
}

pub struct NativeTerrainVegetationExecutor {
    active: Option<NativeWorker>,
    retired: Vec<NativeWorker>,
    queued_events: VecDeque<TerrainVegetationExecutorEvent>,
    diagnostics: TerrainVegetationExecutorDiagnostics,
    shutdown_actor: Option<TerrainVegetationExecutorActor>,
    shutdown_reported: bool,
}

impl NativeTerrainVegetationExecutor {
    pub fn new() -> Self {
        Self {
            active: None,
            retired: Vec::new(),
            queued_events: VecDeque::new(),
            diagnostics: TerrainVegetationExecutorDiagnostics::default(),
            shutdown_actor: None,
            shutdown_reported: false,
        }
    }

    fn poll_workers(&mut self) {
        if let Some(active) = self.active.as_mut() {
            drain_worker_events(active, &mut self.queued_events, &mut self.diagnostics);
        }
        for worker in &mut self.retired {
            drain_worker_events(worker, &mut self.queued_events, &mut self.diagnostics);
        }

        let active_finished = self.active.as_ref().is_some_and(NativeWorker::is_finished);
        if active_finished {
            let mut worker = self.active.take().expect("checked active worker");
            let expected_exit = worker.requests.is_none();
            if worker.join().is_err() || !expected_exit {
                self.diagnostics.transport_failures =
                    self.diagnostics.transport_failures.saturating_add(1);
                self.queued_events
                    .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                        actor: worker.actor,
                        error: "native terrain vegetation worker exited unexpectedly".to_owned(),
                    });
            }
        }

        let mut index = 0;
        while index < self.retired.len() {
            if !self.retired[index].is_finished() {
                index += 1;
                continue;
            }
            let mut worker = self.retired.remove(index);
            if worker.join().is_err() && !worker.retired {
                self.diagnostics.transport_failures =
                    self.diagnostics.transport_failures.saturating_add(1);
                self.queued_events
                    .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                        actor: worker.actor,
                        error: "retired terrain vegetation worker panicked".to_owned(),
                    });
            }
        }

        if let Some(actor) = self.shutdown_actor
            && !self.shutdown_reported
            && self.active.is_none()
            && self.retired.is_empty()
        {
            self.shutdown_reported = true;
            self.queued_events
                .push_back(TerrainVegetationExecutorEvent::ShutdownComplete { actor });
        }
    }
}

impl Default for NativeTerrainVegetationExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl TerrainVegetationExecutor for NativeTerrainVegetationExecutor {
    fn kind(&self) -> TerrainVegetationExecutorKind {
        TerrainVegetationExecutorKind::NativeThread
    }

    fn try_submit(
        &mut self,
        job: &TerrainVegetationExecutorJob,
    ) -> Result<(), TerrainVegetationSubmitError> {
        let worker = self.active.as_mut().ok_or_else(|| {
            TerrainVegetationSubmitError::Failed(
                "native terrain vegetation worker is not active".to_owned(),
            )
        })?;
        if worker.actor != job.identity.actor {
            return Err(TerrainVegetationSubmitError::Failed(
                "native terrain vegetation job targets a retired actor".to_owned(),
            ));
        }
        let sender = worker.requests.as_ref().ok_or_else(|| {
            TerrainVegetationSubmitError::Failed(
                "native terrain vegetation worker ingress is closed".to_owned(),
            )
        })?;
        match sender.try_send(*job) {
            Ok(()) => {
                self.diagnostics.submitted_jobs = self.diagnostics.submitted_jobs.saturating_add(1);
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(TerrainVegetationSubmitError::Full),
            Err(TrySendError::Disconnected(_)) => Err(TerrainVegetationSubmitError::Failed(
                "native terrain vegetation request channel disconnected".to_owned(),
            )),
        }
    }

    fn drain_events(&mut self) -> Vec<TerrainVegetationExecutorEvent> {
        self.poll_workers();
        self.queued_events.drain(..).collect()
    }

    fn restart(
        &mut self,
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    ) -> Result<(), String> {
        source.validate()?;
        if let Some(mut active) = self.active.take() {
            active.disconnect(true);
            self.retired.push(active);
        }
        self.shutdown_actor = None;
        self.shutdown_reported = false;
        self.active = Some(NativeWorker::spawn(actor, source)?);
        self.diagnostics.restarts = self.diagnostics.restarts.saturating_add(1);
        self.queued_events
            .push_back(TerrainVegetationExecutorEvent::Ready { actor, source });
        Ok(())
    }

    fn request_shutdown(&mut self, actor: TerrainVegetationExecutorActor) {
        if self.shutdown_actor.is_some() {
            return;
        }
        self.shutdown_actor = Some(actor);
        if let Some(active) = self.active.as_mut() {
            active.disconnect(false);
        }
        for worker in &mut self.retired {
            worker.disconnect(true);
        }
        self.poll_workers();
    }

    fn is_terminated(&self) -> bool {
        self.shutdown_actor.is_some()
            && self.active.as_ref().is_none_or(NativeWorker::is_finished)
            && self.retired.iter().all(NativeWorker::is_finished)
    }

    fn diagnostics(&self) -> TerrainVegetationExecutorDiagnostics {
        self.diagnostics
    }
}

impl Drop for NativeTerrainVegetationExecutor {
    fn drop(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.disconnect(false);
        }
        for worker in &mut self.retired {
            worker.disconnect(true);
        }
        if let Some(mut active) = self.active.take() {
            let _ = active.join();
        }
        for worker in &mut self.retired {
            let _ = worker.join();
        }
    }
}

fn drain_worker_events(
    worker: &mut NativeWorker,
    queued: &mut VecDeque<TerrainVegetationExecutorEvent>,
    diagnostics: &mut TerrainVegetationExecutorDiagnostics,
) {
    loop {
        match worker.completions.try_recv() {
            Ok(event) => {
                if matches!(event, TerrainVegetationExecutorEvent::Completed { .. }) {
                    diagnostics.completed_jobs = diagnostics.completed_jobs.saturating_add(1);
                }
                queued.push_back(event);
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use crate::{
        TerrainVegetationExecutor, TerrainVegetationExecutorActor, TerrainVegetationExecutorEvent,
        TerrainVegetationExecutorJob, TerrainVegetationJobIdentity, TerrainVegetationSlotToken,
        TerrainViewportTileId,
    };
    use mclone_worldgen::terrain_preview::{
        TerrainPreviewContentStage, TerrainPreviewProfile, TerrainPreviewRequest,
        TerrainPreviewSurfaceQuality,
    };

    use super::*;

    fn request() -> TerrainPreviewRequest {
        TerrainPreviewRequest {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: 12_345,
            center_x: 0,
            center_z: 0,
            sample_spacing: 4,
            cells_per_axis: 64,
            topology: Default::default(),
            content_stage: TerrainPreviewContentStage::Cover,
            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
        }
    }

    #[test]
    fn named_thread_moves_typed_products_and_shuts_down() {
        let request = request();
        let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
        let actor = TerrainVegetationExecutorActor {
            executor_generation: 1,
            source_epoch: 1,
        };
        let identity = TerrainVegetationJobIdentity {
            actor,
            request_id: 1,
            tile: TerrainViewportTileId {
                profile: request.profile,
                seed: request.seed,
                tile_x: 0,
                tile_z: 0,
                sample_spacing: request.sample_spacing,
                content_stage: request.content_stage,
                surface_quality: request.surface_quality,
            },
            slot: TerrainVegetationSlotToken {
                physical_slot: 0,
                slot_generation: 1,
            },
        };
        let mut executor = NativeTerrainVegetationExecutor::new();
        executor.restart(actor, source).unwrap();
        assert!(matches!(
            executor.drain_events().as_slice(),
            [TerrainVegetationExecutorEvent::Ready { .. }]
        ));
        executor
            .try_submit(&TerrainVegetationExecutorJob {
                identity,
                source,
                request,
            })
            .unwrap();

        let completed = loop {
            if let Some(event) = executor.drain_events().into_iter().next() {
                break event;
            }
            thread::yield_now();
        };
        assert!(matches!(
            completed,
            TerrainVegetationExecutorEvent::Completed {
                identity: completed_identity,
                ..
            } if completed_identity == identity
        ));
        executor.request_shutdown(actor);
        loop {
            if executor.drain_events().into_iter().any(|event| {
                matches!(
                    event,
                    TerrainVegetationExecutorEvent::ShutdownComplete { .. }
                )
            }) {
                break;
            }
            thread::yield_now();
        }
        assert!(executor.is_terminated());
        assert_eq!(executor.diagnostics().submitted_jobs, 1);
        assert_eq!(executor.diagnostics().completed_jobs, 1);
    }
}

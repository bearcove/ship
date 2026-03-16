use std::sync::Arc;

use ship_frontend_service::*;
use ship_policy::ParticipantName;
use ship_runtime::Runtime;
use tokio::sync::Mutex;

/// Implementation of the Frontend roam service, backed by ship-runtime.
#[derive(Clone)]
pub struct FrontendImpl {
    runtime: Arc<Mutex<Runtime>>,
}

impl FrontendImpl {
    pub fn new(runtime: Arc<Mutex<Runtime>>) -> Self {
        Self { runtime }
    }
}

impl Frontend for FrontendImpl {
    async fn connect(&self) -> ConnectSnapshot {
        let rt = self.runtime.lock().await;
        let topology = rt.topology().clone();

        // Build room snapshots for each lane.
        let mut rooms = Vec::new();
        for lane in &topology.lanes {
            let task = rt.current_task(&lane.id).ok().flatten();
            // Blocks require &mut self (ensure_warm), so we skip for now
            // and return empty blocks. The frontend will get them via events.
            rooms.push(RoomSnapshot {
                room_id: lane.id.clone(),
                current_task: task,
                recent_blocks: Vec::new(),
            });
        }

        ConnectSnapshot { topology, rooms }
    }

    async fn subscribe(&self, events: roam::Tx<FrontendEvent>) {
        let rt = self.runtime.lock().await;
        let mut rx = rt.subscribe();
        drop(rt);

        // Forward runtime events to the roam channel until it closes.
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let fe = match event {
                        ship_runtime::FrontendEvent::BlockChanged { room_id, block } => {
                            FrontendEvent::BlockChanged { room_id, block }
                        }
                        ship_runtime::FrontendEvent::TaskChanged { room_id, task } => {
                            FrontendEvent::TaskChanged { room_id, task }
                        }
                        ship_runtime::FrontendEvent::TaskCleared { room_id } => {
                            FrontendEvent::TaskCleared { room_id }
                        }
                        ship_runtime::FrontendEvent::RoomChanged { .. } => {
                            continue;
                        }
                        ship_runtime::FrontendEvent::RoomRemoved { .. } => {
                            continue;
                        }
                        ship_runtime::FrontendEvent::BlockRemoved { .. } => {
                            continue;
                        }
                    };
                    if events.send(fe).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(n, "frontend subscriber lagged, dropped events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    }

    async fn send_message(&self, input: HumanInput) {
        let mut rt = self.runtime.lock().await;
        let topology = rt.topology().clone();
        let human_name = topology.human.name.clone();

        // Open and seal a block from the human.
        let block_id = match rt.open_block(
            &input.room_id,
            Some(human_name),
            None,
            ship_policy::BlockContent::Text { text: input.text },
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(error = %e, "failed to open block for human input");
                return;
            }
        };

        match rt.seal_block(&input.room_id, &block_id) {
            Ok(deliveries) => {
                if let Err(e) = rt.process_deliveries(deliveries) {
                    tracing::warn!(error = %e, "failed to process deliveries from human input");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to seal human input block");
            }
        }
    }

    async fn set_agent_model(&self, participant: ParticipantName, model_spec: String) {
        let rt = self.runtime.lock().await;
        if let Some(channels) = rt.agents().get(&participant) {
            let spec = match ship_agent::ModelSpec::parse(&model_spec) {
                Some(s) => s,
                None => {
                    tracing::warn!(%model_spec, "invalid model spec from frontend");
                    return;
                }
            };
            let _ = channels
                .tx
                .send(ship_agent::AgentInput::SetModel(spec))
                .await;
        }
    }
}

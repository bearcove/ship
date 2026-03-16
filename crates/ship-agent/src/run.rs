use std::sync::Arc;

use ship_acp::{AgentDriver, AgentSessionConfig, StopReason};
use ship_policy::{BlockContent, BlockId, DeliveryContent};
use ship_types::{AgentKind, Role, SessionEvent};
use tokio::sync::mpsc;
use ulid::Ulid;

use crate::{
    AgentChannels, AgentConfig, AgentInput, AgentOutput, AgentStatus, ModelSpec, RoomReader,
};

/// Spawn an agent and return channel endpoints.
///
/// This starts a background tokio task that:
/// 1. Spawns an ACP agent process via the driver
/// 2. Loops: receives deliveries, prompts the agent, streams output back
/// 3. Handles model changes (restarting if the agent kind changes)
/// 4. Shuts down cleanly on Shutdown input or channel close
pub fn spawn_agent<R: RoomReader + 'static>(
    config: AgentConfig,
    driver: Arc<dyn AgentDriver>,
    room_reader: Arc<R>,
) -> AgentChannels {
    let (input_tx, input_rx) = mpsc::channel(32);
    let (output_tx, output_rx) = mpsc::channel(256);

    tokio::spawn(agent_loop(config, driver, room_reader, input_rx, output_tx));

    AgentChannels {
        tx: input_tx,
        rx: output_rx,
    }
}

fn role_for(role: ship_policy::AgentRole) -> Role {
    match role {
        ship_policy::AgentRole::Admiral => Role::Captain,
        ship_policy::AgentRole::Captain => Role::Captain,
        ship_policy::AgentRole::Mate => Role::Mate,
    }
}

struct LiveAgent {
    handle: ship_acp::AgentHandle,
    kind: AgentKind,
}

async fn agent_loop<R: RoomReader + 'static>(
    config: AgentConfig,
    driver: Arc<dyn AgentDriver>,
    room_reader: Arc<R>,
    mut input_rx: mpsc::Receiver<AgentInput>,
    output_tx: mpsc::Sender<AgentOutput>,
) {
    let Some(spec) = ModelSpec::parse(&config.model_spec) else {
        let _ = output_tx
            .send(AgentOutput::StatusChanged(AgentStatus::Dead {
                error: format!("invalid model spec: {}", config.model_spec),
            }))
            .await;
        return;
    };

    let mut live = match spawn_acp(&driver, &config, spec.kind).await {
        Ok(live) => live,
        Err(e) => {
            let _ = output_tx
                .send(AgentOutput::StatusChanged(AgentStatus::Dead {
                    error: e.to_string(),
                }))
                .await;
            return;
        }
    };

    // Set the initial model.
    if let Err(e) = driver.set_model(&live.handle, &spec.model).await {
        tracing::warn!(error = %e, "failed to set initial model");
    }

    let _ = output_tx
        .send(AgentOutput::StatusChanged(AgentStatus::Idle))
        .await;

    loop {
        let Some(input) = input_rx.recv().await else {
            // Channel closed — runtime dropped us.
            let _ = driver.kill(&live.handle).await;
            break;
        };

        match input {
            AgentInput::Delivery(delivery) => {
                let _ = output_tx
                    .send(AgentOutput::StatusChanged(AgentStatus::Prompting))
                    .await;

                let block_id = BlockId::new(Ulid::new().to_string());
                let prompt_text = delivery_to_prompt_text(&delivery);

                let parts = vec![ship_types::PromptContentPart::Text { text: prompt_text }];

                match driver.prompt(&live.handle, &parts).await {
                    Ok(response) => {
                        // Drain notifications into block updates.
                        drain_notifications(
                            &driver,
                            &live.handle,
                            &block_id,
                            &output_tx,
                        )
                        .await;

                        if matches!(response.stop_reason, StopReason::ContextExhausted) {
                            let _ = output_tx
                                .send(AgentOutput::StatusChanged(AgentStatus::ContextUsage {
                                    used_pct: 100,
                                }))
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = output_tx
                            .send(AgentOutput::UpdateBlock {
                                block_id: block_id.clone(),
                                content: BlockContent::Error {
                                    message: e.to_string(),
                                },
                            })
                            .await;
                    }
                }

                let _ = output_tx
                    .send(AgentOutput::StatusChanged(AgentStatus::Idle))
                    .await;
            }

            AgentInput::SetModel(new_spec_str) => {
                let Some(new_spec) = ModelSpec::parse(&new_spec_str) else {
                    tracing::warn!(spec = %new_spec_str, "ignoring invalid model spec");
                    continue;
                };

                if new_spec.kind != live.kind {
                    // Agent kind changed — need to kill and re-spawn.
                    let _ = driver.kill(&live.handle).await;

                    match spawn_acp(&driver, &config, new_spec.kind).await {
                        Ok(new_live) => {
                            live = new_live;
                        }
                        Err(e) => {
                            let _ = output_tx
                                .send(AgentOutput::StatusChanged(AgentStatus::Dead {
                                    error: e.to_string(),
                                }))
                                .await;
                            return;
                        }
                    }
                }

                if let Err(e) = driver.set_model(&live.handle, &new_spec.model).await {
                    tracing::warn!(error = %e, "failed to set model");
                }
            }

            AgentInput::Shutdown => {
                let _ = driver.kill(&live.handle).await;
                break;
            }
        }
    }
}

async fn spawn_acp(
    driver: &Arc<dyn AgentDriver>,
    config: &AgentConfig,
    kind: AgentKind,
) -> Result<LiveAgent, ship_acp::AgentError> {
    let acp_config = AgentSessionConfig {
        worktree_path: config.worktree_path.clone(),
        mcp_servers: config.mcp_servers.clone(),
        resume_session_id: None,
    };

    let info = driver
        .spawn(kind, role_for(config.role), &acp_config)
        .await?;

    Ok(LiveAgent {
        handle: info.handle,
        kind,
    })
}

fn delivery_to_prompt_text(delivery: &ship_policy::Delivery) -> String {
    match &delivery.content {
        DeliveryContent::Message { text } => {
            format!("[from @{}] {text}", delivery.from)
        }
        DeliveryContent::Question { text } => {
            format!("[question from @{}] {text}", delivery.from)
        }
        DeliveryContent::Bounce { reason, allowed } => {
            let names = allowed
                .iter()
                .map(|n| format!("@{n}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{reason} You can mention: {names}")
        }
        DeliveryContent::Denied {
            attempted_target,
            reason,
        } => {
            format!("Cannot message @{attempted_target}: {reason}")
        }
        DeliveryContent::Guidance { text } => text.clone(),
        DeliveryContent::Submitted { summary } => {
            format!("[submitted by @{}] {summary}", delivery.from)
        }
        DeliveryContent::Committed {
            step,
            commit_summary,
            diff_section,
        } => {
            let step_str = step.as_deref().unwrap_or("commit");
            format!(
                "[committed by @{}: {step_str}] {commit_summary}\n{diff_section}",
                delivery.from
            )
        }
        DeliveryContent::PlanSet { plan_status } => {
            format!("[plan update from @{}] {plan_status}", delivery.from)
        }
        DeliveryContent::ActivitySummary { summary } => {
            format!("[activity] {summary}")
        }
        DeliveryContent::TaskAssigned { title, description } => {
            format!("[task assigned: {title}] {description}")
        }
        DeliveryContent::ChecksStarted { context } => {
            format!("[checks started: {context}]")
        }
        DeliveryContent::ChecksFinished {
            context,
            all_passed,
            summary,
        } => {
            let status = if *all_passed { "passed" } else { "FAILED" };
            format!("[checks {status}: {context}] {summary}")
        }
    }
}

async fn drain_notifications(
    driver: &Arc<dyn AgentDriver>,
    handle: &ship_acp::AgentHandle,
    block_id: &BlockId,
    output_tx: &mpsc::Sender<AgentOutput>,
) {
    use futures_util::StreamExt;

    let mut stream = driver.notifications(handle);
    while let Some(event) = StreamExt::next(&mut stream).await {
        match event {
            SessionEvent::BlockFinalized { text, .. } => {
                let _ = output_tx
                    .send(AgentOutput::UpdateBlock {
                        block_id: block_id.clone(),
                        content: BlockContent::Text { text },
                    })
                    .await;
            }
            SessionEvent::ContextUpdated {
                remaining_percent, ..
            } => {
                let _ = output_tx
                    .send(AgentOutput::StatusChanged(AgentStatus::ContextUsage {
                        used_pct: 100 - remaining_percent,
                    }))
                    .await;
            }
            _ => {
                // TODO: handle BlockAppend (streaming), ToolCall blocks,
                // PlanUpdate, Permission requests, etc.
            }
        }
    }
}

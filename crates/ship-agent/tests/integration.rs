use std::sync::Arc;

use camino::Utf8PathBuf;
use ship_acp::{FakeAgentDriver, StopReason};
use ship_agent::{AgentConfig, AgentInput, AgentOutput, AgentStatus, ModelSpec, RoomReader, spawn_agent};
use ship_policy::{AgentRole, Block, Delivery, DeliveryContent, ParticipantName, RoomId};

/// A fake room reader that returns a configurable list of blocks.
struct FakeRoomReader {
    blocks: Vec<Block>,
}

impl FakeRoomReader {
    fn empty() -> Self {
        Self { blocks: vec![] }
    }

    #[allow(dead_code)]
    fn with_blocks(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }
}

impl RoomReader for FakeRoomReader {
    async fn recent_blocks(&self, _room_id: &RoomId, _limit: usize) -> Vec<Block> {
        self.blocks.clone()
    }
}

fn test_config() -> AgentConfig {
    AgentConfig {
        room_id: RoomId::from_static("lane-1"),
        participant: ParticipantName::from_static("Cedar"),
        role: AgentRole::Captain,
        model_spec: ModelSpec::parse("claude::opus").unwrap(),
        system_prompt: "You are Cedar, a captain.".into(),
        mcp_servers: vec![],
        worktree_path: Utf8PathBuf::from("/tmp/test"),
    }
}

fn test_delivery() -> Delivery {
    Delivery {
        to: ParticipantName::from_static("Cedar"),
        from: ParticipantName::from_static("Jordan"),
        content: DeliveryContent::Message {
            text: "hello".into(),
        },
        urgent: false,
    }
}

/// Receive the next AgentOutput, panicking with a message on timeout.
async fn recv_output(rx: &mut tokio::sync::mpsc::Receiver<AgentOutput>) -> AgentOutput {
    tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for agent output")
        .expect("agent output channel closed unexpectedly")
}

/// Receive the next status change, ignoring non-status outputs.
async fn recv_status(rx: &mut tokio::sync::mpsc::Receiver<AgentOutput>) -> AgentStatus {
    loop {
        match recv_output(rx).await {
            AgentOutput::StatusChanged(s) => return s,
            _ => continue,
        }
    }
}

#[tokio::test]
async fn spawn_and_shutdown() {
    let driver = FakeAgentDriver::default();
    let reader = Arc::new(FakeRoomReader::empty());

    let mut ch = spawn_agent(test_config(), Arc::new(driver.clone()), reader);

    // First output should be Idle (after successful spawn).
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Send shutdown.
    ch.tx.send(AgentInput::Shutdown).await.unwrap();

    // Channel should close (recv returns None).
    let closed = tokio::time::timeout(std::time::Duration::from_secs(5), ch.rx.recv())
        .await
        .expect("timed out waiting for channel close");
    assert!(closed.is_none(), "expected channel to close after shutdown");
}

#[tokio::test]
async fn delivery_prompts_agent() {
    let driver = FakeAgentDriver::default();
    let reader = Arc::new(FakeRoomReader::empty());

    driver.push_response(StopReason::EndTurn);

    let mut ch = spawn_agent(test_config(), Arc::new(driver.clone()), reader);

    // Wait for initial Idle.
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Send a delivery.
    ch.tx
        .send(AgentInput::Delivery(test_delivery()))
        .await
        .unwrap();

    // Should see Prompting then Idle.
    assert!(matches!(
        recv_status(&mut ch.rx).await,
        AgentStatus::Prompting
    ));
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Verify the prompt was actually called.
    let log = driver.prompt_log();
    assert_eq!(log.len(), 1, "expected exactly one prompt call");

    // Clean shutdown.
    ch.tx.send(AgentInput::Shutdown).await.unwrap();
}

#[tokio::test]
async fn set_model_same_kind() {
    let driver = FakeAgentDriver::default();
    let reader = Arc::new(FakeRoomReader::empty());

    let mut ch = spawn_agent(test_config(), Arc::new(driver.clone()), reader);
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Change model within same kind (claude::opus -> claude::sonnet).
    let new_spec = ModelSpec::parse("claude::sonnet").unwrap();
    ch.tx.send(AgentInput::SetModel(new_spec)).await.unwrap();

    // Give the agent loop a moment to process.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // set_model should have been called (initial + the new one).
    let model_log = driver.model_set_log();
    assert!(
        model_log.iter().any(|(_, m)| m == "sonnet"),
        "expected set_model to be called with 'sonnet', got: {model_log:?}"
    );

    // No respawn: only the initial spawn should exist.
    assert_eq!(
        driver.spawn_records().len(),
        1,
        "expected no respawn for same-kind model change"
    );

    // No kill should have happened.
    assert!(
        driver.killed_handles().is_empty(),
        "expected no kill for same-kind model change"
    );

    ch.tx.send(AgentInput::Shutdown).await.unwrap();
}

#[tokio::test]
async fn set_model_different_kind_respawns() {
    let driver = FakeAgentDriver::default();
    let reader = Arc::new(FakeRoomReader::empty());

    let mut ch = spawn_agent(test_config(), Arc::new(driver.clone()), reader);
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Capture the initial handle.
    let initial_handle = driver.spawn_records()[0].handle.clone();

    // Change to a different kind (claude -> codex).
    let new_spec = ModelSpec::parse("codex::gpt-5.4-high").unwrap();
    ch.tx.send(AgentInput::SetModel(new_spec)).await.unwrap();

    // Give the agent loop time to process the respawn.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Old handle should have been killed.
    let killed = driver.killed_handles();
    assert!(
        killed.contains(&initial_handle),
        "expected old handle to be killed on kind change"
    );

    // A second spawn should have happened.
    let spawns = driver.spawn_records();
    assert_eq!(spawns.len(), 2, "expected respawn for different-kind model change");

    // The second spawn should be Codex.
    assert_eq!(
        spawns[1].kind,
        ship_types::AgentKind::Codex,
        "expected second spawn to be Codex"
    );

    ch.tx.send(AgentInput::Shutdown).await.unwrap();
}

#[tokio::test]
async fn first_prompt_includes_system_prompt() {
    let driver = FakeAgentDriver::default();
    let reader = Arc::new(FakeRoomReader::empty());

    driver.push_response(StopReason::EndTurn);

    let config = test_config();
    let system_prompt = config.system_prompt.clone();

    let mut ch = spawn_agent(config, Arc::new(driver.clone()), reader);
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Send a delivery.
    ch.tx
        .send(AgentInput::Delivery(test_delivery()))
        .await
        .unwrap();

    // Wait for prompt cycle to finish.
    assert!(matches!(
        recv_status(&mut ch.rx).await,
        AgentStatus::Prompting
    ));
    assert!(matches!(recv_status(&mut ch.rx).await, AgentStatus::Idle));

    // Check that the prompt parts include the system prompt.
    let log = driver.prompt_log();
    assert_eq!(log.len(), 1);
    let parts = &log[0].1;

    let has_system_prompt = parts.iter().any(|p| match p {
        ship_types::PromptContentPart::Text { text } => text == &system_prompt,
        _ => false,
    });
    assert!(
        has_system_prompt,
        "expected system prompt in first prompt parts, got: {parts:?}"
    );

    ch.tx.send(AgentInput::Shutdown).await.unwrap();
}

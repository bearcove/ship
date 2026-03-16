use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_core::Stream;
use futures_util::stream;
use ship_types::{AgentKind, Role, SessionEvent};

use crate::{
    AgentDriver, AgentError, AgentHandle, AgentSessionConfig, AgentSpawnInfo, PromptResponse,
    StopReason,
};

#[derive(Debug, Clone)]
pub struct SpawnRecord {
    pub kind: AgentKind,
    pub role: Role,
    pub session_config: AgentSessionConfig,
    pub handle: AgentHandle,
}

#[derive(Debug, Clone)]
pub struct FakePromptScript {
    pub expected_handle: Option<AgentHandle>,
    pub response: Result<PromptResponse, AgentError>,
    pub events: Vec<SessionEvent>,
}

#[derive(Default)]
struct FakeAgentDriverInner {
    scripts: VecDeque<FakePromptScript>,
    notifications: HashMap<AgentHandle, VecDeque<SessionEvent>>,
    spawns: Vec<SpawnRecord>,
    prompts: Vec<(AgentHandle, Vec<ship_types::PromptContentPart>)>,
    cancelled: Vec<AgentHandle>,
    killed: Vec<AgentHandle>,
    model_sets: Vec<(AgentHandle, String)>,
    current_models: HashMap<AgentHandle, String>,
    set_model_errors: VecDeque<AgentError>,
}

// r[testability.no-subprocess-in-tests]
#[derive(Clone, Default)]
pub struct FakeAgentDriver {
    inner: Arc<Mutex<FakeAgentDriverInner>>,
}

impl FakeAgentDriver {
    pub fn push_script(&self, script: FakePromptScript) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .scripts
            .push_back(script);
    }

    pub fn push_response(&self, stop_reason: StopReason) {
        self.push_script(FakePromptScript {
            expected_handle: None,
            response: Ok(PromptResponse { stop_reason }),
            events: Vec::new(),
        });
    }

    pub fn queue_notifications(&self, handle: &AgentHandle, events: Vec<SessionEvent>) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .notifications
            .entry(handle.clone())
            .or_default()
            .extend(events);
    }

    pub fn spawn_records(&self) -> Vec<SpawnRecord> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .spawns
            .clone()
    }

    pub fn prompt_log(&self) -> Vec<(AgentHandle, Vec<ship_types::PromptContentPart>)> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .prompts
            .clone()
    }

    pub fn cancelled_handles(&self) -> Vec<AgentHandle> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .cancelled
            .clone()
    }

    pub fn killed_handles(&self) -> Vec<AgentHandle> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .killed
            .clone()
    }

    pub fn model_set_log(&self) -> Vec<(AgentHandle, String)> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .model_sets
            .clone()
    }

    pub fn current_model(&self, handle: &AgentHandle) -> Option<String> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .current_models
            .get(handle)
            .cloned()
    }

    pub fn set_current_model_for_test(&self, handle: &AgentHandle, model_id: impl Into<String>) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .current_models
            .insert(handle.clone(), model_id.into());
    }

    pub fn push_set_model_error(&self, error: AgentError) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .set_model_errors
            .push_back(error);
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock().expect("fake agent driver mutex poisoned");
        inner.scripts.clear();
        inner.notifications.clear();
        inner.prompts.clear();
        inner.cancelled.clear();
        inner.killed.clear();
    }
}

#[async_trait::async_trait]
impl AgentDriver for FakeAgentDriver {
    async fn spawn(
        &self,
        kind: AgentKind,
        role: Role,
        config: &AgentSessionConfig,
    ) -> Result<AgentSpawnInfo, AgentError> {
        let handle = AgentHandle::new(crate::AcpSessionId::new());

        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .spawns
            .push(SpawnRecord {
                kind,
                role,
                session_config: config.clone(),
                handle: handle.clone(),
            });

        Ok(AgentSpawnInfo {
            handle: handle.clone(),
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
            acp_session_id: "fake-acp-session".to_owned(),
            was_resumed: false,
            protocol_version: 0,
            agent_name: None,
            agent_version: None,
            cap_load_session: false,
            cap_resume_session: false,
            cap_prompt_image: false,
            cap_prompt_audio: false,
            cap_prompt_embedded_context: false,
            cap_mcp_http: false,
            cap_mcp_sse: false,
        })
    }

    async fn prompt(
        &self,
        handle: &AgentHandle,
        parts: &[ship_types::PromptContentPart],
    ) -> Result<PromptResponse, AgentError> {
        let mut inner = self.inner.lock().expect("fake agent driver mutex poisoned");
        inner.prompts.push((handle.clone(), parts.to_owned()));

        let script = inner
            .scripts
            .pop_front()
            .unwrap_or_else(|| FakePromptScript {
                expected_handle: None,
                response: Ok(PromptResponse {
                    stop_reason: StopReason::EndTurn,
                }),
                events: Vec::new(),
            });

        if let Some(expected) = script.expected_handle
            && expected != *handle
        {
            return Err(AgentError {
                message: "prompt called with unexpected handle".to_owned(),
            });
        }

        if !script.events.is_empty() {
            inner
                .notifications
                .entry(handle.clone())
                .or_default()
                .extend(script.events);
        }

        script.response
    }

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .cancelled
            .push(handle.clone());
        Ok(())
    }

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>> {
        let events = self
            .inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .notifications
            .remove(handle)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();

        Box::pin(stream::iter(events))
    }

    async fn resolve_permission(
        &self,
        _handle: &AgentHandle,
        _permission_id: &str,
        _option_id: &str,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    async fn set_model(&self, handle: &AgentHandle, model_id: &str) -> Result<(), AgentError> {
        let mut inner = self.inner.lock().expect("fake agent driver mutex poisoned");
        if let Some(error) = inner.set_model_errors.pop_front() {
            return Err(error);
        }
        inner.model_sets.push((handle.clone(), model_id.to_owned()));
        inner
            .current_models
            .insert(handle.clone(), model_id.to_owned());
        Ok(())
    }

    async fn set_effort(
        &self,
        _handle: &AgentHandle,
        _config_id: &str,
        _value_id: &str,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .killed
            .push(handle.clone());
        Ok(())
    }
}

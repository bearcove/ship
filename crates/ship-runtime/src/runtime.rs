use ship_db::ShipDb;
use jiff::Timestamp;
use ship_policy::{
    AgentRole, Block, BlockContent, BlockId, Delivery, Participant, ParticipantName, RoomId,
    Task, TaskId, TaskPhase, Topology,
};
use ulid::Ulid;

use crate::events::{FrontendEvent, RoomSummary};
use crate::room::{Feed, Room, RoomState};
use tokio::sync::broadcast;

const RECENT_BLOCKS_LIMIT: usize = 100;

/// Snapshot of a room's state, sent to the frontend on connect.
pub struct RoomSnapshot {
    pub room_id: RoomId,
    pub current_task: Option<Task>,
    pub recent_blocks: Vec<Block>,
}

/// Everything the frontend needs on connect.
pub struct ConnectSnapshot {
    pub topology: Topology,
    pub rooms: Vec<RoomSnapshot>,
}

/// In-memory room state. Borrowed independently from the db.
struct Rooms {
    rooms: Vec<Room>,
}

impl Rooms {
    fn new(topology: &Topology) -> Self {
        let rooms = topology
            .lanes
            .iter()
            .map(|s| Room::cold(s.id.clone()))
            .collect();
        Self { rooms }
    }

    fn get_mut(&mut self, room_id: &RoomId) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|r| &r.id == room_id)
    }

    /// Ensure a room is warm, hydrating from db if needed.
    fn ensure_warm(&mut self, room_id: &RoomId, db: &ShipDb) -> Result<&mut Feed, RuntimeError> {
        let room = self
            .get_mut(room_id)
            .ok_or_else(|| RuntimeError::RoomNotFound(room_id.clone()))?;

        if matches!(room.state, RoomState::Cold) {
            let blocks = db.list_blocks(room_id).map_err(RuntimeError::Db)?;
            let feed = room.ensure_warm();
            feed.hydrate(blocks);
        }

        Ok(room.feed_mut().expect("just warmed"))
    }
}

/// The ship runtime: manages the topology of rooms, routes messages
/// through policy, persists to ship-db, and broadcasts to subscribers.
pub struct Runtime {
    db: ShipDb,
    topology: Topology,
    rooms: Rooms,
    tx: broadcast::Sender<FrontendEvent>,
}

impl Runtime {
    /// Create a new runtime backed by the given database.
    /// Loads topology from db if one exists.
    pub fn new(db: ShipDb) -> Self {
        let topology = db
            .load_topology()
            .ok()
            .flatten()
            .unwrap_or_else(empty_topology);
        let rooms = Rooms::new(&topology);
        let (tx, _) = broadcast::channel(256);
        Self {
            db,
            topology,
            rooms,
            tx,
        }
    }

    /// Initialize a fresh topology and persist it.
    pub fn init_topology(&mut self, topology: Topology) -> Result<(), ship_db::StoreError> {
        self.db.save_topology(&topology)?;
        self.rooms = Rooms::new(&topology);
        self.topology = topology;
        Ok(())
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Subscribe to the frontend event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<FrontendEvent> {
        self.tx.subscribe()
    }

    /// Emit a frontend event (best-effort, no error if nobody is listening).
    fn emit(&self, event: FrontendEvent) {
        let _ = self.tx.send(event);
    }

    /// Open a new unsealed block in a room. Persists to db immediately.
    ///
    /// Validates that the sender (if any) is a member of the room.
    pub fn open_block(
        &mut self,
        room_id: &RoomId,
        from: Option<ParticipantName>,
        to: Option<ParticipantName>,
        content: BlockContent,
    ) -> Result<BlockId, RuntimeError> {
        if let Some(ref sender_name) = from {
            self.check_room_membership(room_id, sender_name)?;
        }
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        let block = feed.open_block(room_id.clone(), from, to, content);
        let block_clone = block.clone();
        self.db.insert_block(&block_clone).map_err(RuntimeError::Db)?;
        self.emit(FrontendEvent::BlockChanged {
            room_id: room_id.clone(),
            block: block_clone.clone(),
        });
        Ok(block_clone.id)
    }

    /// Seal a block (mark it as finalized). Persists to db.
    ///
    /// After sealing, runs the block through policy: parses mentions,
    /// checks allowed_mentions, and routes deliveries.
    /// Returns the list of deliveries produced (may be empty).
    pub fn seal_block(
        &mut self,
        room_id: &RoomId,
        block_id: &BlockId,
    ) -> Result<Vec<Delivery>, RuntimeError> {
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        let block = feed
            .seal_block(block_id)
            .ok_or_else(|| RuntimeError::BlockNotFound(block_id.clone()))?;
        let sealed_at = block.sealed_at.expect("just sealed");
        let content = block.content.clone();
        let from_name = block.from.as_ref().map(|p| p.to_string());
        let sealed_block = block.clone();
        self.db
            .seal_block(block_id, sealed_at, &content)
            .map_err(RuntimeError::Db)?;
        self.emit(FrontendEvent::BlockChanged {
            room_id: room_id.clone(),
            block: sealed_block,
        });

        let deliveries = self.route_sealed_block(&content, from_name.as_deref());
        Ok(deliveries)
    }

    /// Check that a participant is a member of the given room.
    fn check_room_membership(
        &self,
        room_id: &RoomId,
        sender: &ParticipantName,
    ) -> Result<(), RuntimeError> {
        let members = self.topology.lane_members(room_id);
        let is_member = members
            .as_ref()
            .is_some_and(|m| m.iter().any(|p| p.name == *sender));
        if !is_member {
            return Err(RuntimeError::NotAMember {
                participant: sender.clone(),
                room: room_id.clone(),
            });
        }
        Ok(())
    }

    /// Process deliveries by creating blocks in the appropriate rooms.
    /// Returns the number of deliveries that were successfully delivered.
    pub fn process_deliveries(&mut self, deliveries: Vec<Delivery>) -> Result<usize, RuntimeError> {
        let mut delivered = 0;
        for delivery in deliveries {
            // Find the room the recipient belongs to.
            let room_id = self.room_for_participant(delivery.to.as_str());
            let Some(room_id) = room_id else {
                tracing::warn!(
                    to = %delivery.to,
                    "delivery recipient not found in any room, dropping"
                );
                continue;
            };

            let content = delivery_to_block_content(&delivery);
            let from = delivery.from.clone();
            let to = delivery.to.clone();

            let block_id = self.open_block(
                &room_id,
                Some(from),
                Some(to),
                content,
            )?;
            // Deliveries are complete — seal immediately.
            // Don't re-route (that would cause infinite loops).
            let feed = self.rooms.ensure_warm(&room_id, &self.db)?;
            let block = feed
                .seal_block(&block_id)
                .expect("just opened this block");
            let sealed_at = block.sealed_at.expect("just sealed");
            let content = block.content.clone();
            self.db
                .seal_block(&block_id, sealed_at, &content)
                .map_err(RuntimeError::Db)?;
            delivered += 1;
        }
        Ok(delivered)
    }

    // ── Task lifecycle ───────────────────────────────────────────────

    /// Assign a new task to a lane. The lane must not have an active task.
    pub fn assign_task(
        &mut self,
        room_id: &RoomId,
        title: String,
        description: String,
    ) -> Result<TaskId, RuntimeError> {
        // Check there's no active task.
        let existing = self.db.current_task(room_id).map_err(RuntimeError::Db)?;
        if let Some(existing) = existing {
            if !existing.phase.is_terminal() {
                return Err(RuntimeError::TaskAlreadyActive {
                    room: room_id.clone(),
                    task: existing.id,
                });
            }
        }

        let task = Task {
            id: TaskId::new(Ulid::new().to_string()),
            room_id: room_id.clone(),
            title,
            description,
            phase: TaskPhase::Assigned,
            created_at: Timestamp::now(),
            completed_at: None,
        };

        self.db.insert_task(&task).map_err(RuntimeError::Db)?;
        self.db
            .set_current_task(room_id, Some(&task.id))
            .map_err(RuntimeError::Db)?;
        let task_id = task.id.clone();
        self.emit(FrontendEvent::TaskChanged {
            room_id: room_id.clone(),
            task,
        });
        Ok(task_id)
    }

    /// Transition a task to a new phase. Validates via policy.
    pub fn transition_task(
        &mut self,
        room_id: &RoomId,
        new_phase: TaskPhase,
    ) -> Result<(), RuntimeError> {
        let task = self
            .db
            .current_task(room_id)
            .map_err(RuntimeError::Db)?
            .ok_or_else(|| RuntimeError::NoActiveTask(room_id.clone()))?;

        if !ship_policy::can_transition(task.phase, new_phase) {
            return Err(RuntimeError::InvalidTransition {
                task: task.id,
                from: task.phase,
                to: new_phase,
            });
        }

        let completed_at = if new_phase.is_terminal() {
            Some(Timestamp::now())
        } else {
            None
        };

        self.db
            .update_task_phase(&task.id, new_phase, completed_at)
            .map_err(RuntimeError::Db)?;

        if new_phase.is_terminal() {
            self.db
                .set_current_task(room_id, None)
                .map_err(RuntimeError::Db)?;
            self.emit(FrontendEvent::TaskCleared {
                room_id: room_id.clone(),
            });
        } else {
            // Re-read to get updated task for the event.
            if let Ok(Some(updated)) = self.db.current_task(room_id) {
                self.emit(FrontendEvent::TaskChanged {
                    room_id: room_id.clone(),
                    task: updated,
                });
            }
        }

        Ok(())
    }

    /// Get the current task for a room (if any).
    pub fn current_task(&self, room_id: &RoomId) -> Result<Option<Task>, RuntimeError> {
        self.db.current_task(room_id).map_err(RuntimeError::Db)
    }

    /// Get the current task phase for a room (None if no active task).
    pub fn current_phase(&self, room_id: &RoomId) -> Result<Option<TaskPhase>, RuntimeError> {
        Ok(self.current_task(room_id)?.map(|t| t.phase))
    }

    /// Find which room a participant belongs to.
    fn room_for_participant(&self, name: &str) -> Option<RoomId> {
        self.topology
            .lane_for_participant(name.into())
            .map(|s| s.id.clone())
    }

    /// Parse mentions and route deliveries for a sealed block.
    fn route_sealed_block(
        &self,
        content: &BlockContent,
        from_name: Option<&str>,
    ) -> Vec<Delivery> {
        let Some(from_name) = from_name else {
            return vec![];
        };

        let text = match content {
            BlockContent::Text { text } => text,
            _ => return vec![],
        };

        let mention = ship_policy::parse_mention(text, &self.topology);
        match mention {
            ship_policy::ParsedMention::Found { name, rest } => {
                let sender = self.topology.find_participant(from_name.into());
                let allowed = sender
                    .map(|s| ship_policy::allowed_mentions(&self.topology, s))
                    .unwrap_or_default();

                if allowed.iter().any(|a| a == &name) {
                    let action = ship_policy::Action::MessageSent {
                        from: ParticipantName::new(from_name.to_owned()),
                        mention: name,
                        text: rest,
                    };
                    ship_policy::route(&action, &self.topology)
                } else {
                    vec![]
                }
            }
            ship_policy::ParsedMention::None => {
                let action = ship_policy::Action::UnaddressedMessage {
                    from: ParticipantName::new(from_name.to_owned()),
                    text: text.clone(),
                };
                ship_policy::route(&action, &self.topology)
            }
            _ => vec![],
        }
    }

    /// Update an unsealed block's content. Persists to db.
    pub fn update_block(
        &mut self,
        room_id: &RoomId,
        block_id: &BlockId,
        content: BlockContent,
    ) -> Result<(), RuntimeError> {
        self.db
            .update_block_content(block_id, &content)
            .map_err(RuntimeError::Db)?;
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        let updated = feed.update_block(block_id, content).cloned();
        if let Some(block) = updated {
            self.emit(FrontendEvent::BlockChanged {
                room_id: room_id.clone(),
                block,
            });
        }
        Ok(())
    }

    /// Get the feed blocks for a room (warming it if needed).
    pub fn blocks(&mut self, room_id: &RoomId) -> Result<&[Block], RuntimeError> {
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        Ok(feed.blocks())
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    Db(ship_db::StoreError),
    RoomNotFound(RoomId),
    BlockNotFound(BlockId),
    NotAMember {
        participant: ParticipantName,
        room: RoomId,
    },
    TaskAlreadyActive {
        room: RoomId,
        task: TaskId,
    },
    NoActiveTask(RoomId),
    InvalidTransition {
        task: TaskId,
        from: TaskPhase,
        to: TaskPhase,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::RoomNotFound(id) => write!(f, "room not found: {id}"),
            Self::BlockNotFound(id) => write!(f, "block not found: {id}"),
            Self::NotAMember { participant, room } => {
                write!(f, "{participant} is not a member of room {room}")
            }
            Self::TaskAlreadyActive { room, task } => {
                write!(f, "room {room} already has active task {task}")
            }
            Self::NoActiveTask(room) => write!(f, "room {room} has no active task"),
            Self::InvalidTransition { task, from, to } => {
                write!(f, "invalid transition for task {task}: {from:?} → {to:?}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

fn delivery_to_block_content(delivery: &Delivery) -> BlockContent {
    use ship_policy::DeliveryContent;
    match &delivery.content {
        DeliveryContent::Message { text } => BlockContent::Text {
            text: text.clone(),
        },
        DeliveryContent::Question { text } => BlockContent::Text {
            text: text.clone(),
        },
        DeliveryContent::Bounce { reason, allowed } => BlockContent::Error {
            message: format!(
                "{reason} Allowed recipients: {}",
                allowed.join(", ")
            ),
        },
        DeliveryContent::Denied {
            attempted_target,
            reason,
        } => BlockContent::Error {
            message: format!("Cannot message {attempted_target}: {reason}"),
        },
        DeliveryContent::Guidance { text } => BlockContent::Text {
            text: text.clone(),
        },
        DeliveryContent::Submitted { summary } => BlockContent::Milestone {
            kind: ship_policy::MilestoneKind::ReviewSubmitted,
            title: "Work submitted for review".to_owned(),
            summary: summary.clone(),
        },
        DeliveryContent::Committed {
            step,
            commit_summary,
            diff_section,
        } => BlockContent::Milestone {
            kind: ship_policy::MilestoneKind::StepCommitted,
            title: step
                .as_deref()
                .unwrap_or("Committed")
                .to_owned(),
            summary: format!("{commit_summary}\n{diff_section}"),
        },
        DeliveryContent::PlanSet { plan_status } => BlockContent::Milestone {
            kind: ship_policy::MilestoneKind::PlanSet,
            title: "Plan updated".to_owned(),
            summary: plan_status.clone(),
        },
        DeliveryContent::ActivitySummary { summary } => BlockContent::Text {
            text: summary.clone(),
        },
        DeliveryContent::TaskAssigned { title, description } => BlockContent::Milestone {
            kind: ship_policy::MilestoneKind::TaskAccepted,
            title: title.clone(),
            summary: description.clone(),
        },
        DeliveryContent::ChecksStarted { context } => BlockContent::Text {
            text: format!("CI checks started: {context}"),
        },
        DeliveryContent::ChecksFinished {
            context,
            all_passed,
            summary,
        } => BlockContent::Text {
            text: if *all_passed {
                format!("CI checks passed: {context}\n{summary}")
            } else {
                format!("CI checks failed: {context}\n{summary}")
            },
        },
    }
}

fn empty_topology() -> Topology {
    Topology {
        human: Participant::human("Human"),
        admiral: Participant::agent("Admiral", AgentRole::Admiral),
        lanes: vec![],
    }
}

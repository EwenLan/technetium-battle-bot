use std::time::{Duration, Instant};

use crate::ai::{DecisionState, decide, individual};
use crate::protocol::{decode, empty_response, encode};
use crate::rules::build::BuildMask;
use crate::rules::constants::HTTP_DECISION_TIMEOUT_MS;
use crate::world::World;

pub struct Session {
    world: World,
    decision: DecisionState,
    last_round: Option<i32>,
    last_team: Option<String>,
    last_key: Option<String>,
    last_reply: Vec<u8>,
    build_mask: Option<BuildMask>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Session {
    pub fn new(build_mask: Option<BuildMask>) -> Self {
        Self {
            world: World::default(),
            decision: DecisionState::default(),
            last_round: None,
            last_team: None,
            last_key: None,
            last_reply: Vec::new(),
            build_mask,
        }
    }

    pub fn handle(&mut self, body: &[u8]) -> Vec<u8> {
        let Ok(request) = decode(body) else {
            return empty_response();
        };
        let round = request.observation.round_no;
        if self.last_round == Some(round) {
            return self.same_round(&request.canonical_key);
        }
        if self.last_round.is_some_and(|last| round <= last) {
            return empty_response();
        }
        if self
            .last_team
            .as_ref()
            .is_some_and(|team| team != &request.observation.team_our.team_id)
        {
            return empty_response();
        }
        self.next_round(request.observation, request.canonical_key)
    }

    fn same_round(&self, key: &str) -> Vec<u8> {
        if self.last_key.as_deref() == Some(key) {
            return self.last_reply.clone();
        }
        empty_response()
    }

    fn next_round(&mut self, observation: crate::domain::Observation, key: String) -> Vec<u8> {
        let deadline = Instant::now() + Duration::from_millis(HTTP_DECISION_TIMEOUT_MS);
        self.world.apply(observation.clone());
        self.decision.reports = individual::reconcile(&observation, &mut self.decision);
        let mut draft = self.decision.clone();
        let planned = decide(
            &observation,
            &self.world.events,
            &mut draft,
            self.build_mask.as_ref(),
            deadline,
        );
        let encoded = encode(&planned).ok().filter(|_| Instant::now() <= deadline);
        let committed = encoded.is_some();
        let reply = encoded.unwrap_or_else(empty_response);
        self.last_team = Some(observation.team_our.team_id);
        self.last_round = Some(observation.round_no);
        self.last_key = Some(key);
        self.last_reply = reply.clone();
        if committed {
            self.decision = draft;
        }
        reply
    }
}

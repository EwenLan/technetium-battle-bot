use std::collections::VecDeque;

use crate::event::WorldEvent;
use crate::rules::constants::{MAX_EVENT_LOG_RECORDS, VERSION_INCREMENT};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord {
    pub id: u64,
    pub observed_round: i32,
    pub event: WorldEvent,
}

#[derive(Clone, Debug, Default)]
pub struct EventLog {
    records: VecDeque<EventRecord>,
    next_id: u64,
}

impl EventLog {
    pub fn append(&mut self, round: i32, events: &[WorldEvent]) {
        for event in events {
            self.records.push_back(EventRecord {
                id: self.next_id,
                observed_round: round,
                event: event.clone(),
            });
            self.next_id = self.next_id.saturating_add(VERSION_INCREMENT);
        }
        while self.records.len() > MAX_EVENT_LOG_RECORDS {
            self.records.pop_front();
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &EventRecord> {
        self.records.iter()
    }

    pub fn after(&self, cursor: Option<u64>) -> impl Iterator<Item = &EventRecord> {
        self.records
            .iter()
            .filter(move |record| cursor.is_none_or(|id| record.id > id))
    }
}

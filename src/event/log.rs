use std::collections::{BTreeMap, VecDeque};

use crate::event::WorldEvent;
use crate::rules::constants::{MAX_EVENT_LOG_RECORDS, VERSION_INCREMENT, ZERO_VERSION};

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReaderId {
    Strategy,
    Mission,
    Tactics,
    Individual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventCursor {
    next_id: u64,
}

impl Default for EventCursor {
    fn default() -> Self {
        Self {
            next_id: ZERO_VERSION,
        }
    }
}

impl EventCursor {
    pub const fn next_id(self) -> u64 {
        self.next_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveryReceipt {
    reader: ReaderId,
    start: EventCursor,
    next: EventCursor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryBatch {
    reader: ReaderId,
    start: EventCursor,
    next: EventCursor,
    records: Vec<EventRecord>,
    truncated: bool,
}

impl DeliveryBatch {
    pub fn records(&self) -> &[EventRecord] {
        &self.records
    }

    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    pub const fn receipt(&self) -> DeliveryReceipt {
        DeliveryReceipt {
            reader: self.reader,
            start: self.start,
            next: self.next,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgeError {
    StaleReceipt,
}

#[derive(Clone, Debug, Default)]
pub struct EventInbox {
    cursors: BTreeMap<ReaderId, EventCursor>,
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

impl EventInbox {
    pub fn cursor(&self, reader: ReaderId) -> EventCursor {
        self.cursors.get(&reader).copied().unwrap_or_default()
    }

    pub fn poll(&self, reader: ReaderId, log: &EventLog) -> DeliveryBatch {
        let start = self.cursor(reader);
        let truncated = log
            .records
            .front()
            .is_some_and(|record| record.id > start.next_id);
        let records: Vec<_> = log
            .records
            .iter()
            .filter(|record| record.id >= start.next_id)
            .cloned()
            .collect();
        let next = records
            .last()
            .map(|record| EventCursor {
                next_id: record.id.saturating_add(VERSION_INCREMENT),
            })
            .unwrap_or(start);
        DeliveryBatch {
            reader,
            start,
            next,
            records,
            truncated,
        }
    }

    pub fn acknowledge(&mut self, receipt: DeliveryReceipt) -> Result<(), AcknowledgeError> {
        if self.cursor(receipt.reader) != receipt.start {
            return Err(AcknowledgeError::StaleReceipt);
        }
        self.cursors.insert(receipt.reader, receipt.next);
        Ok(())
    }
}

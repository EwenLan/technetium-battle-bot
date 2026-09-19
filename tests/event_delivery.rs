use technetium_battle_bot::event::{AcknowledgeError, EventInbox, EventLog, ReaderId, WorldEvent};
use technetium_battle_bot::rules::constants::{
    EMPTY_COUNT, FIRST_ROUND, MAX_EVENT_LOG_RECORDS, MIN_ACTION_ROUNDS, SINGLE_ITEM,
    VERSION_INCREMENT, ZERO_VERSION,
};
use technetium_battle_bot::runtime::Session;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");

#[test]
fn readers_acknowledge_the_same_events_independently() {
    let log = log_with_rounds();
    let mut inbox = EventInbox::default();
    let strategy = inbox.poll(ReaderId::Strategy, &log);
    let mission = inbox.poll(ReaderId::Mission, &log);

    assert_eq!(strategy.records(), mission.records());
    assert!(!strategy.truncated());
    inbox
        .acknowledge(strategy.receipt())
        .expect("current receipt");
    assert_eq!(
        inbox.poll(ReaderId::Strategy, &log).records().len(),
        EMPTY_COUNT
    );
    assert_eq!(
        inbox.poll(ReaderId::Mission, &log).records().len(),
        mission.records().len()
    );
}

#[test]
fn a_stale_delivery_receipt_cannot_move_a_reader_cursor() {
    let log = log_with_rounds();
    let mut inbox = EventInbox::default();
    let first = inbox.poll(ReaderId::Tactics, &log);
    let duplicate = inbox.poll(ReaderId::Tactics, &log);

    inbox.acknowledge(first.receipt()).expect("current receipt");
    assert_eq!(
        inbox.acknowledge(duplicate.receipt()),
        Err(AcknowledgeError::StaleReceipt)
    );
}

#[test]
fn a_reader_detects_when_unread_history_was_truncated() {
    let mut log = EventLog::default();
    for sequence in EMPTY_COUNT..MAX_EVENT_LOG_RECORDS + SINGLE_ITEM {
        let round = i32::try_from(sequence).expect("bounded round") + FIRST_ROUND;
        log.append(round, &[WorldEvent::RoundStarted(round)]);
    }

    let delivery = EventInbox::default().poll(ReaderId::Individual, &log);
    assert!(delivery.truncated());
    assert_eq!(delivery.records().len(), MAX_EVENT_LOG_RECORDS);
    assert_eq!(
        delivery.records().first().expect("record").id,
        VERSION_INCREMENT
    );
}

#[test]
fn committed_strategy_turn_advances_its_cursor_once() {
    let mut session = Session::new();
    assert_eq!(
        session.event_cursor(ReaderId::Strategy).next_id(),
        ZERO_VERSION
    );

    let first_reply = session.handle(DAY_REQUEST.as_bytes());
    let after_first = session.event_cursor(ReaderId::Strategy);
    assert!(after_first.next_id() > ZERO_VERSION);
    assert_eq!(session.handle(DAY_REQUEST.as_bytes()), first_reply);
    assert_eq!(session.event_cursor(ReaderId::Strategy), after_first);
}

fn log_with_rounds() -> EventLog {
    let mut log = EventLog::default();
    log.append(FIRST_ROUND, &[WorldEvent::RoundStarted(FIRST_ROUND)]);
    let next_round = FIRST_ROUND + MIN_ACTION_ROUNDS;
    log.append(next_round, &[WorldEvent::RoundStarted(next_round)]);
    log
}

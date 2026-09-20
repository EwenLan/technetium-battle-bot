use super::{GoalEvidence, MissionRegistry};
use crate::domain::{Action, ActiveOwners, MissionSpec, Observation, OwnerAllocator, OwnerPath};
use crate::event::{EventRecord, WorldEvent};
use crate::fsm::MissionState;
use crate::protocol::decode;

const DAY_REQUEST: &str = include_str!("../../../tests/fixtures/day.json");
const WORKER_ID: i64 = 10;
const COLLECT_EVENT_ID: u64 = 70;
const SELL_EVENT_ID: u64 = 71;
const GOLD_EVENT_ID: u64 = 72;
const COLLECT_ROUND: i32 = 1;
const SELL_ROUND: i32 = 2;
const COMPLETE_ROUND: i32 = 3;
const START_GOLD: i32 = 75;
const END_GOLD: i32 = 78;
const SALE_QUANTITY: i32 = 1;
const ORE: &str = "iron";

#[test]
fn observed_collect_then_sell_completes_the_economy_cycle() {
    let (mut registry, owner, mission) = active_economy();
    let collected = record_collection(&mut registry, &owner);

    assert_eq!(
        registry.view(mission).expect("mission").progress_evidence(),
        &[event_evidence(COLLECT_EVENT_ID, SELL_ROUND)]
    );
    assert!(registry.record_action_commit(&owner, WORKER_ID, &sale_action(), &collected,));
    let sold = after_sale(&collected);
    let result = registry.reconcile(&sold, &sale_events());

    assert_eq!(
        result.resolved().first().map(|item| item.state()),
        Some(MissionState::Succeeded)
    );
    let view = registry.view(mission).expect("mission");
    assert_eq!(view.state(), MissionState::Succeeded);
    assert_eq!(
        view.completion_evidence().len(),
        sale_events().len() + crate::rules::constants::SINGLE_ITEM
    );
}

#[test]
fn sale_without_positive_gold_evidence_keeps_economy_pending() {
    let (mut registry, owner, mission) = active_economy();
    let collected = record_collection(&mut registry, &owner);
    assert!(registry.record_action_commit(&owner, WORKER_ID, &sale_action(), &collected,));
    let mut sold = after_sale(&collected);
    sold.team_our.gold_num = Some(START_GOLD);
    let events = [inventory_event(SELL_EVENT_ID, COMPLETE_ROUND)];

    assert!(registry.reconcile(&sold, &events).resolved().is_empty());
    let view = registry.view(mission).expect("mission");
    assert_eq!(view.state(), MissionState::Executing);
    assert_eq!(
        view.progress_evidence(),
        &[event_evidence(COLLECT_EVENT_ID, SELL_ROUND)]
    );
}

#[test]
fn selling_preexisting_inventory_does_not_complete_a_cycle() {
    let (mut registry, owner, mission) = active_economy();
    let mut initial = observation();
    role_mut(&mut initial).backpack.push(ORE.to_owned());
    assert!(registry.record_action_commit(&owner, WORKER_ID, &sale_action(), &initial,));
    let sold = after_sale(&initial);

    assert!(
        registry
            .reconcile(&sold, &sale_events())
            .resolved()
            .is_empty()
    );
    let view = registry.view(mission).expect("mission");
    assert_eq!(view.state(), MissionState::Executing);
    assert!(view.progress_evidence().is_empty());
}

fn active_economy() -> (MissionRegistry, OwnerPath, crate::domain::MissionId) {
    let mut registry = MissionRegistry::default();
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let owner = owners.create_assignment(&mut allocator).expect("owner");
    let mission = registry
        .register(WORKER_ID, MissionSpec::economy(), owner)
        .expect("mission");
    registry.assign(mission).expect("ready");
    registry.activate(mission).expect("assigned");
    (registry, owner, mission)
}

fn record_collection(registry: &mut MissionRegistry, owner: &OwnerPath) -> Observation {
    let initial = observation();
    let mine = initial
        .map_info
        .zones
        .iter()
        .find(|zone| zone.neutral_type == ORE)
        .expect("mine")
        .pos;
    assert!(registry.record_action_commit(owner, WORKER_ID, &Action::Collect(mine), &initial,));
    let collected = after_collect(&initial);
    registry.reconcile(&collected, &[inventory_event(COLLECT_EVENT_ID, SELL_ROUND)]);
    collected
}

fn observation() -> Observation {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    assert_eq!(observation.round_no, COLLECT_ROUND);
    assert_eq!(observation.team_our.gold_num, Some(START_GOLD));
    observation
}

fn after_collect(initial: &Observation) -> Observation {
    let mut collected = initial.clone();
    collected.round_no = SELL_ROUND;
    role_mut(&mut collected).backpack.push(ORE.to_owned());
    collected
}

fn after_sale(collected: &Observation) -> Observation {
    let mut sold = collected.clone();
    sold.round_no = COMPLETE_ROUND;
    role_mut(&mut sold).backpack.clear();
    sold.team_our.gold_num = Some(END_GOLD);
    sold
}

fn role_mut(observation: &mut Observation) -> &mut crate::domain::Role {
    observation
        .team_our
        .roles
        .iter_mut()
        .find(|role| role.id == WORKER_ID)
        .expect("worker")
}

fn sale_action() -> Action {
    Action::Sell {
        name: ORE.to_owned(),
        quantity: SALE_QUANTITY,
    }
}

fn sale_events() -> Vec<EventRecord> {
    vec![
        inventory_event(SELL_EVENT_ID, COMPLETE_ROUND),
        EventRecord {
            id: GOLD_EVENT_ID,
            observed_round: COMPLETE_ROUND,
            event: WorldEvent::GoldChanged {
                previous: START_GOLD,
                current: END_GOLD,
            },
        },
    ]
}

fn inventory_event(id: u64, round: i32) -> EventRecord {
    EventRecord {
        id,
        observed_round: round,
        event: WorldEvent::InventoryChanged(WORKER_ID),
    }
}

fn event_evidence(event: u64, observed_round: i32) -> GoalEvidence {
    GoalEvidence::EventObserved {
        event,
        observed_round,
    }
}

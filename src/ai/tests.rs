use super::{DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::{
    Action, DeadlineKind, MissionDeadline, MissionId, MissionSpec, OwnerPath, Pos, Role,
};
use crate::fsm::MissionState;
use crate::protocol::decode;

const DAY_REQUEST: &str = include_str!("../../tests/fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("../../tests/fixtures/night.json");
const WEAPON_ID: i64 = 20;
const WORKER_ID: i64 = 10;
const MOVE_X: i32 = 4;
const MOVE_Y: i32 = 4;
const OTHER_SITE_X: i32 = 5;
const OTHER_SITE_Y: i32 = 4;
const BUILDING_KIND: &str = "gatling";
const BUILDING_ID: i64 = 90;
const BUILDING_HP: i32 = 1000;
const NEXT_ROUND: i32 = 2;
const PIONEER_ID: i64 = 11;
const PIONEER_MOVE_X: i32 = 4;
const PIONEER_MOVE_Y: i32 = 2;

#[test]
fn rejected_owned_proposal_preserves_existing_work() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let worker = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.id == WORKER_ID)
        .expect("worker");
    let mut arbiter = Arbiter::new(&observation);
    let mut state = DecisionState::default();
    let previous = accept_move(&observation, &mut state, MissionSpec::economy());

    let accepted = propose_owned(
        &observation,
        &mut state,
        &mut arbiter,
        WORKER_ID,
        Action::Move(worker.pos),
        MissionSpec::construction(
            Pos {
                x: OTHER_SITE_X,
                y: OTHER_SITE_Y,
            },
            BUILDING_KIND,
        ),
    )
    .expect("proposal");

    assert!(!accepted);
    assert!(state.active_owners.is_current(&previous));
    assert_eq!(state.role_owners.get(&WORKER_ID), Some(&previous));
    assert_eq!(mission_state(&state, &previous), MissionState::Executing);
    assert!(
        arbiter
            .finish(&state.active_owners)
            .response
            .role_command_map
            .is_empty()
    );
}

#[test]
fn attack_owner_is_committed_to_the_controller() {
    let observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    let target = observation
        .robot
        .as_ref()
        .expect("robots")
        .roles
        .first()
        .expect("robot")
        .pos;
    let mut arbiter = Arbiter::new(&observation);
    let mut state = DecisionState::default();

    assert!(
        propose_owned(
            &observation,
            &mut state,
            &mut arbiter,
            WEAPON_ID,
            Action::Attack {
                controller: WORKER_ID,
                targets: vec![target],
            },
            MissionSpec::defense(WEAPON_ID),
        )
        .expect("proposal")
    );

    let owner = state.role_owners.get(&WORKER_ID).expect("controller owner");
    assert!(state.active_owners.is_current(owner));
    assert!(!state.role_owners.contains_key(&WEAPON_ID));
}

#[test]
fn successive_steps_reuse_assignment_and_replace_intent() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();

    let first = accept_move(&observation, &mut state, MissionSpec::economy());
    let second = accept_move(&observation, &mut state, MissionSpec::economy());

    assert_eq!(first.mission(), second.mission());
    assert_eq!(first.plan(), second.plan());
    assert_ne!(first.intent(), second.intent());
    assert!(!state.active_owners.is_current(&first));
    assert!(state.active_owners.is_current(&second));
}

#[test]
fn blocked_assignment_resumes_when_the_next_action_is_accepted() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let first = accept_move(&observation, &mut state, MissionSpec::economy());
    state.block_mission(&first);
    assert_eq!(mission_state(&state, &first), MissionState::Blocked);

    let second = accept_move(&observation, &mut state, MissionSpec::economy());

    assert_eq!(first.mission(), second.mission());
    assert_eq!(mission_state(&state, &second), MissionState::Executing);
}

#[test]
fn changing_mission_spec_replaces_the_assignment() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();

    let economy = accept_move(&observation, &mut state, MissionSpec::economy());
    let construction = accept_move(
        &observation,
        &mut state,
        MissionSpec::construction(
            Pos {
                x: MOVE_X,
                y: MOVE_Y,
            },
            BUILDING_KIND,
        ),
    );

    assert_ne!(economy.mission(), construction.mission());
    assert_ne!(economy.plan(), construction.plan());
    assert!(!state.active_owners.is_current(&economy));
    assert!(state.active_owners.is_current(&construction));
    assert_eq!(mission_state(&state, &economy), MissionState::Cancelled);
    assert_eq!(
        mission_state(&state, &construction),
        MissionState::Executing
    );
}

#[test]
fn changing_objective_replaces_assignment_within_the_same_kind() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();

    let first = accept_move(
        &observation,
        &mut state,
        MissionSpec::construction(
            Pos {
                x: MOVE_X,
                y: MOVE_Y,
            },
            BUILDING_KIND,
        ),
    );
    let second = accept_move(
        &observation,
        &mut state,
        MissionSpec::construction(
            Pos {
                x: OTHER_SITE_X,
                y: OTHER_SITE_Y,
            },
            BUILDING_KIND,
        ),
    );

    assert_ne!(first.mission(), second.mission());
    assert_ne!(first.plan(), second.plan());
    assert!(!state.active_owners.is_current(&first));
    assert!(state.active_owners.is_current(&second));
    assert_eq!(mission_state(&state, &first), MissionState::Cancelled);
    assert_eq!(mission_state(&state, &second), MissionState::Executing);
}

#[test]
fn discarded_draft_does_not_advance_persistent_owner_ids() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let state = DecisionState::default();
    let mut discarded = state.clone();
    let discarded_owner = accept_move(&observation, &mut discarded, MissionSpec::economy());
    let mut committed = state;

    let committed_owner = accept_move(&observation, &mut committed, MissionSpec::economy());

    assert_eq!(discarded_owner, committed_owner);
}

#[test]
fn mission_reconciliation_deactivates_a_completed_assignment() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let site = Pos {
        x: MOVE_X,
        y: MOVE_Y,
    };
    let owner = accept_move(
        &observation,
        &mut state,
        MissionSpec::construction(site, BUILDING_KIND),
    );
    observation.team_our.roles.push(Role {
        id: BUILDING_ID,
        pos: site,
        role_type: BUILDING_KIND.to_owned(),
        health: Some(BUILDING_HP),
        attack_power: None,
        attack_range: None,
        back_pack_capability: None,
        backpack: Vec::new(),
        level: None,
        cooldown: None,
    });

    state.reconcile_missions(&observation, &[]);

    assert_eq!(mission_state(&state, &owner), MissionState::Succeeded);
    assert!(!state.active_owners.is_current(&owner));
    assert!(!state.role_owners.contains_key(&WORKER_ID));
}

#[test]
fn timely_action_submission_keeps_the_mission_open_for_effects() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let deadline = MissionDeadline::new(observation.round_no, true, DeadlineKind::ActionSubmission);
    let site = Pos {
        x: MOVE_X,
        y: MOVE_Y,
    };
    let action = Action::Build {
        name: BUILDING_KIND.to_owned(),
        pos: site,
    };
    let mut arbiter = Arbiter::new(&observation);
    assert!(
        propose_owned(
            &observation,
            &mut state,
            &mut arbiter,
            WORKER_ID,
            action,
            MissionSpec::construction(site, BUILDING_KIND).with_deadline(deadline),
        )
        .expect("proposal")
    );
    let arbitration = arbiter.finish(&state.active_owners);
    super::individual::record_committed(&observation, &arbitration, &mut state);
    let owner = *state.role_owners.get(&WORKER_ID).expect("role owner");
    observation.round_no = NEXT_ROUND;
    let late_action = Action::Build {
        name: BUILDING_KIND.to_owned(),
        pos: site,
    };
    state.record_mission_action(&owner, WORKER_ID, &late_action, &observation);

    state.reconcile_missions(&observation, &[]);

    assert_eq!(mission_state(&state, &owner), MissionState::Executing);
    assert!(state.active_owners.is_current(&owner));
    assert_eq!(
        state
            .mission_view(owner.mission().id)
            .expect("mission view")
            .goal_submission_round(),
        Some(deadline.at_round())
    );
}

#[test]
fn capability_filter_rejects_before_allocating_an_assignment() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let mut arbiter = Arbiter::new(&observation);
    let action = Action::Move(Pos {
        x: PIONEER_MOVE_X,
        y: PIONEER_MOVE_Y,
    });

    assert!(
        !propose_owned(
            &observation,
            &mut state,
            &mut arbiter,
            PIONEER_ID,
            action.clone(),
            MissionSpec::economy(),
        )
        .expect("capability filter")
    );
    assert!(state.role_owners.is_empty());
    assert!(
        propose_owned(
            &observation,
            &mut state,
            &mut arbiter,
            PIONEER_ID,
            action,
            MissionSpec::challenge(),
        )
        .expect("challenge proposal")
    );
    assert!(
        state
            .mission_view(MissionId::new(crate::rules::constants::ZERO_VERSION))
            .is_some()
    );
}

fn accept_move(
    observation: &crate::domain::Observation,
    state: &mut DecisionState,
    spec: MissionSpec,
) -> OwnerPath {
    let mut arbiter = Arbiter::new(observation);
    let action = Action::Move(Pos {
        x: MOVE_X,
        y: MOVE_Y,
    });
    assert!(
        propose_owned(observation, state, &mut arbiter, WORKER_ID, action, spec,)
            .expect("proposal")
    );
    let arbitration = arbiter.finish(&state.active_owners);
    assert!(
        arbitration
            .response
            .role_command_map
            .contains_key(&WORKER_ID.to_string())
    );
    *state.role_owners.get(&WORKER_ID).expect("role owner")
}

fn mission_state(state: &DecisionState, owner: &OwnerPath) -> MissionState {
    state
        .mission_view(owner.mission().id)
        .expect("mission record")
        .state()
}

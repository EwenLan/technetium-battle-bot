use crate::domain::{Action, GoalPredicate, Observation};
use crate::event::{EventRecord, WorldEvent};
use crate::rules::constants::MIN_ACTION_ROUNDS;

use super::goal::{GoalEvaluation, GoalEvidence};
use super::record::MissionRecord;

#[derive(Clone, Debug, Default)]
pub(super) struct EconomyProgress {
    checkpoint: Option<EconomyCheckpoint>,
    evidence: Vec<GoalEvidence>,
    sale_observed: bool,
}

#[derive(Clone, Debug)]
struct EconomyCheckpoint {
    effect_round: i32,
    action: EconomyAction,
}

#[derive(Clone, Debug)]
enum EconomyAction {
    Collect {
        item: String,
        before: usize,
    },
    Sell {
        item: String,
        remaining: usize,
        gold_before: i32,
    },
}

impl EconomyProgress {
    pub(super) fn evidence(&self) -> &[GoalEvidence] {
        &self.evidence
    }

    fn evaluation(&self) -> GoalEvaluation {
        if self.sale_observed {
            GoalEvaluation::Satisfied(self.evidence.clone())
        } else {
            GoalEvaluation::Pending
        }
    }
}

pub(super) fn record_action(record: &mut MissionRecord, action: &Action, input: &Observation) {
    if record.spec.goal() != &GoalPredicate::EconomyCycleCompleted {
        return;
    }
    record.economy_progress.checkpoint = checkpoint(action, record.assignee, input);
}

pub(super) fn reconcile(record: &mut MissionRecord, input: &Observation, events: &[EventRecord]) {
    let Some(checkpoint) = record.economy_progress.checkpoint.take() else {
        return;
    };
    if checkpoint.effect_round != input.round_no {
        return;
    }
    match checkpoint.action {
        EconomyAction::Collect { item, before } => {
            reconcile_collect(record, input, events, &item, before)
        }
        EconomyAction::Sell {
            item,
            remaining,
            gold_before,
        } => reconcile_sale(record, input, events, &item, remaining, gold_before),
    }
}

pub(super) fn evaluation(record: &MissionRecord) -> Option<GoalEvaluation> {
    (record.spec.goal() == &GoalPredicate::EconomyCycleCompleted)
        .then(|| record.economy_progress.evaluation())
}

fn checkpoint(action: &Action, assignee: i64, input: &Observation) -> Option<EconomyCheckpoint> {
    let role = input
        .team_our
        .roles
        .iter()
        .find(|role| role.id == assignee)?;
    let action = match action {
        Action::Collect(pos) => collect_checkpoint(input, role, *pos)?,
        Action::Sell { name, quantity } => sell_checkpoint(input, role, name, *quantity)?,
        _ => return None,
    };
    Some(EconomyCheckpoint {
        effect_round: input.round_no.checked_add(MIN_ACTION_ROUNDS)?,
        action,
    })
}

fn collect_checkpoint(
    input: &Observation,
    role: &crate::domain::Role,
    pos: crate::domain::Pos,
) -> Option<EconomyAction> {
    let item = input
        .map_info
        .zones
        .iter()
        .find(|zone| zone.pos == pos)?
        .neutral_type
        .clone();
    Some(EconomyAction::Collect {
        before: item_count(role, &item),
        item,
    })
}

fn sell_checkpoint(
    input: &Observation,
    role: &crate::domain::Role,
    item: &str,
    quantity: i32,
) -> Option<EconomyAction> {
    let quantity = usize::try_from(quantity).ok()?;
    let remaining = item_count(role, item).checked_sub(quantity)?;
    Some(EconomyAction::Sell {
        item: item.to_owned(),
        remaining,
        gold_before: input.team_our.gold_num?,
    })
}

fn reconcile_collect(
    record: &mut MissionRecord,
    input: &Observation,
    events: &[EventRecord],
    item: &str,
    before: usize,
) {
    if current_item_count(input, record.assignee, item).is_none_or(|count| count <= before) {
        return;
    }
    if let Some(evidence) = inventory_evidence(events, record.assignee, input.round_no) {
        add_evidence(&mut record.economy_progress.evidence, evidence);
    }
}

fn reconcile_sale(
    record: &mut MissionRecord,
    input: &Observation,
    events: &[EventRecord],
    item: &str,
    remaining: usize,
    gold_before: i32,
) {
    if record.economy_progress.evidence.is_empty()
        || current_item_count(input, record.assignee, item).is_none_or(|count| count > remaining)
    {
        return;
    }
    let inventory = inventory_evidence(events, record.assignee, input.round_no);
    let gold = gold_evidence(events, input.round_no, gold_before);
    let Some((inventory, gold)) = inventory.zip(gold) else {
        return;
    };
    add_evidence(&mut record.economy_progress.evidence, inventory);
    add_evidence(&mut record.economy_progress.evidence, gold);
    record.economy_progress.sale_observed = true;
}

fn inventory_evidence(events: &[EventRecord], actor: i64, round: i32) -> Option<GoalEvidence> {
    event_evidence(
        events,
        round,
        |event| matches!(event, WorldEvent::InventoryChanged(id) if *id == actor),
    )
}

fn gold_evidence(events: &[EventRecord], round: i32, before: i32) -> Option<GoalEvidence> {
    event_evidence(events, round, |event| {
        matches!(event, WorldEvent::GoldChanged { previous, current }
            if *previous == before && current > previous)
    })
}

fn event_evidence(
    events: &[EventRecord],
    round: i32,
    predicate: impl Fn(&WorldEvent) -> bool,
) -> Option<GoalEvidence> {
    events
        .iter()
        .find(|record| record.observed_round == round && predicate(&record.event))
        .map(|record| GoalEvidence::EventObserved {
            event: record.id,
            observed_round: record.observed_round,
        })
}

fn current_item_count(input: &Observation, actor: i64, item: &str) -> Option<usize> {
    input
        .team_our
        .roles
        .iter()
        .find(|role| role.id == actor)
        .map(|role| item_count(role, item))
}

fn item_count(role: &crate::domain::Role, item: &str) -> usize {
    role.backpack
        .iter()
        .filter(|candidate| candidate.as_str() == item)
        .count()
}

fn add_evidence(evidence: &mut Vec<GoalEvidence>, item: GoalEvidence) {
    if !evidence.contains(&item) {
        evidence.push(item);
        evidence.sort_unstable();
    }
}

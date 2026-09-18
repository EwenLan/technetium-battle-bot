use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::domain::Pos;
use crate::rules::constants::{MAX_PATH_EXPANSIONS, PATH_STEP_COST, SINGLE_ITEM};
use crate::rules::geometry::{distance, neighbors};
use crate::world::WorldView;

#[derive(Clone, Copy, Debug)]
pub struct PathResult {
    pub next: Pos,
    pub goal: Pos,
    pub steps: i32,
}

type Queue = BinaryHeap<Reverse<(i32, i32, i32, i32, usize, Pos)>>;
type Parents = BTreeMap<Pos, (i32, Pos)>;

pub fn next_step(
    view: &WorldView<'_>,
    actor: i64,
    start: Pos,
    goals: &[Pos],
) -> Option<PathResult> {
    if goals.contains(&start) {
        return Some(PathResult {
            next: start,
            goal: start,
            steps: crate::rules::constants::ZERO_ROUNDS,
        });
    }
    let mut queue = Queue::new();
    let mut parents = Parents::new();
    let mut closed = BTreeSet::new();
    let mut sequence = crate::rules::constants::EMPTY_COUNT;
    parents.insert(start, (crate::rules::constants::ZERO_SCORE, start));
    push_node(
        &mut queue,
        start,
        crate::rules::constants::ZERO_SCORE,
        goals,
        sequence,
    );
    while let Some(Reverse((_, _, _, _, _, pos))) = queue.pop() {
        if !closed.insert(pos) || closed.len() > MAX_PATH_EXPANSIONS {
            continue;
        }
        if goals.contains(&pos) {
            return reconstruct(start, pos, &parents);
        }
        expand(
            view,
            actor,
            pos,
            goals,
            &mut queue,
            &mut parents,
            &mut sequence,
        );
    }
    None
}

fn expand(
    view: &WorldView<'_>,
    actor: i64,
    pos: Pos,
    goals: &[Pos],
    queue: &mut Queue,
    parents: &mut Parents,
    sequence: &mut usize,
) {
    let Some((cost, _)) = parents.get(&pos).copied() else {
        return;
    };
    for neighbor in neighbors(pos).filter(|cell| view.can_enter(*cell, actor)) {
        let Some(candidate) = cost.checked_add(PATH_STEP_COST) else {
            continue;
        };
        let old = parents
            .get(&neighbor)
            .map(|entry| entry.0)
            .unwrap_or(i32::MAX);
        if candidate < old {
            parents.insert(neighbor, (candidate, pos));
            *sequence = sequence.saturating_add(SINGLE_ITEM);
            push_node(queue, neighbor, candidate, goals, *sequence);
        }
    }
}

fn push_node(queue: &mut Queue, pos: Pos, cost: i32, goals: &[Pos], sequence: usize) {
    let estimate = goals
        .iter()
        .map(|goal| distance(pos, *goal))
        .min()
        .unwrap_or(i32::MAX);
    let heuristic = estimate.saturating_mul(PATH_STEP_COST);
    queue.push(Reverse((
        cost.saturating_add(heuristic),
        heuristic,
        pos.y,
        pos.x,
        sequence,
        pos,
    )));
}

fn reconstruct(start: Pos, target: Pos, parents: &Parents) -> Option<PathResult> {
    let mut node = target;
    let mut steps = crate::rules::constants::ZERO_ROUNDS;
    loop {
        let (_, parent) = parents.get(&node)?;
        steps = steps.saturating_add(crate::rules::constants::MIN_ACTION_ROUNDS);
        if *parent == start {
            return Some(PathResult {
                next: node,
                goal: target,
                steps,
            });
        }
        node = *parent;
    }
}

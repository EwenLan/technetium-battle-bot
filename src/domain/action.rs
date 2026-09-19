use std::collections::BTreeMap;

use serde::Serialize;

use super::{OwnerPath, Pos};

#[derive(Clone, Debug)]
pub enum Action {
    Move(Pos),
    Collect(Pos),
    Sell { name: String, quantity: i32 },
    Buy { name: String, quantity: i32 },
    Build { name: String, pos: Pos },
    Remove(Pos),
    Attack { controller: i64, targets: Vec<Pos> },
    AcceptTask,
    SubmitAnswer(String),
    SummonTreasure { pos: Pos, items: Vec<String> },
    Use { name: String, pos: Option<Pos> },
    Drop(String),
}

#[derive(Clone, Debug)]
pub struct ActionProposal {
    actor: i64,
    owner: OwnerPath,
    action: Action,
}

impl ActionProposal {
    pub const fn new(actor: i64, owner: OwnerPath, action: Action) -> Self {
        Self {
            actor,
            owner,
            action,
        }
    }

    pub const fn actor(&self) -> i64 {
        self.actor
    }

    pub const fn owner(&self) -> &OwnerPath {
        &self.owner
    }

    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub fn reporter(&self) -> i64 {
        match &self.action {
            Action::Attack { controller, .. } => *controller,
            _ => self.actor,
        }
    }

    pub fn into_command(self) -> Command {
        Command::from(self.action)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Command {
    action: &'static str,
    #[serde(rename = "controllerId", skip_serializing_if = "Option::is_none")]
    controller_id: Option<String>,
    #[serde(rename = "targetPos", skip_serializing_if = "Option::is_none")]
    target_pos: Option<Vec<Pos>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "num", skip_serializing_if = "Option::is_none")]
    quantity: Option<i32>,
    #[serde(rename = "taskAnswer", skip_serializing_if = "Option::is_none")]
    task_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<Vec<String>>,
}

impl Command {
    fn blank(action: &'static str) -> Self {
        Self {
            action,
            controller_id: None,
            target_pos: None,
            name: None,
            quantity: None,
            task_answer: None,
            item: None,
        }
    }

    fn target(action: &'static str, pos: Pos) -> Self {
        let mut command = Self::blank(action);
        command.target_pos = Some(vec![pos]);
        command
    }
}

impl From<Action> for Command {
    fn from(action: Action) -> Self {
        match action {
            Action::Move(_) | Action::Collect(_) | Action::Remove(_) | Action::Build { .. } => {
                location_command(action)
            }
            Action::Sell { .. } | Action::Buy { .. } | Action::Use { .. } | Action::Drop(_) => {
                inventory_command(action)
            }
            _ => special_command(action),
        }
    }
}

fn location_command(action: Action) -> Command {
    match action {
        Action::Move(pos) => Command::target("move", pos),
        Action::Collect(pos) => Command::target("collect", pos),
        Action::Remove(pos) => Command::target("remove", pos),
        Action::Build { name, pos } => command_with_name(Command::target("build", pos), name),
        _ => unreachable!("location action dispatch"),
    }
}

fn inventory_command(action: Action) -> Command {
    match action {
        Action::Sell { name, quantity } => trade("sell", name, quantity),
        Action::Buy { name, quantity } => trade("buy", name, quantity),
        Action::Use { name, pos } => use_item(name, pos),
        Action::Drop(name) => command_with_name(Command::blank("drop"), name),
        _ => unreachable!("inventory action dispatch"),
    }
}

fn special_command(action: Action) -> Command {
    match action {
        Action::Attack {
            controller,
            targets,
        } => attack(controller, targets),
        Action::AcceptTask => Command::blank("acceptTask"),
        Action::SubmitAnswer(answer) => answer_command(answer),
        Action::SummonTreasure { pos, items } => treasure(pos, items),
        _ => unreachable!("special action dispatch"),
    }
}

fn command_with_name(mut command: Command, name: String) -> Command {
    command.name = Some(name);
    command
}

fn trade(action: &'static str, name: String, quantity: i32) -> Command {
    let mut command = command_with_name(Command::blank(action), name);
    command.quantity = Some(quantity);
    command
}

fn attack(controller: i64, targets: Vec<Pos>) -> Command {
    let mut command = Command::blank("attack");
    command.controller_id = Some(controller.to_string());
    command.target_pos = Some(targets);
    command
}

fn answer_command(answer: String) -> Command {
    let mut command = Command::blank("submitAnswer");
    command.task_answer = Some(answer);
    command
}

fn treasure(pos: Pos, items: Vec<String>) -> Command {
    let mut command = Command::target("summonTreasure", pos);
    command.item = Some(items);
    command
}

fn use_item(name: String, pos: Option<Pos>) -> Command {
    let mut command = command_with_name(Command::blank("use"), name);
    command.target_pos = pos.map(|target| vec![target]);
    command
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Response {
    #[serde(rename = "roleCommandMap")]
    pub role_command_map: BTreeMap<String, Command>,
    pub prompt: String,
    #[serde(rename = "executeCmd")]
    pub execute_cmd: String,
}

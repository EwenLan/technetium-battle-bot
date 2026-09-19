use technetium_battle_bot::domain::{Action, ActionProposal, ActiveOwners, OwnerAllocator};

pub fn owned_action(
    owners: &mut ActiveOwners,
    allocator: &mut OwnerAllocator,
    actor: i64,
    action: Action,
) -> ActionProposal {
    let owner = owners.create_path(allocator).expect("owner path");
    ActionProposal::new(actor, owner, action)
}

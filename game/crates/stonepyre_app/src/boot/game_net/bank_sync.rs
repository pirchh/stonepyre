use bevy::prelude::*;

use stonepyre_ui::bank::{BankItemActionQueue, BankItemData, BankTabData, BankUiState};

use super::runtime::{
    send_bank_deposit_all_to_server,
    send_bank_deposit_to_server,
    send_bank_withdraw_to_server,
};
use super::status::{GameNetRuntime, GameNetStatus};

/// Syncs the `bank_open` flag and `bank_tabs` from `GameNetStatus` into `BankUiState`.
/// Runs after `pump_game_net_results` so the status is fresh before the UI reads it.
pub fn sync_bank_from_server(
    mut status: ResMut<GameNetStatus>,
    mut bank_ui: ResMut<BankUiState>,
) {
    // Always mirror tab data when dirty.
    if status.bank_dirty {
        bank_ui.tabs = status
            .bank_tabs
            .iter()
            .map(|t| BankTabData {
                tab_idx: t.tab_idx,
                display_name: t.display_name.clone(),
                tag_filters: t.tag_filters.clone(),
                items: t
                    .items
                    .iter()
                    .map(|i| BankItemData {
                        slot_idx: i.slot_idx,
                        item_id: i.item_id.clone(),
                        quantity: i.quantity,
                    })
                    .collect(),
            })
            .collect();

        bank_ui.needs_rebuild = true;
        status.bank_dirty = false;
    }

    // Propagate open/close from network status to UI state.
    if status.bank_open && !bank_ui.open {
        bank_ui.open = true;
        bank_ui.needs_rebuild = true;
    }

    // If the server says the bank is closed (e.g. after BankClose we set status.bank_open = false
    // in send_bank_item_actions_to_server), reflect that.
    if !status.bank_open && bank_ui.open {
        bank_ui.open = false;
        bank_ui.needs_rebuild = true;
    }
}

/// Drains `BankItemActionQueue` and forwards each action to the server.
pub fn send_bank_item_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut status: ResMut<GameNetStatus>,
    mut queue: ResMut<BankItemActionQueue>,
) {
    use stonepyre_ui::bank::BankItemAction;

    let actions: Vec<_> = queue.actions.drain(..).collect();

    for action in actions {
        match action {
            BankItemAction::Withdraw { tab_idx, slot_idx, item_id, quantity } => {
                let sent = send_bank_withdraw_to_server(&game_net, tab_idx, slot_idx, item_id.clone(), quantity);
                if !sent {
                    warn!(
                        "bank withdraw dropped; websocket not ready tab={} slot={} item={} qty={}",
                        tab_idx, slot_idx, item_id, quantity
                    );
                }
            }
            BankItemAction::DepositInvSlot { inv_slot_idx, item_id, quantity } => {
                let sent = send_bank_deposit_to_server(&game_net, inv_slot_idx, item_id.clone(), quantity);
                if !sent {
                    warn!(
                        "bank deposit dropped; websocket not ready inv_slot={} item={}",
                        inv_slot_idx, item_id
                    );
                }
            }
            BankItemAction::DepositAll => {
                let sent = send_bank_deposit_all_to_server(&game_net);
                if !sent {
                    warn!("bank deposit all dropped; websocket not ready");
                }
            }
            BankItemAction::Close => {
                // Close is purely client-side: flip the status flag so the panel can be
                // re-opened by walking to a booth again.
                status.bank_open = false;
            }
        }
    }
}

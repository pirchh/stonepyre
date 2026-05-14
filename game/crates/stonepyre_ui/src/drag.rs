// crates/stonepyre_ui/src/drag.rs
//
// OSRS-style click-vs-drag inventory interaction.
//
// Press + release quickly (< DRAG_THRESHOLD px movement) = primary action:
//   • bag item in inventory  → equip to the appropriate bag slot
//   • item in a bag panel    → take to first available inventory slot
//   • anything else          → "Use X ->" (select for use-on)
//
// Press + move cursor = drag:
//   • inv  → inv   : swap the two slots (SwapInvSlots)
//   • inv  → bag   : put item into bag (BagPutItem)
//   • bag  → inv   : take item to first available slot (BagTakeItem)
//   • bag  → bag   : move item to other bag (BagMoveItem)

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::inventory::{Inventory, PlayerBagSlots};

use crate::bag::{bag_slot_idx_at_cursor, BagItemAction, BagItemActionQueue, BagUiState};
use crate::inventory::{
    inv_slot_idx_at_cursor, inventory_item_for_slot, InventoryItemAction,
    InventoryItemActionQueue, InventoryUiState,
};

const DRAG_THRESHOLD: f32 = 8.0;
const GHOST_SIZE: f32 = 52.0;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum DragSource {
    Inventory { slot_idx: usize },
    Bag { bag_slot: u8, slot_idx: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DragTarget {
    Inventory { slot_idx: usize },
    Bag { bag_slot: u8, slot_idx: usize },
}

pub struct ActiveDrag {
    pub source: DragSource,
    pub ghost_entity: Option<Entity>,
    pub press_start_pos: Vec2,
    pub is_dragging: bool,
    pub item_id: String,
    pub icon_path: Option<String>,
}

#[derive(Resource, Default)]
pub struct DragState {
    pub active: Option<ActiveDrag>,
    /// The slot currently under the cursor while dragging (for visual highlight).
    pub hovered_drop_target: Option<DragTarget>,
}

#[derive(Component)]
pub(crate) struct DragGhost;

// ── Systems ───────────────────────────────────────────────────────────────────

/// Detect a mouse-down on an inventory or bag slot and record the press.
pub(crate) fn drag_begin_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inv_state: Res<InventoryUiState>,
    bag_state: Res<BagUiState>,
    bag_slots: Res<PlayerBagSlots>,
    inv_q: Query<&Inventory>,
    mut drag_state: ResMut<DragState>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if drag_state.active.is_some() {
        return;
    }

    let Ok(win) = windows.single() else { return; };
    let Some(cursor) = win.cursor_position() else { return; };

    // Check main inventory first.
    if inv_state.open {
        if let Some(slot_idx) = inv_slot_idx_at_cursor(&windows) {
            let Ok(inv) = inv_q.single() else { return; };
            if let Some(stk) = inv.container.slots.get(slot_idx).and_then(|s| s.as_ref()) {
                let icon_path = default_item_defs()
                    .get(stk.id.as_str())
                    .and_then(|d| d.inventory_icon.clone());
                drag_state.active = Some(ActiveDrag {
                    source: DragSource::Inventory { slot_idx },
                    ghost_entity: None,
                    press_start_pos: cursor,
                    is_dragging: false,
                    item_id: stk.id.clone(),
                    icon_path,
                });
                return;
            }
        }
    }

    // Check bag panels.
    if bag_state.open.iter().any(|&o| o) {
        if let Some((bag_slot, slot_idx)) = bag_slot_idx_at_cursor(&windows, &bag_state, &bag_slots) {
            let item_data = bag_slots
                .slots
                .iter()
                .find(|s| s.bag_slot == bag_slot)
                .and_then(|s| s.items.iter().find(|i| i.slot_idx == slot_idx))
                .cloned();

            if let Some(item) = item_data {
                let icon_path = default_item_defs()
                    .get(item.item_id.as_str())
                    .and_then(|d| d.inventory_icon.clone());
                drag_state.active = Some(ActiveDrag {
                    source: DragSource::Bag { bag_slot, slot_idx },
                    ghost_entity: None,
                    press_start_pos: cursor,
                    is_dragging: false,
                    item_id: item.item_id.clone(),
                    icon_path,
                });
            }
        }
    }
}

/// Each frame while dragging: spawn/move ghost, update hovered drop target.
pub(crate) fn drag_update_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inv_state: Res<InventoryUiState>,
    bag_state: Res<BagUiState>,
    bag_slots: Res<PlayerBagSlots>,
    mut drag_state: ResMut<DragState>,
    mut ghost_q: Query<&mut Node, With<DragGhost>>,
) {
    if drag_state.active.is_none() {
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        // drag_end_system handles the release — don't act here.
        return;
    }

    let Ok(win) = windows.single() else { return; };
    let Some(cursor) = win.cursor_position() else { return; };

    // Transition to drag mode once the cursor has moved enough.
    let should_start = {
        let drag = drag_state.active.as_ref().unwrap();
        !drag.is_dragging && (cursor - drag.press_start_pos).length() > DRAG_THRESHOLD
    };

    if should_start {
        let drag = drag_state.active.as_mut().unwrap();
        drag.is_dragging = true;

        let icon_handle: Handle<Image> = drag
            .icon_path
            .as_ref()
            .map(|p| asset_server.load(p))
            .unwrap_or_default();

        let ghost = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(cursor.x - GHOST_SIZE * 0.5),
                    top: Val::Px(cursor.y - GHOST_SIZE * 0.5),
                    width: Val::Px(GHOST_SIZE),
                    height: Val::Px(GHOST_SIZE),
                    ..default()
                },
                ImageNode {
                    image: icon_handle,
                    color: Color::srgba(1.0, 1.0, 1.0, 0.72),
                    ..default()
                },
                GlobalZIndex(100),
                Pickable::IGNORE,
                DragGhost,
                Name::new("drag_ghost"),
            ))
            .id();

        drag.ghost_entity = Some(ghost);
    }

    let is_dragging = drag_state.active.as_ref().map(|d| d.is_dragging).unwrap_or(false);
    if !is_dragging {
        return;
    }

    // Move the ghost to follow the cursor.
    let ghost_e = drag_state.active.as_ref().and_then(|d| d.ghost_entity);
    if let Some(ghost_e) = ghost_e {
        if let Ok(mut node) = ghost_q.get_mut(ghost_e) {
            node.left = Val::Px(cursor.x - GHOST_SIZE * 0.5);
            node.top = Val::Px(cursor.y - GHOST_SIZE * 0.5);
        }
    }

    // Update the highlighted drop target.
    drag_state.hovered_drop_target =
        find_drop_target(&windows, &inv_state, &bag_state, &bag_slots);
}

/// On mouse release: resolve as a click (primary action) or a drop.
pub(crate) fn drag_end_system(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inv_state: Res<InventoryUiState>,
    bag_state: Res<BagUiState>,
    bag_slots: Res<PlayerBagSlots>,
    inv_q: Query<&Inventory>,
    mut inv_ui_state: ResMut<InventoryUiState>,
    mut inv_action_queue: ResMut<InventoryItemActionQueue>,
    mut bag_action_queue: ResMut<BagItemActionQueue>,
    mut drag_state: ResMut<DragState>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    let Some(drag) = drag_state.active.take() else { return; };
    drag_state.hovered_drop_target = None;

    // Always clean up the ghost entity.
    if let Some(ghost_e) = drag.ghost_entity {
        if let Ok(mut ec) = commands.get_entity(ghost_e) {
            ec.despawn();
        }
    }

    if drag.is_dragging {
        // ── Drag resolution ───────────────────────────────────────────────
        let Some(target) = find_drop_target(&windows, &inv_state, &bag_state, &bag_slots) else {
            return; // dropped on empty space — do nothing
        };

        // Ignore drops onto the same slot.
        let is_self = match (&drag.source, &target) {
            (
                DragSource::Inventory { slot_idx: s },
                DragTarget::Inventory { slot_idx: t },
            ) => s == t,
            (
                DragSource::Bag { bag_slot: bs, slot_idx: si },
                DragTarget::Bag { bag_slot: bt, slot_idx: ti },
            ) => bs == bt && si == ti,
            _ => false,
        };
        if is_self {
            return;
        }

        match (&drag.source, &target) {
            // inv → inv : swap slots
            (
                DragSource::Inventory { slot_idx: from },
                DragTarget::Inventory { slot_idx: to },
            ) => {
                inv_action_queue.actions.push(crate::inventory::InventoryItemActionRequest {
                    action: InventoryItemAction::MoveToSlot { to_slot: *to },
                    slot_idx: *from,
                    item_id: drag.item_id.clone(),
                    quantity: 1,
                });
            }
            // inv → bag : put into bag
            (
                DragSource::Inventory { slot_idx: from },
                DragTarget::Bag { bag_slot, .. },
            ) => {
                bag_action_queue.actions.push(BagItemAction::PutItem {
                    bag_slot: *bag_slot,
                    inventory_slot_idx: *from,
                });
            }
            // bag → inv : take to first available slot
            (
                DragSource::Bag { bag_slot, slot_idx: from },
                DragTarget::Inventory { .. },
            ) => {
                bag_action_queue.actions.push(BagItemAction::Take {
                    bag_slot: *bag_slot,
                    bag_item_slot_idx: *from,
                });
            }
            // bag → bag (different bags only)
            (
                DragSource::Bag { bag_slot: from_bs, slot_idx: from_si },
                DragTarget::Bag { bag_slot: to_bs, .. },
            ) if from_bs != to_bs => {
                bag_action_queue.actions.push(BagItemAction::MoveItem {
                    from_bag_slot: *from_bs,
                    from_item_slot: *from_si,
                    to_bag_slot: *to_bs,
                });
            }
            _ => {} // same-bag reorder not yet supported
        }
    } else {
        // ── Click (primary action) ─────────────────────────────────────────
        match &drag.source {
            DragSource::Inventory { slot_idx } => {
                let defs = default_item_defs();
                let tags: &[String] = defs
                    .get(drag.item_id.as_str())
                    .map(|d| d.tags.as_slice())
                    .unwrap_or_default();

                if tags.iter().any(|t| t == "bag_general") {
                    inv_action_queue.actions.push(crate::inventory::InventoryItemActionRequest {
                        action: InventoryItemAction::EquipBag { bag_slot: 0 },
                        slot_idx: *slot_idx,
                        item_id: drag.item_id.clone(),
                        quantity: 1,
                    });
                } else if tags.iter().any(|t| t == "bag_typed") {
                    inv_action_queue.actions.push(crate::inventory::InventoryItemActionRequest {
                        action: InventoryItemAction::EquipBag { bag_slot: 1 },
                        slot_idx: *slot_idx,
                        item_id: drag.item_id.clone(),
                        quantity: 1,
                    });
                } else {
                    // Generic item: "Use X ->" (select for use-on-another).
                    let display_name = defs
                        .get(drag.item_id.as_str())
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| drag.item_id.clone());

                    if let Ok(inv) = inv_q.single() {
                        if let Some(item) = inventory_item_for_slot(inv, *slot_idx) {
                            inv_ui_state.selected_use_item = Some(item);
                            inv_ui_state.status_message = format!("Use {} ->", display_name);
                        }
                    }
                }
            }
            DragSource::Bag { bag_slot, slot_idx } => {
                // Left-click a bag item: take it to first available inventory slot.
                bag_action_queue.actions.push(BagItemAction::Take {
                    bag_slot: *bag_slot,
                    bag_item_slot_idx: *slot_idx,
                });
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_drop_target(
    windows: &Query<&Window, With<PrimaryWindow>>,
    inv_state: &InventoryUiState,
    bag_state: &BagUiState,
    bag_slots: &PlayerBagSlots,
) -> Option<DragTarget> {
    if inv_state.open {
        if let Some(slot_idx) = inv_slot_idx_at_cursor(windows) {
            return Some(DragTarget::Inventory { slot_idx });
        }
    }
    if bag_state.open.iter().any(|&o| o) {
        if let Some((bag_slot, slot_idx)) = bag_slot_idx_at_cursor(windows, bag_state, bag_slots) {
            return Some(DragTarget::Bag { bag_slot, slot_idx });
        }
    }
    None
}

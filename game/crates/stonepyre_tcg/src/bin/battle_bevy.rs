//! Stonepyre TCG — native Bevy battle tester with Hearthstone-inspired horizontal layout.
//!
//! Run from the game/ workspace directory:
//!   cargo run -p stonepyre_tcg --bin battle_bevy --features stonepyre_tcg/bevy_battle
use bevy::asset::AssetPlugin;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::{WindowPlugin, WindowResolution};
use rand::seq::SliceRandom;
use std::path::PathBuf;
use uuid::Uuid;

use stonepyre_tcg::{
    CardColor, CardType, DeckDefinition,
    engine::{GameEngine, GameEvent},
    files::load_registry_from_dir,
    match_state::{PlayerId, UnitInPlay},
    CardRegistry,
};

// ─── Players ─────────────────────────────────────────────────────────────────
const HUMAN: PlayerId = 0;
const AI: PlayerId = 1;

// ─── Card geometry — HAND cards (compact preview in tray) ────────────────────
// Hover lerps Node size up to ~board size for the Hearthstone "pop" effect.
// Art occupies ~40% of card height (Hearthstone-like proportion).
const CARD_W: f32 = 156.0;
const CARD_H: f32 = 220.0;
const CARD_BORDER: f32 = 3.0;
const CARD_GAP: f32 = 8.0;
const ART_H: f32 = 94.0;
const MANA_GEM_SIZE: f32 = 36.0;

// ─── Card geometry — BOARD cards (full size, detailed display) ────────────────
// Art ≈ 50% of card height for hero presence.
const BOARD_CARD_W: f32 = 200.0;
const BOARD_CARD_H: f32 = 280.0;
const BOARD_ART_H: f32 = 144.0;

// ─── Layout zones — calibrated for 1920×1080 window ──────────────────────────
// Total vertical: 1080 = 36header + (board flex_grow) + 244hand + 50mana
// Board flex_grow gets ~750px; content = 92+296+2+296+92 = 778, ~28px tight via FlexEnd
const HEADER_H: f32 = 36.0;
const PENDING_BANNER_H: f32 = 44.0;
const HERO_PORTRAIT_W: f32 = 96.0;
const HERO_PORTRAIT_H: f32 = 80.0;
const HERO_HP_BADGE: f32 = 30.0;
const HERO_ROW_H: f32 = 92.0;         // portrait(80) + badge_overlap(12)
const ENEMY_BOARD_H: f32 = 296.0;     // board card(280) + 8px padding each side
const YOUR_BOARD_H: f32 = 296.0;
const SIDE_PANEL_W: f32 = 180.0;
const HAND_ZONE_H: f32 = 244.0;       // compact card(220) + 24px breathing room
const MANA_BAR_H: f32 = 50.0;

// ─── Colors (Hearthstone-inspired) ───────────────────────────────────────────
const APP_BG:        Color = Color::srgb(0.04, 0.04, 0.07);
const HEADER_BG:     Color = Color::srgb(0.06, 0.06, 0.09);

// Hero zones (RED)
const HERO_BG:       Color = Color::srgb(0.12, 0.04, 0.04);
const HERO_BORDER:   Color = Color::srgb(0.91, 0.30, 0.24); // Bright red
const HERO_BORDER_W: f32 = 3.0;

// Battleground zone (PURPLE)
const BATTLEFIELD_BG: Color = Color::srgb(0.06, 0.03, 0.10);
const BATTLEFIELD_BORDER: Color = Color::srgb(0.80, 0.20, 1.00); // Bright purple
const BATTLEFIELD_BORDER_W: f32 = 4.0;

// Mana zone (YELLOW)
const MANA_ZONE_BG: Color = Color::srgb(0.10, 0.09, 0.03);
const MANA_ZONE_BORDER: Color = Color::srgb(1.00, 0.85, 0.20); // Bright yellow
const MANA_ZONE_BORDER_W: f32 = 2.0;

// End Turn zone (BLUE)
const ENDTURN_BG:    Color = Color::srgb(0.04, 0.06, 0.12);
const ENDTURN_BORDER: Color = Color::srgb(0.20, 0.70, 1.00); // Bright blue
const ENDTURN_BORDER_W: f32 = 2.0;

const ENEMY_BG:      Color = Color::srgb(0.10, 0.04, 0.04);
const YOURS_BG:      Color = Color::srgb(0.04, 0.04, 0.10);
const HAND_BG:       Color = Color::srgb(0.05, 0.05, 0.08);
const FOOTER_BG:     Color = Color::srgb(0.05, 0.05, 0.07);
const CARD_BG:       Color = Color::srgb(0.05, 0.05, 0.10);
const DIVIDER_COL:   Color = Color::srgb(0.15, 0.15, 0.20);
const BANNER_BG:     Color = Color::srgb(0.18, 0.05, 0.30);
const BANNER_BORDER: Color = Color::srgb(0.55, 0.20, 0.80);

// Drag feedback colors
const DRAG_VALID_BG:   Color = Color::srgba(0.15, 0.68, 0.38, 0.3); // Green tint
const DRAG_INVALID_BG: Color = Color::srgba(0.91, 0.30, 0.24, 0.2); // Red tint

const MANA_BG:     Color = Color::srgb(0.10, 0.28, 0.55);
const MANA_BORDER: Color = Color::srgb(0.23, 0.50, 0.91);
const ATK_BG:      Color = Color::srgb(0.55, 0.12, 0.10);
const ATK_BORDER:  Color = Color::srgb(0.91, 0.22, 0.18);
const HP_BG:       Color = Color::srgb(0.10, 0.42, 0.20);
const HP_BORDER:   Color = Color::srgb(0.15, 0.68, 0.38);
const DUR_BG:      Color = Color::srgb(0.50, 0.30, 0.05);
const DUR_BORDER:  Color = Color::srgb(0.90, 0.60, 0.10);
const HPBAR_FILL:  Color = Color::srgb(0.15, 0.68, 0.38);
const HPBAR_BG:    Color = Color::srgb(0.10, 0.12, 0.12);
const PIP_ON:      Color = Color::srgb(0.10, 0.28, 0.55);
const PIP_OFF:     Color = Color::srgb(0.12, 0.12, 0.14);

const BTN_END_BG:    Color = Color::srgb(0.45, 0.10, 0.08);
const BTN_END_BDR:   Color = Color::srgb(0.85, 0.20, 0.16);
const BTN_POWER_BG:  Color = Color::srgb(0.40, 0.28, 0.00);
const BTN_POWER_BDR: Color = Color::srgb(0.85, 0.65, 0.10);
const BTN_CANCEL_BG: Color = Color::srgb(0.18, 0.18, 0.20);
const BTN_CANCEL_BDR:Color = Color::srgb(0.40, 0.40, 0.45);
const BTN_ATTACK_BG: Color = Color::srgb(0.45, 0.08, 0.08);
const BTN_ATTACK_BDR:Color = Color::srgb(0.85, 0.15, 0.15);
const BTN_PLAY_BG:   Color = Color::srgb(0.08, 0.35, 0.12);
const BTN_PLAY_BDR:  Color = Color::srgb(0.15, 0.68, 0.25);

const TXT_NAME:  Color = Color::srgb(0.96, 0.82, 0.38);
const TXT_RULES: Color = Color::srgb(0.80, 0.80, 0.85);
const TXT_FAINT: Color = Color::srgb(0.50, 0.50, 0.55);
const TXT_LOG:   Color = Color::srgb(0.70, 0.70, 0.80);
const TXT_WHITE: Color = Color::WHITE;
const TXT_GOLD:  Color = Color::srgb(1.00, 0.85, 0.20);
const TXT_GREEN: Color = Color::srgb(0.30, 0.90, 0.50);
const GOLD_GLOW: Color = Color::srgb(0.95, 0.78, 0.10); // can-attack highlight

// ─── Drag-and-drop state ─────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq)]
enum DropZoneType {
    EnemyUnit(usize),
    FriendlyUnit(usize),
    EnemyHero,
    FriendlyHero,
    EmptyBoard,  // Can play units here
    NoZone,      // Dragging over invalid area
}

#[derive(Clone, Debug)]
struct DragPayload {
    hand_idx: usize,
    card_id: String,
    card_def: stonepyre_tcg::CardDefinition,
}

#[derive(Resource, Debug)]
struct DragState {
    dragging: Option<DragPayload>,
    ghost_entity: Option<Entity>,
    hover_zone: DropZoneType,
    is_valid: bool,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            dragging: None,
            ghost_entity: None,
            hover_zone: DropZoneType::NoZone,
            is_valid: false,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
struct MousePos(Vec2);

/// Set by `handle_drag_end` when a unit was successfully played from hand to
/// board; consumed by `rebuild_board` so the freshly-spawned unit card gets
/// tagged with `SlamAnimation` on its next build.
#[derive(Resource, Default)]
struct JustPlayed(Option<Uuid>);

/// Set by attack handlers (both human-driven and AI-driven) when a unit just
/// attacked and is still alive; consumed by `rebuild_board` to tag the attacker
/// with `LungeAnimation`. `is_ai` flips the lunge direction (DOWN instead of UP).
#[derive(Resource, Default, Clone, Copy)]
struct PendingLunge {
    attacker_id: Option<Uuid>,
    is_ai:       bool,
}

/// Set by attack handlers when a unit was struck (and survived). Consumed by
/// `rebuild_board` so the defender card gets tagged with `HitReactionAnimation`.
#[derive(Resource, Default)]
struct PendingHitFlash {
    defender_id: Option<Uuid>,
}

/// Drives the AI's turn one action per "step". `tick_ai_turn` reads this each
/// frame: if active and the cooldown has elapsed, it picks the next AI action,
/// executes it on the engine, sets the relevant animation triggers, and starts
/// a new cooldown so the animation has time to play out before the next move.
#[derive(Resource, Default)]
struct AiTurnState {
    active:   bool,
    cooldown: f32,
}

// ─── Pending action ──────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Debug, Default)]
enum Pending {
    #[default]
    None,
    PlayCard { hand_idx: usize },
    AttackWith { board_idx: usize },
    ChampionPower,
    Dragging { hand_idx: usize }, // New: card being dragged
}

// ─── Game resource ───────────────────────────────────────────────────────────
#[derive(Resource)]
struct TcgGame {
    engine: GameEngine,
    log: Vec<String>,
    pending: Pending,
    needs_rebuild: bool,
    font: Handle<Font>,
    deck_names: (String, String),
    available_decks: Vec<DeckDefinition>,
}

// ─── Marker components ───────────────────────────────────────────────────────
#[derive(Component)] struct BattleUiRoot;
/// Persistent transparent root for ephemeral overlays (attack arrow dots).
/// Lives OUTSIDE `BattleUiRoot` so `rebuild_board`'s recursive despawn doesn't
/// race with overlay-owning systems trying to despawn the same children.
#[derive(Component)] struct UiOverlayRoot;
#[derive(Component)] struct HandCardMarker(usize);
#[derive(Component)] struct YourUnitMarker(usize);
#[derive(Component)] struct EnemyUnitMarker(usize);
#[derive(Component)] struct EndTurnBtn;
#[derive(Component)] struct PowerBtn;
#[derive(Component)] struct AttackHeroBtn;
#[derive(Component)] struct CancelBtn;

// Drag / zone markers
#[derive(Component)] struct BattlegroundZone;
#[derive(Component)] struct HandZone;
#[derive(Component)] struct EnemyHeroZone;
#[derive(Component)] struct FriendlyHeroZone;
#[derive(Component)] struct HandCardHover; // marks hand cards for hover-scale system
#[derive(Component)] struct DragGhost;     // the ghost card that follows the cursor

/// Plays a brief "slam-down" tween (oversize → settle) on a freshly-summoned
/// board unit. Lives until `elapsed >= duration`, then the system removes it.
#[derive(Component)]
struct SlamAnimation {
    elapsed: f32,
    base_w:  f32,
    base_h:  f32,
}

/// Brief forward-lunge tween for an attacker — sine bump on margin.top so the
/// card pops toward its target and settles back. Removed when finished.
/// `is_ai` flips the direction: AI attackers lunge DOWN (toward you), human
/// attackers lunge UP (toward the enemy).
#[derive(Component)]
struct LungeAnimation {
    elapsed: f32,
    is_ai:   bool,
}

/// Brief defender hit reaction — flashes the card's border red and dips its
/// scale slightly, as if compressed by the impact. Used when a unit takes
/// damage from an enemy attack (and survives).
#[derive(Component)]
struct HitReactionAnimation {
    elapsed:         f32,
    /// Captured on the first frame so we can lerp back at the end.
    captured_border: Option<Color>,
    base_w:          f32,
    base_h:          f32,
}

/// Marker for the enemy hero portrait so we can route clicks at it like a unit.
#[derive(Component)] struct EnemyHeroTarget;

/// Marker for the dotted overlay segments that visualize the attack-target arrow.
#[derive(Component)] struct AttackArrowDot;

/// Per-card fan position, computed once at UI build time.
/// `animate_hand_cards` lerps back toward `rest_my` (not 0) when the card is unhovered.
#[derive(Component, Clone, Copy)]
struct HandFanData {
    rest_my:      f32, // resting margin.top — positive sinks the card into the tray
    rest_mx_left: f32, // initial margin.left — negative overlaps onto previous card
}

/// Smooth hover animation target for hand cards.
/// A lerp system moves Transform toward these targets each frame.
#[derive(Component)]
struct HandCardScale {
    target_scale: f32,
    target_y:     f32,
}

// ─── Color helpers ───────────────────────────────────────────────────────────
fn border_for_color(c: &CardColor) -> Color {
    match c {
        CardColor::Red     => Color::srgb(0.91, 0.30, 0.24),
        CardColor::Green   => Color::srgb(0.15, 0.68, 0.38),
        CardColor::Black   => Color::srgb(0.56, 0.27, 0.68),
        CardColor::White   => Color::srgb(0.93, 0.94, 0.95),
        CardColor::Blue    => Color::srgb(0.16, 0.50, 0.73),
        CardColor::Purple  => Color::srgb(0.61, 0.35, 0.71),
        CardColor::Neutral => Color::srgb(0.58, 0.65, 0.65),
    }
}

fn art_for_color(c: &CardColor) -> Color {
    match c {
        CardColor::Red     => Color::srgb(0.28, 0.05, 0.05),
        CardColor::Green   => Color::srgb(0.05, 0.18, 0.07),
        CardColor::Black   => Color::srgb(0.06, 0.00, 0.12),
        CardColor::White   => Color::srgb(0.22, 0.15, 0.00),
        CardColor::Blue    => Color::srgb(0.00, 0.09, 0.20),
        CardColor::Purple  => Color::srgb(0.10, 0.00, 0.18),
        CardColor::Neutral => Color::srgb(0.10, 0.13, 0.18),
    }
}

fn type_str(t: &CardType) -> &'static str {
    match t { CardType::Unit => "Unit", CardType::Spell => "Spell", CardType::Relic => "Relic", CardType::Champion => "Champion" }
}

fn trunc(s: &str, n: usize) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i >= n { out.push_str(".."); break; }
        out.push(c);
    }
    out
}

// ─── Stat display ─────────────────────────────────────────────────────────────
/// A stat value (atk/hp/durability) plus the color treatment that goes with it.
/// Hearthstone shows base stats as plain white numbers and only tints them green
/// when buffed above base or red when damaged below base — that's the rule here.
#[derive(Clone)]
struct StatDisplay {
    val:    String,
    bg:     Color,
    border: Color,
    text:   Color,
}

impl StatDisplay {
    fn neutral(val: impl Into<String>) -> Self {
        Self {
            val:    val.into(),
            bg:     Color::srgba(0.0, 0.0, 0.0, 0.55),  // dark, no hue
            border: Color::srgb(0.40, 0.42, 0.50),      // subtle grey rim
            text:   TXT_WHITE,
        }
    }
    fn buffed(val: impl Into<String>) -> Self {
        Self {
            val:    val.into(),
            bg:     Color::srgb(0.04, 0.18, 0.07),
            border: Color::srgb(0.30, 0.90, 0.50),
            text:   Color::srgb(0.55, 1.00, 0.65),
        }
    }
    fn damaged(val: impl Into<String>) -> Self {
        Self {
            val:    val.into(),
            bg:     Color::srgb(0.18, 0.04, 0.04),
            border: Color::srgb(0.91, 0.30, 0.24),
            text:   Color::srgb(1.00, 0.50, 0.45),
        }
    }
    fn durability(val: impl Into<String>) -> Self {
        Self {
            val:    val.into(),
            bg:     DUR_BG,
            border: DUR_BORDER,
            text:   TXT_WHITE,
        }
    }
}

// ─── Spawn helpers ───────────────────────────────────────────────────────────

fn stat_circle(
    commands: &mut Commands,
    font: &Handle<Font>,
    val: &str,
    bg: Color,
    border: Color,
    text: Color,
    size: f32,
) -> Entity {
    let root = commands.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Percent(50.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border),
        Pickable::IGNORE,
    )).id();
    let txt = commands.spawn((
        Text::new(val.to_string()),
        TextFont { font: font.clone(), font_size: (size * 0.48).max(11.0), ..default() },
        TextColor(text),
        Pickable::IGNORE,
    )).id();
    commands.entity(root).add_child(txt);
    root
}

fn label(commands: &mut Commands, font: &Handle<Font>, text: &str, size: f32, color: Color) -> Entity {
    commands.spawn((
        Text::new(text.to_string()),
        TextFont { font: font.clone(), font_size: size, ..default() },
        TextColor(color),
        Pickable::IGNORE,
    )).id()
}

fn btn(commands: &mut Commands, font: &Handle<Font>, text: &str, bg: Color, border: Color, w: f32, h: f32) -> Entity {
    let root = commands.spawn((
        Node {
            width: Val::Px(w),
            height: Val::Px(h),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border),
        Interaction::default(),
    )).id();
    let t = label(commands, font, text, 11.0, TXT_WHITE);
    commands.entity(root).add_child(t);
    root
}

/// Spawn a card node.
/// - `card_w/card_h/art_h`: dimensions (hand cards vs compact board cards)
/// - `show_cost`: true for hand cards (renders mana gem); false for board units
///
/// The mana gem is intentionally added as the LAST child so it renders on top
/// of the art area in Bevy's child-order z-stack.
fn spawn_card(
    commands: &mut Commands,
    font: &Handle<Font>,
    cost: u8,
    name: &str,
    type_label: &str,
    rules: &str,
    art_color: Color,
    border_color: Color,
    atk: Option<StatDisplay>,
    hp_or_dur: Option<StatDisplay>,
    card_w: f32,
    card_h: f32,
    art_h: f32,
    show_cost: bool,
) -> Entity {
    let root = commands.spawn((
        Node {
            width: Val::Px(card_w),
            height: Val::Px(card_h),
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Relative,
            border: UiRect::all(Val::Px(CARD_BORDER)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            // Clip anything that overflows — long rules text won't bleed past stats.
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(CARD_BG),
        BorderColor::all(border_color),
        Interaction::default(),
        // Explicit Transform so the hover animation system has something to mutate.
        // Bevy 0.18 does NOT auto-add Transform to UI Node entities.
        Transform::default(),
        Name::new("card"),
    )).id();

    // ── Art area — fixed-size, NEVER shrinks ──────────────────────────────────
    // `flex_shrink: 0.0` is critical — without it, long rules text below will
    // squish the art (Bevy flex default is shrink=1.0). This is how Hearthstone
    // keeps the art rectangle constant regardless of card content.
    let art_top = if show_cost { 11.0 } else { 6.0 };
    let art = commands.spawn((
        Node {
            height: Val::Px(art_h),
            flex_shrink: 0.0,
            margin: UiRect { top: Val::Px(art_top), left: Val::Px(6.0), right: Val::Px(6.0), bottom: Val::Px(5.0) },
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(art_color),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.22)),
        Pickable::IGNORE,
        Name::new("art"),
    )).id();
    commands.entity(root).add_child(art);

    // ── Name banner — gold text, slightly bigger and more prominent ──────────
    let name_size = if show_cost { 16.5 } else { 15.0 };
    let name_max  = if show_cost { 18 } else { 16 };
    let name_e = commands.spawn((
        Text::new(trunc(name, name_max)),
        TextFont { font: font.clone(), font_size: name_size, ..default() },
        TextColor(TXT_NAME),
        Node {
            margin: UiRect { top: Val::Px(2.0), left: Val::Px(6.0), right: Val::Px(6.0), bottom: Val::Px(1.0), ..default() },
            ..default()
        },
        Pickable::IGNORE,
    )).id();
    commands.entity(root).add_child(name_e);

    // ── Rules text — board cards show full rules; hand cards keep it short ──
    // (We dropped the redundant "Unit"/"Spell" type line — readers infer it from layout.)
    // Suppress the type_label warning while keeping the API signature stable.
    let _ = type_label;
    let rules_size = if show_cost { 11.0 } else { 10.5 };
    let rules_max  = if show_cost { 80 } else { 60 };
    let rules_e = commands.spawn((
        Text::new(trunc(rules, rules_max)),
        TextFont { font: font.clone(), font_size: rules_size, ..default() },
        TextColor(TXT_RULES),
        Node {
            margin: UiRect { top: Val::Px(2.0), left: Val::Px(6.0), right: Val::Px(6.0), ..default() },
            flex_shrink: 1.0,
            ..default()
        },
        Pickable::IGNORE,
    )).id();
    commands.entity(root).add_child(rules_e);

    // ── Stats row — pushed to bottom via margin-top:auto ─────────────────────
    let stats = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect { left: Val::Px(6.0), right: Val::Px(6.0), bottom: Val::Px(5.0), top: Val::Px(3.0) },
            margin: UiRect { top: Val::Auto, ..default() },
            ..default()
        },
        Pickable::IGNORE,
    )).id();

    // Stat circles scale with card size: full board cards get 42px, compact hand cards get 32px
    let stat_size = if card_h >= 250.0 { 42.0_f32 } else { 32.0_f32 };

    if let Some(a) = atk {
        let c = stat_circle(commands, font, &a.val, a.bg, a.border, a.text, stat_size);
        commands.entity(stats).add_child(c);
    } else {
        let sp = commands.spawn((Node { width: Val::Px(stat_size), ..default() }, Pickable::IGNORE)).id();
        commands.entity(stats).add_child(sp);
    }

    if let Some(h) = hp_or_dur {
        let c = stat_circle(commands, font, &h.val, h.bg, h.border, h.text, stat_size);
        commands.entity(stats).add_child(c);
    }

    commands.entity(root).add_child(stats);

    // ── Mana cost gem — added LAST so it renders on top of the art ───────────
    // In Bevy UI, later children z-stack above earlier ones.
    // Only hand cards show the cost; board units have already paid it.
    if show_cost {
        let gem = commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                top: Val::Px(4.0),
                width: Val::Px(MANA_GEM_SIZE),
                height: Val::Px(MANA_GEM_SIZE),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.5)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.40, 0.75)),
            BorderColor::all(Color::srgb(0.40, 0.70, 1.00)),
            Pickable::IGNORE,
        )).id();
        let gem_font = (MANA_GEM_SIZE * 0.52).max(9.0);
        let gem_txt = label(commands, font, &cost.to_string(), gem_font, TXT_WHITE);
        commands.entity(gem).add_child(gem_txt);
        commands.entity(root).add_child(gem);
    }

    root
}

/// Spawn a card from a CardDefinition.
/// `is_board`: true = compact board dimensions (no cost gem); false = full hand-card size.
fn card_from_def(
    commands: &mut Commands,
    font: &Handle<Font>,
    def: &stonepyre_tcg::CardDefinition,
    border_override: Option<Color>,
    is_board: bool,
) -> Entity {
    let border = border_override.unwrap_or_else(|| border_for_color(&def.color));
    let art = art_for_color(&def.color);
    // Card definitions show base stats — always neutral (white text).
    let (atk, hp_dur) = match def.card_type {
        CardType::Unit | CardType::Champion => (
            def.attack.map(|v| StatDisplay::neutral(v.to_string())),
            def.health.map(|v| StatDisplay::neutral(v.to_string())),
        ),
        CardType::Relic => (None, def.durability.map(|v| StatDisplay::durability(v.to_string()))),
        CardType::Spell => (None, None),
    };
    let (cw, ch, ah, show_cost) = if is_board {
        (BOARD_CARD_W, BOARD_CARD_H, BOARD_ART_H, false)
    } else {
        (CARD_W, CARD_H, ART_H, true)
    };
    spawn_card(commands, font, def.cost, &def.name, type_str(&def.card_type),
        &def.rules_text, art, border, atk, hp_dur, cw, ch, ah, show_cost)
}

/// Spawn a card representing an in-play unit (shows current stats).
fn card_from_unit(
    commands: &mut Commands,
    font: &Handle<Font>,
    unit: &UnitInPlay,
    registry: &CardRegistry,
    can_act: bool,
    is_targeting: bool,
) -> Entity {
    let def = registry.get(&unit.card_def_id);
    let color = def.map(|d| &d.color).unwrap_or(&CardColor::Neutral);
    let cost = def.map(|d| d.cost).unwrap_or(0);
    let rules = def.map(|d| d.rules_text.as_str()).unwrap_or("");

    let border = if can_act {
        GOLD_GLOW
    } else if is_targeting {
        Color::srgb(0.70, 0.25, 0.85)
    } else if unit.is_shielded {
        Color::srgb(0.93, 0.94, 0.95)
    } else {
        border_for_color(color)
    };

    let kws: Vec<&str> = unit.keywords.iter().map(|k| k.display_name()).collect();
    let display_rules = if kws.is_empty() {
        rules.to_string()
    } else {
        format!("{} | {}", rules, kws.join(", "))
    };

    // Color treatment depends on current vs base: Hearthstone-style — only highlight
    // stats when they've changed from the card's printed value.
    let base_atk = def.and_then(|d| d.attack).unwrap_or(unit.current_attack);
    let base_hp  = def.and_then(|d| d.health).unwrap_or(unit.current_health);

    let atk_display = if unit.current_attack > base_atk {
        StatDisplay::buffed(unit.current_attack.to_string())
    } else if unit.current_attack < base_atk {
        StatDisplay::damaged(unit.current_attack.to_string())
    } else {
        StatDisplay::neutral(unit.current_attack.to_string())
    };

    let hp_display = if unit.current_health > base_hp {
        StatDisplay::buffed(unit.current_health.to_string())
    } else if unit.current_health < base_hp {
        StatDisplay::damaged(unit.current_health.to_string())
    } else {
        StatDisplay::neutral(unit.current_health.to_string())
    };

    spawn_card(commands, font, cost, &unit.display_name, "Unit", &display_rules,
        art_for_color(color), border,
        Some(atk_display), Some(hp_display),
        BOARD_CARD_W, BOARD_CARD_H, BOARD_ART_H, false)
}

// ─── Layout builders ─────────────────────────────────────────────────────────


// ─── Full board layout ────────────────────────────────────────────────────────

/// Arch-shaped hero portrait frame — placeholder for a character PNG.
/// Health shown as a red badge overlaid at the bottom of the frame.
/// All children are `Pickable::IGNORE` so the wrapper can receive Interaction
/// events cleanly (used for "click enemy hero to attack").
fn hero_portrait(commands: &mut Commands, font: &Handle<Font>, hp: i32, is_player: bool) -> Entity {
    let wrapper = commands.spawn((
        Node {
            width: Val::Px(HERO_PORTRAIT_W),
            height: Val::Px(HERO_PORTRAIT_H + HERO_HP_BADGE * 0.5),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            ..default()
        },
    )).id();

    let frame = commands.spawn((
        Node {
            width: Val::Px(HERO_PORTRAIT_W),
            height: Val::Px(HERO_PORTRAIT_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius {
                top_left: Val::Px(40.0),
                top_right: Val::Px(40.0),
                bottom_left: Val::Px(6.0),
                bottom_right: Val::Px(6.0),
            },
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.11, 0.07)),
        BorderColor::all(Color::srgb(0.82, 0.62, 0.22)),
        Pickable::IGNORE,
    )).id();

    let lbl = commands.spawn((
        Text::new(if is_player { "YOU" } else { "AI" }),
        TextFont { font: font.clone(), font_size: 10.0, ..default() },
        TextColor(TXT_FAINT),
        Pickable::IGNORE,
    )).id();
    commands.entity(frame).add_child(lbl);
    commands.entity(wrapper).add_child(frame);

    let badge = commands.spawn((
        Node {
            width: Val::Px(HERO_HP_BADGE),
            height: Val::Px(HERO_HP_BADGE),
            border: UiRect::all(Val::Px(2.5)),
            border_radius: BorderRadius::all(Val::Percent(50.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            margin: UiRect { top: Val::Px(-HERO_HP_BADGE * 0.5), ..default() },
            ..default()
        },
        BackgroundColor(Color::srgb(0.55, 0.06, 0.06)),
        BorderColor::all(Color::srgb(0.85, 0.22, 0.22)),
        Pickable::IGNORE,
    )).id();
    let hp_txt = commands.spawn((
        Text::new(hp.to_string()),
        TextFont { font: font.clone(), font_size: 14.0, ..default() },
        TextColor(TXT_WHITE),
        Pickable::IGNORE,
    )).id();
    commands.entity(badge).add_child(hp_txt);
    commands.entity(wrapper).add_child(badge);

    wrapper
}

/// Horizontal row of mana gems (Hearthstone crystal bar).
fn mana_gem_row(commands: &mut Commands, font: &Handle<Font>, mana: u8, max_mana: u8) -> Entity {
    let row = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
    )).id();

    // Mana number label
    let lbl = label(commands, font, &format!("{}/{}", mana, max_mana), 12.0, TXT_GOLD);
    commands.entity(row).add_child(lbl);

    // Gem circles
    for i in 0..max_mana.max(1) {
        let filled = i < mana;
        let gem = commands.spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(if filled { Color::srgb(0.18, 0.48, 0.90) } else { Color::srgb(0.07, 0.09, 0.18) }),
            BorderColor::all(if filled { Color::srgb(0.50, 0.78, 1.00) } else { Color::srgb(0.20, 0.22, 0.35) }),
        )).id();
        commands.entity(row).add_child(gem);
    }
    row
}

fn build_battle_ui(
    commands: &mut Commands,
    game: &TcgGame,
    just_played: Option<Uuid>,
    pending_lunge: PendingLunge,
    pending_hit_defender: Option<Uuid>,
) {
    let font = &game.font;
    let engine = &game.engine;
    let p0 = &engine.state.players[HUMAN as usize];
    let p1 = &engine.state.players[AI as usize];
    let pending = &game.pending;
    let is_your_turn = engine.state.active_player == HUMAN && !engine.is_over();
    let registry = &engine.registry;

    // Root — overflow clip so hand cards peek below without scrollbar
    let root = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(APP_BG),
        BattleUiRoot,
    )).id();

    // ── Turn bar ─────────────────────────────────────────────────────────────
    {
        let ts = if engine.is_over() {
            format!("Game Over — Player {} wins!", engine.winner().map(|w| w+1).unwrap_or(0))
        } else if is_your_turn { format!("Your Turn  ·  Turn {}", engine.state.turn)
        } else { format!("AI Turn  ·  Turn {}", engine.state.turn) };
        let tc = if engine.is_over() { TXT_GOLD } else if is_your_turn { TXT_GREEN } else { TXT_FAINT };
        let bar = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(HEADER_H), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, BackgroundColor(Color::srgb(0.04,0.04,0.07)))).id();
        let t = label(commands, font, &ts, 11.0, tc);
        commands.entity(bar).add_child(t);
        commands.entity(root).add_child(bar);
    }

    // ── Pending banner ────────────────────────────────────────────────────────
    if *pending != Pending::None {
        let bt = match pending {
            Pending::PlayCard { hand_idx } => { let cid = p0.hand.get(*hand_idx).cloned().unwrap_or_default(); let n = registry.get(&cid).map(|d| d.name.as_str()).unwrap_or("?"); format!("Select target for {n}") }
            Pending::AttackWith { board_idx } => { let n = p0.board.get(*board_idx).map(|u| u.display_name.as_str()).unwrap_or("?"); format!("Attack with {n}  ·  click enemy or Attack Hero") }
            Pending::ChampionPower => "Champion Power  ·  select a target".into(),
            Pending::Dragging { .. } => "Drag to a valid target...".into(),
            Pending::None => String::new(),
        };
        let banner = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(PENDING_BANNER_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)), column_gap: Val::Px(12.0), ..default() }, BackgroundColor(BANNER_BG))).id();
        let t = label(commands, font, &bt, 10.0, Color::srgb(0.88,0.70,1.0));
        commands.entity(banner).add_child(t);
        let cancel = btn(commands, font, "✕ Cancel", BTN_CANCEL_BG, BTN_CANCEL_BDR, 80.0, 26.0);
        commands.entity(cancel).insert(CancelBtn);
        commands.entity(banner).add_child(cancel);
        commands.entity(root).add_child(banner);
    }

    // ── Board: [left pad = SIDE_PANEL_W] | [center] | [right = SIDE_PANEL_W with EndTurn] ──
    // Equal side panels mean center board is perfectly centered on screen.
    let board_section = commands.spawn((
        Node { width: Val::Percent(100.0), flex_grow: 1.0, flex_direction: FlexDirection::Row, ..default() },
        BackgroundColor(Color::srgb(0.04, 0.03, 0.07)),
        BattlegroundZone,
    )).id();

    // Left spacer
    let lpad = commands.spawn(Node { width: Val::Px(SIDE_PANEL_W), flex_shrink: 0.0, ..default() }).id();
    commands.entity(board_section).add_child(lpad);

    // Center board column — FlexEnd pushes content to bottom so your hero/units are close to hand
    let center = commands.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, ..default() })).id();

    // Opponent hero — centered at top of board. Clickable as an attack target.
    {
        let hr = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(HERO_ROW_H), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, BackgroundColor(Color::srgb(0.07,0.04,0.04)))).id();
        let p = hero_portrait(commands, font, p1.health, false);
        // Wrapper receives Interaction; children all have Pickable::IGNORE.
        commands.entity(p).insert((Interaction::default(), EnemyHeroTarget));
        commands.entity(hr).add_child(p);
        commands.entity(center).add_child(hr);
    }

    // Enemy units
    {
        let row = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(ENEMY_BOARD_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(CARD_GAP), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect { bottom: Val::Px(1.0), ..default() }, ..default() }, BackgroundColor(ENEMY_BG), BorderColor::all(DIVIDER_COL))).id();
        if let Some(ch) = &p1.champion_in_play { if let Some(def) = registry.get(&ch.card_def_id) { let c = card_from_def(commands, font, def, None, true); commands.entity(row).add_child(c); } }
        for (i, unit) in p1.board.iter().enumerate() {
            let tgt = matches!(pending, Pending::AttackWith {..} | Pending::PlayCard {..} | Pending::ChampionPower);
            let c = card_from_unit(commands, font, unit, registry, false, tgt);

            // Slam-down on freshly-summoned AI units.
            if Some(unit.instance_id) == just_played {
                commands.entity(c).insert(SlamAnimation {
                    elapsed: 0.0,
                    base_w:  BOARD_CARD_W,
                    base_h:  BOARD_CARD_H,
                });
            }
            // Lunge on the AI's attacker (lunges DOWN toward you).
            if Some(unit.instance_id) == pending_lunge.attacker_id && pending_lunge.is_ai {
                commands.entity(c).insert(LungeAnimation { elapsed: 0.0, is_ai: true });
            }
            // Hit reaction if this enemy unit was struck by you.
            if Some(unit.instance_id) == pending_hit_defender {
                commands.entity(c).insert(HitReactionAnimation {
                    elapsed:         0.0,
                    captured_border: None,
                    base_w:          BOARD_CARD_W,
                    base_h:          BOARD_CARD_H,
                });
            }

            commands.entity(c).insert(EnemyUnitMarker(i));
            commands.entity(row).add_child(c);
        }
        // Note: no "Attack Hero" button — click the enemy hero portrait at the top instead.
        if p1.board.is_empty() && p1.champion_in_play.is_none() { let e = label(commands, font, "(empty)", 10.0, TXT_FAINT); commands.entity(row).add_child(e); }
        commands.entity(center).add_child(row);
    }

    // Center divider
    let div = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(2.0), ..default() }, BackgroundColor(DIVIDER_COL))).id();
    commands.entity(center).add_child(div);

    // Your units
    {
        let row = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(YOUR_BOARD_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(CARD_GAP), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect { bottom: Val::Px(1.0), ..default() }, ..default() }, BackgroundColor(YOURS_BG), BorderColor::all(DIVIDER_COL))).id();
        if let Some(ch) = &p0.champion_in_play { if let Some(def) = registry.get(&ch.card_def_id) { let c = card_from_def(commands, font, def, Some(GOLD_GLOW), true); commands.entity(row).add_child(c); } }
        for (i, unit) in p0.board.iter().enumerate() {
            let can_act   = is_your_turn && unit.can_attack();
            let highlight = can_act && matches!(pending, Pending::None);
            let c = card_from_unit(commands, font, unit, registry, highlight, false);

            // Slam-down on freshly-summoned units.
            if Some(unit.instance_id) == just_played {
                commands.entity(c).insert(SlamAnimation {
                    elapsed: 0.0,
                    base_w:  BOARD_CARD_W,
                    base_h:  BOARD_CARD_H,
                });
            }
            // Lunge on the human's attacker (lunges UP toward enemy).
            if Some(unit.instance_id) == pending_lunge.attacker_id && !pending_lunge.is_ai {
                commands.entity(c).insert(LungeAnimation { elapsed: 0.0, is_ai: false });
            }
            // Hit reaction if this unit was the defender of an attack (AI hit it).
            if Some(unit.instance_id) == pending_hit_defender {
                commands.entity(c).insert(HitReactionAnimation {
                    elapsed:         0.0,
                    captured_border: None,
                    base_w:          BOARD_CARD_W,
                    base_h:          BOARD_CARD_H,
                });
            }

            commands.entity(c).insert(YourUnitMarker(i));
            commands.entity(row).add_child(c);
        }
        if p0.board.is_empty() && p0.champion_in_play.is_none() { let e = label(commands, font, "(empty)", 10.0, TXT_FAINT); commands.entity(row).add_child(e); }
        commands.entity(center).add_child(row);
    }

    // Your hero — centered at bottom of board, power button to the right
    {
        let hr = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(HERO_ROW_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(16.0), ..default() }, BackgroundColor(Color::srgb(0.05,0.04,0.09)))).id();
        let p = hero_portrait(commands, font, p0.health, true);
        commands.entity(hr).add_child(p);
        // Champion power button — sits right of your hero like HS hero power
        if is_your_turn && matches!(pending, Pending::None) {
            if let Some(ch) = &p0.champion_in_play {
                if let Some(def) = registry.get(&ch.card_def_id) {
                    if let Some(power) = &def.champion_power {
                        if p0.mana >= power.cost && !p0.champion_power_used {
                            let pb = btn(commands, font, &format!("★\n{}🔷", power.cost), BTN_POWER_BG, BTN_POWER_BDR, 62.0, 62.0);
                            commands.entity(pb).insert(PowerBtn);
                            commands.entity(hr).add_child(pb);
                        }
                    }
                }
            }
        }
        commands.entity(center).add_child(hr);
    }

    commands.entity(board_section).add_child(center);

    // Right panel — End Turn vertically centered, same width as left pad
    {
        let panel = commands.spawn((Node { width: Val::Px(SIDE_PANEL_W), flex_shrink: 0.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: Val::Px(10.0), ..default() }, BackgroundColor(Color::srgb(0.04,0.03,0.07)))).id();
        if is_your_turn && matches!(pending, Pending::None) {
            let end = btn(commands, font, "END\nTURN", BTN_END_BG, BTN_END_BDR, 100.0, 60.0);
            commands.entity(end).insert(EndTurnBtn);
            commands.entity(panel).add_child(end);
        }
        if let Some(last) = game.log.last() {
            let lg = label(commands, font, &trunc(last, 16), 7.5, TXT_LOG);
            commands.entity(panel).add_child(lg);
        }
        commands.entity(board_section).add_child(panel);
    }

    commands.entity(root).add_child(board_section);

    // ── Mana bar — sits ABOVE the hand tray so cards never butt against it ───
    // (Hearthstone keeps the crystal bar outside the hand's playable zone.)
    {
        let mbar = commands.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(MANA_BAR_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect { right: Val::Px(SIDE_PANEL_W + 8.0), left: Val::Px(8.0), top: Val::Px(0.0), bottom: Val::Px(4.0) },
                border: UiRect { top: Val::Px(1.0), bottom: Val::Px(1.0), ..default() },
                ..default()
            },
            BackgroundColor(Color::srgb(0.03, 0.03, 0.06)),
            BorderColor::all(DIVIDER_COL),
        )).id();
        let gems = mana_gem_row(commands, font, p0.mana, p0.max_mana);
        commands.entity(mbar).add_child(gems);
        commands.entity(root).add_child(mbar);
    }

    // ── Hand — cards fan out in an arc, overlapping like Hearthstone ─────────
    {
        let hand_zone = commands.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(HAND_ZONE_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                justify_content: JustifyContent::Center,
                // No column_gap — overlap is controlled per-card via HandFanData.rest_mx_left
                padding: UiRect { top: Val::Px(8.0), left: Val::Px(SIDE_PANEL_W), right: Val::Px(SIDE_PANEL_W), bottom: Val::Px(0.0) },
                border: UiRect { top: Val::Px(1.0), ..default() },
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.07)),
            BorderColor::all(DIVIDER_COL),
            HandZone,
        )).id();

        let hand_len = p0.hand.len();
        // Center index of the fan (e.g. 2.0 for 5 cards).
        let center_idx = (hand_len.max(1) as f32 - 1.0) * 0.5;
        // Avoid division-by-zero for a single-card hand.
        let max_dist   = center_idx.max(1.0);
        // Overlap (left margin) scales with hand size:
        //   ≤3 cards → spaced; 4–5 → light overlap; 6+ → heavy tucking.
        let overlap = match hand_len {
            0..=3 => 10.0_f32,   // small positive gap
            4..=5 => -22.0,
            6     => -42.0,
            _     => -58.0,
        };

        for (i, cid) in p0.hand.iter().enumerate() {
            if let Some(def) = registry.get(cid) {
                let affordable = p0.mana >= def.cost;
                // Unaffordable cards get a greyed-out border
                let bov = if !affordable { Some(Color::srgb(0.28, 0.28, 0.30)) } else { None };
                let card = card_from_def(commands, font, def, bov, false);
                // ZIndex(0) at rest; animate_hand_cards bumps it to ZIndex(100) on hover
                // so the popped card draws above its siblings in the fan.
                commands.entity(card).insert((HandCardHover, ZIndex(0)));

                // Fan arch: edge cards sit slightly lower in the tray, center card sits
                // at the top — a gentle "smile" curve. Hearthstone uses ~10–14 px.
                let dist_from_center = (i as f32 - center_idx).abs();
                let t = dist_from_center / max_dist;     // 0 at center, 1 at edges
                let rest_my = (t * t) * 18.0;            // quadratic arc, max 18 px sag at edges
                let rest_mx_left = if i == 0 { 0.0 } else { overlap };
                commands.entity(card).insert(HandFanData { rest_my, rest_mx_left });

                // Draggable cards get the HandCardMarker — the card itself IS the button
                if is_your_turn && affordable && matches!(pending, Pending::None) {
                    commands.entity(card).insert(HandCardMarker(i));
                }
                commands.entity(hand_zone).add_child(card);
            }
        }
        if p0.hand.is_empty() { let e = label(commands, font, "(no cards)", 10.0, TXT_FAINT); commands.entity(hand_zone).add_child(e); }
        commands.entity(root).add_child(hand_zone);
    }
}

// ─── Systems ─────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let manifest = env!("CARGO_MANIFEST_DIR");
    let asset_root = format!("{}/../../assets", manifest);

    let cards_path = PathBuf::from(&asset_root).join("content/tcg/cards");
    let registry = match load_registry_from_dir(&cards_path) {
        Ok(r) => { info!("Loaded {} TCG cards.", r.len()); r }
        Err(e) => { error!("Failed to load cards: {}", e); CardRegistry::new() }
    };

    let decks_path = PathBuf::from(&asset_root).join("content/tcg/decks");
    let mut available_decks: Vec<DeckDefinition> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&decks_path) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(e.path()) {
                    if let Ok(d) = serde_json::from_str::<DeckDefinition>(&text) {
                        available_decks.push(d);
                    }
                }
            }
        }
    }
    if available_decks.is_empty() {
        warn!("No decks found in {:?}", decks_path);
        return;
    }

    let font: Handle<Font> = asset_server.load("fonts/ui.ttf");

    let mut rng = rand::rng();
    let d0 = available_decks[0].clone();
    let d1 = available_decks.last().unwrap().clone();
    let deck_names = (d0.name.clone(), d1.name.clone());
    let mut p1_ids = d0.card_ids.clone();
    let mut p2_ids = d1.card_ids.clone();
    p1_ids.shuffle(&mut rng);
    p2_ids.shuffle(&mut rng);

    let deck0 = DeckDefinition { id: d0.id.clone(), name: d0.name, card_ids: p1_ids };
    let deck1 = DeckDefinition { id: d1.id.clone(), name: d1.name, card_ids: p2_ids };

    let mut engine = GameEngine::new(registry, deck0, deck1);
    let mut log = Vec::new();
    let evs = engine.begin_game();
    log_events(&evs, &mut log);

    commands.insert_resource(TcgGame {
        engine,
        log,
        pending: Pending::None,
        needs_rebuild: true,
        font,
        deck_names,
        available_decks,
    });

    // Initialize drag-and-drop state and animation triggers
    commands.insert_resource(DragState::default());
    commands.insert_resource(MousePos::default());
    commands.insert_resource(JustPlayed::default());
    commands.insert_resource(PendingLunge::default());
    commands.insert_resource(PendingHitFlash::default());
    commands.insert_resource(AiTurnState::default());

    // Persistent overlay root for the attack arrow. Renders above the main UI
    // via GlobalZIndex and lives outside BattleUiRoot so rebuild_board never
    // touches its children.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left:   Val::Px(0.0),
            top:    Val::Px(0.0),
            width:  Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(1000),
        Pickable::IGNORE,
        UiOverlayRoot,
    ));
}

fn rebuild_board(
    mut commands:      Commands,
    mut game:          ResMut<TcgGame>,
    mut just_played:   ResMut<JustPlayed>,
    mut pending_lunge: ResMut<PendingLunge>,
    mut pending_hit:   ResMut<PendingHitFlash>,
    ui_roots:          Query<Entity, With<BattleUiRoot>>,
) {
    if !game.needs_rebuild { return; }

    for e in ui_roots.iter() {
        commands.entity(e).despawn();
    }

    let game_ref: &TcgGame = &*game;
    build_battle_ui(
        &mut commands,
        game_ref,
        just_played.0,
        *pending_lunge,
        pending_hit.defender_id,
    );

    // Consume one-shot animation triggers so they only fire once.
    just_played.0             = None;
    pending_lunge.attacker_id = None;
    pending_lunge.is_ai       = false;
    pending_hit.defender_id   = None;

    game.needs_rebuild = false;
}

fn handle_interactions(
    mut game:          ResMut<TcgGame>,
    mut pending_lunge: ResMut<PendingLunge>,
    mut pending_hit:   ResMut<PendingHitFlash>,
    mut ai_state:      ResMut<AiTurnState>,
    // Note: hand cards are played via drag-and-drop (handle_drag_end), not click.
    your_q:        Query<(&Interaction, &YourUnitMarker), Changed<Interaction>>,
    enemy_q:       Query<(&Interaction, &EnemyUnitMarker), Changed<Interaction>>,
    enemy_hero_q:  Query<&Interaction, (Changed<Interaction>, With<EnemyHeroTarget>)>,
    end_q:         Query<&Interaction, (Changed<Interaction>, With<EndTurnBtn>)>,
    power_q:       Query<&Interaction, (Changed<Interaction>, With<PowerBtn>)>,
    cancel_q:      Query<&Interaction, (Changed<Interaction>, With<CancelBtn>)>,
) {
    for (intr, marker) in your_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        on_your_unit_click(&mut game, marker.0);
    }

    for (intr, marker) in enemy_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        on_enemy_unit_click(&mut game, marker.0, &mut pending_lunge, &mut pending_hit);
    }

    for intr in enemy_hero_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        if matches!(game.pending, Pending::AttackWith { .. }) {
            on_attack_hero(&mut game, &mut pending_lunge);
        }
    }

    for intr in end_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        on_end_turn(&mut game, &mut ai_state);
    }

    // Champion power
    for intr in power_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        on_power_click(&mut game);
    }

    // Cancel (banner button — kept as a backup, click-empty also cancels)
    for intr in cancel_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        game.pending = Pending::None;
        game.needs_rebuild = true;
    }
}

/// Cancel any pending action (AttackWith / PlayCard / ChampionPower) when the
/// user clicks somewhere that isn't a valid UI target. Runs AFTER
/// `handle_interactions` so legitimate clicks resolve the pending first;
/// only "empty space" clicks reach this and trigger the cancel.
fn cancel_on_empty_click(
    mut game: ResMut<TcgGame>,
    buttons: Res<ButtonInput<MouseButton>>,
    drag_state: Res<DragState>,
    interactions: Query<&Interaction>,
) {
    if !buttons.just_pressed(MouseButton::Left) { return; }
    if drag_state.dragging.is_some()           { return; }
    if matches!(game.pending, Pending::None)   { return; }

    // If anything UI is currently in the Pressed state, the click hit a target
    // — don't cancel; let the proper handler do its job.
    if interactions.iter().any(|i| matches!(i, Interaction::Pressed)) { return; }

    game.pending = Pending::None;
    game.needs_rebuild = true;
}

// ─── Action handlers ─────────────────────────────────────────────────────────

fn on_hand_click(game: &mut TcgGame, hand_idx: usize) {
    if game.engine.is_over() || game.engine.state.active_player != HUMAN { return; }

    let cid = match game.engine.state.players[HUMAN as usize].hand.get(hand_idx).cloned() {
        Some(c) => c, None => return,
    };
    let def = match game.engine.registry.get(&cid).cloned() {
        Some(d) => d, None => return,
    };

    // Spells with targeting enter select mode
    if def.is_spell() {
        if def.spell_effect.as_ref().map(|fx| fx.targeting.requires_target()).unwrap_or(false) {
            game.pending = Pending::PlayCard { hand_idx };
            game.needs_rebuild = true;
            return;
        }
    }

    match game.engine.play_card(HUMAN, &cid, None) {
        Ok(evs) => log_events(&evs, &mut game.log),
        Err(e)  => game.log.push(format!("⚠ {}", e)),
    }
    game.pending = Pending::None;
    game.needs_rebuild = true;
}

fn on_your_unit_click(game: &mut TcgGame, board_idx: usize) {
    if game.engine.is_over() { return; }

    match game.pending.clone() {
        Pending::PlayCard { hand_idx } => {
            let friendly_id = game.engine.state.players[HUMAN as usize].board
                .get(board_idx).map(|u| u.instance_id);
            if let Some(tid) = friendly_id {
                let cid = game.engine.state.players[HUMAN as usize].hand.get(hand_idx).cloned();
                if let Some(c) = cid {
                    match game.engine.play_card(HUMAN, &c, Some(tid)) {
                        Ok(evs) => log_events(&evs, &mut game.log),
                        Err(e)  => game.log.push(format!("⚠ {}", e)),
                    }
                }
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
        Pending::ChampionPower => {
            let friendly_id = game.engine.state.players[HUMAN as usize].board
                .get(board_idx).map(|u| u.instance_id);
            match game.engine.use_champion_power(HUMAN, friendly_id) {
                Ok(evs) => log_events(&evs, &mut game.log),
                Err(e)  => game.log.push(format!("⚠ {}", e)),
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
        Pending::None => {
            // Select this unit for attack
            if game.engine.state.active_player == HUMAN {
                let can = game.engine.state.players[HUMAN as usize].board
                    .get(board_idx).map(|u| u.can_attack()).unwrap_or(false);
                if can {
                    game.pending = Pending::AttackWith { board_idx };
                    game.needs_rebuild = true;
                }
            }
        }
        Pending::AttackWith { board_idx: current } => {
            // Clicking ANOTHER friendly switches the attacker (if it can attack).
            // Clicking the same one keeps it selected.
            if board_idx != current {
                let can = game.engine.state.players[HUMAN as usize].board
                    .get(board_idx).map(|u| u.can_attack()).unwrap_or(false);
                if can {
                    game.pending = Pending::AttackWith { board_idx };
                    game.needs_rebuild = true;
                }
            }
        }
        _ => {}
    }
}

fn on_enemy_unit_click(
    game: &mut TcgGame,
    enemy_idx: usize,
    pending_lunge: &mut PendingLunge,
    pending_hit: &mut PendingHitFlash,
) {
    if game.engine.is_over() { return; }

    let target_id = game.engine.state.players[AI as usize].board
        .get(enemy_idx).map(|u| u.instance_id);
    let Some(tid) = target_id else { return; };

    match game.pending.clone() {
        Pending::AttackWith { board_idx } => {
            let atk_id = game.engine.state.players[HUMAN as usize].board
                .get(board_idx).map(|u| u.instance_id);
            if let Some(aid) = atk_id {
                match game.engine.attack_unit(HUMAN, aid, tid) {
                    Ok(evs) => {
                        log_events(&evs, &mut game.log);
                        // Tag attacker for lunge only if it survived the trade.
                        let p0 = &game.engine.state.players[HUMAN as usize];
                        if p0.board.iter().any(|u| u.instance_id == aid) {
                            pending_lunge.attacker_id = Some(aid);
                            pending_lunge.is_ai       = false;
                        }
                        // Tag the enemy defender for the hit-reaction flash (only
                        // if it survived; dead units are despawned on rebuild).
                        let p1 = &game.engine.state.players[AI as usize];
                        if p1.board.iter().any(|u| u.instance_id == tid) {
                            pending_hit.defender_id = Some(tid);
                        }
                    }
                    Err(e) => game.log.push(format!("⚠ {}", e)),
                }
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
        Pending::PlayCard { hand_idx } => {
            let cid = game.engine.state.players[HUMAN as usize].hand.get(hand_idx).cloned();
            if let Some(c) = cid {
                match game.engine.play_card(HUMAN, &c, Some(tid)) {
                    Ok(evs) => log_events(&evs, &mut game.log),
                    Err(e)  => game.log.push(format!("⚠ {}", e)),
                }
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
        Pending::ChampionPower => {
            match game.engine.use_champion_power(HUMAN, Some(tid)) {
                Ok(evs) => log_events(&evs, &mut game.log),
                Err(e)  => game.log.push(format!("⚠ {}", e)),
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
        _ => {}
    }
}

fn on_end_turn(game: &mut TcgGame, ai_state: &mut AiTurnState) {
    if game.engine.is_over() { return; }
    match game.engine.end_turn(HUMAN) {
        Ok(evs) => log_events(&evs, &mut game.log),
        Err(e)  => game.log.push(format!("⚠ {}", e)),
    }
    game.pending = Pending::None;

    // Activate the AI driver — it'll take one action per ~0.5s in `tick_ai_turn`
    // so animations have time to play out between moves.
    if !game.engine.is_over() && game.engine.state.active_player == AI {
        ai_state.active   = true;
        ai_state.cooldown = 0.35;   // brief beat before AI starts acting
    }

    game.needs_rebuild = true;
}

fn on_power_click(game: &mut TcgGame) {
    if game.engine.is_over() { return; }

    // Check if power requires a target
    let needs_target = game.engine.state.players[HUMAN as usize].champion_in_play
        .as_ref()
        .and_then(|c| game.engine.registry.get(&c.card_def_id))
        .and_then(|d| d.champion_power.as_ref())
        .map(|p| p.targeting.requires_target())
        .unwrap_or(false);

    if needs_target {
        game.pending = Pending::ChampionPower;
        game.needs_rebuild = true;
    } else {
        match game.engine.use_champion_power(HUMAN, None) {
            Ok(evs) => log_events(&evs, &mut game.log),
            Err(e)  => game.log.push(format!("⚠ {}", e)),
        }
        game.pending = Pending::None;
        game.needs_rebuild = true;
    }
}

fn on_attack_hero(game: &mut TcgGame, pending_lunge: &mut PendingLunge) {
    if game.engine.is_over() { return; }

    let board_idx = match &game.pending {
        Pending::AttackWith { board_idx } => *board_idx,
        _ => return,
    };

    let atk_id = game.engine.state.players[HUMAN as usize].board
        .get(board_idx).map(|u| u.instance_id);

    if let Some(aid) = atk_id {
        match game.engine.attack_hero(HUMAN, aid) {
            Ok(evs) => {
                log_events(&evs, &mut game.log);
                let p0 = &game.engine.state.players[HUMAN as usize];
                if p0.board.iter().any(|u| u.instance_id == aid) {
                    pending_lunge.attacker_id = Some(aid);
                    pending_lunge.is_ai       = false;
                }
            }
            Err(e) => game.log.push(format!("⚠ {}", e)),
        }
    }
    game.pending = Pending::None;
    game.needs_rebuild = true;
}

// ─── AI turn (incremental state machine) ─────────────────────────────────────
//
// The AI used to run its entire turn synchronously inside `on_end_turn`, which
// meant the player never saw the intermediate states — by the time the UI
// rebuilt, every attack and play had already resolved. Now the AI takes ONE
// action per "step" via `tick_ai_turn`, with a cooldown between steps so
// animations have time to play out.

enum AiAction {
    PlayCard          { card_id: String, target: Option<Uuid> },
    UseChampionPower  { target: Option<Uuid> },
    AttackUnit        { attacker_id: Uuid, defender_id: Uuid },
    AttackHero        { attacker_id: Uuid },
}

/// Pure decision function — picks the AI's next action based on current state.
/// Returns `None` when there's nothing left to do (caller should end turn).
/// Priority order matches the old `run_ai`: play cheapest card → champion
/// power → attack (guard > weakest > hero).
fn decide_ai_action(engine: &GameEngine, registry: &CardRegistry) -> Option<AiAction> {
    let state    = &engine.state;
    let ai       = AI    as usize;
    let opponent = HUMAN as usize;

    // 1. Play cheapest affordable card.
    let mana = state.players[ai].mana;
    let mut playable: Vec<(usize, u8)> = state.players[ai].hand.iter().enumerate()
        .filter_map(|(i, cid)| registry.get(cid).filter(|d| d.cost <= mana).map(|d| (i, d.cost)))
        .collect();
    playable.sort_by_key(|&(_, c)| c);

    if let Some((idx, _)) = playable.first().copied() {
        let card_id = state.players[ai].hand[idx].clone();
        if let Some(def) = registry.get(&card_id).cloned() {
            let target = if def.is_spell() {
                def.spell_effect.as_ref().and_then(|fx| {
                    if !fx.targeting.requires_target() { return None; }
                    match fx.targeting {
                        stonepyre_tcg::TargetingRule::EnemyUnit | stonepyre_tcg::TargetingRule::AnyUnit =>
                            state.players[opponent].board.first().map(|u| u.instance_id),
                        stonepyre_tcg::TargetingRule::FriendlyUnit =>
                            state.players[ai].board.iter().min_by_key(|u| u.current_health).map(|u| u.instance_id),
                        _ => None,
                    }
                })
            } else { None };
            return Some(AiAction::PlayCard { card_id, target });
        }
    }

    // 2. Champion power if affordable & unused.
    if let Some(champ) = &state.players[ai].champion_in_play {
        if let Some(def) = registry.get(&champ.card_def_id) {
            if let Some(power) = &def.champion_power {
                if state.players[ai].mana >= power.cost && !state.players[ai].champion_power_used {
                    let target = if power.targeting.requires_target() {
                        match power.targeting {
                            stonepyre_tcg::TargetingRule::EnemyUnit | stonepyre_tcg::TargetingRule::AnyUnit =>
                                state.players[opponent].board.first().map(|u| u.instance_id),
                            stonepyre_tcg::TargetingRule::FriendlyUnit =>
                                state.players[ai].board.iter().min_by_key(|u| u.current_health).map(|u| u.instance_id),
                            _ => None,
                        }
                    } else { None };
                    return Some(AiAction::UseChampionPower { target });
                }
            }
        }
    }

    // 3. Attack with first ready unit. Target priority: guard, weakest, hero.
    let attacker = state.players[ai].board.iter()
        .find(|u| u.can_attack()).map(|u| u.instance_id);
    if let Some(atk_id) = attacker {
        let target = state.players[opponent].board.iter()
            .find(|u| u.has_guard()).map(|u| u.instance_id)
            .or_else(|| state.players[opponent].board.iter()
                .min_by_key(|u| u.current_health).map(|u| u.instance_id));
        return Some(match target {
            Some(tid) => AiAction::AttackUnit { attacker_id: atk_id, defender_id: tid },
            None      => AiAction::AttackHero { attacker_id: atk_id },
        });
    }

    None
}

/// System: while the AI's turn is active and the cooldown has elapsed, decide
/// and execute one action, then start a new cooldown for the next animation.
fn tick_ai_turn(
    mut game:         ResMut<TcgGame>,
    mut ai_state:     ResMut<AiTurnState>,
    mut pending_lunge:ResMut<PendingLunge>,
    mut pending_hit:  ResMut<PendingHitFlash>,
    mut just_played:  ResMut<JustPlayed>,
    time:             Res<Time>,
) {
    if !ai_state.active { return; }
    if game.engine.is_over() {
        ai_state.active = false;
        return;
    }
    if ai_state.cooldown > 0.0 {
        ai_state.cooldown -= time.delta_secs();
        return;
    }

    // Cooldowns are tuned to be a bit longer than each animation so the player
    // has a beat to read what just happened before the next action fires.
    const COOLDOWN_PLAY:   f32 = 0.55;
    const COOLDOWN_POWER:  f32 = 0.45;
    const COOLDOWN_ATTACK: f32 = 0.50;

    let registry = game.engine.registry.clone();
    let action   = decide_ai_action(&game.engine, &registry);

    match action {
        Some(AiAction::PlayCard { card_id, target }) => {
            let before_len = game.engine.state.players[AI as usize].board.len();
            match game.engine.play_card(AI, &card_id, target) {
                Ok(evs) => {
                    log_events(&evs, &mut game.log);
                    let p_ai = &game.engine.state.players[AI as usize];
                    if p_ai.board.len() > before_len {
                        if let Some(u) = p_ai.board.last() {
                            just_played.0 = Some(u.instance_id);
                        }
                    }
                }
                Err(_) => { ai_state.active = false; return; }
            }
            ai_state.cooldown = COOLDOWN_PLAY;
            game.needs_rebuild = true;
        }
        Some(AiAction::UseChampionPower { target }) => {
            if let Ok(evs) = game.engine.use_champion_power(AI, target) {
                log_events(&evs, &mut game.log);
            }
            ai_state.cooldown = COOLDOWN_POWER;
            game.needs_rebuild = true;
        }
        Some(AiAction::AttackUnit { attacker_id, defender_id }) => {
            match game.engine.attack_unit(AI, attacker_id, defender_id) {
                Ok(evs) => {
                    log_events(&evs, &mut game.log);
                    // Lunge on AI attacker if it survived (mutual KO leaves nothing).
                    if game.engine.state.players[AI as usize].board.iter()
                        .any(|u| u.instance_id == attacker_id) {
                        pending_lunge.attacker_id = Some(attacker_id);
                        pending_lunge.is_ai       = true;
                    }
                    // Hit reaction on HUMAN defender if it survived.
                    if game.engine.state.players[HUMAN as usize].board.iter()
                        .any(|u| u.instance_id == defender_id) {
                        pending_hit.defender_id = Some(defender_id);
                    }
                }
                Err(_) => { ai_state.active = false; return; }
            }
            ai_state.cooldown = COOLDOWN_ATTACK;
            game.needs_rebuild = true;
        }
        Some(AiAction::AttackHero { attacker_id }) => {
            if let Ok(evs) = game.engine.attack_hero(AI, attacker_id) {
                log_events(&evs, &mut game.log);
                if game.engine.state.players[AI as usize].board.iter()
                    .any(|u| u.instance_id == attacker_id) {
                    pending_lunge.attacker_id = Some(attacker_id);
                    pending_lunge.is_ai       = true;
                }
            }
            ai_state.cooldown = COOLDOWN_ATTACK;
            game.needs_rebuild = true;
        }
        None => {
            // No more AI actions — end the turn.
            if let Ok(evs) = game.engine.end_turn(AI) {
                log_events(&evs, &mut game.log);
            }
            ai_state.active   = false;
            ai_state.cooldown = 0.0;
            game.needs_rebuild = true;
        }
    }
}

fn log_events(events: &[GameEvent], log: &mut Vec<String>) {
    for ev in events {
        let line = match ev {
            GameEvent::TurnStarted { player, turn, mana } => format!("── Turn {} · P{} · {} mana ──", turn, player+1, mana),
            GameEvent::CardPlayed { player, card_name, .. } => format!("P{} plays {}", player+1, card_name),
            GameEvent::UnitSummoned { player, card_name, attack, health, .. } => format!("P{} → {} ({}/{})", player+1, card_name, attack, health),
            GameEvent::TokenSummoned { player, name, attack, health, .. } => format!("P{} token: {} ({}/{})", player+1, name, attack, health),
            GameEvent::SpellCast { player, card_name } => format!("P{} casts {}", player+1, card_name),
            GameEvent::RelicPlayed { player, card_name, durability } => format!("P{} relic: {} [{}]", player+1, card_name, durability),
            GameEvent::ChampionPlayed { player, card_name } => format!("P{} Champion: {}!", player+1, card_name),
            GameEvent::UnitAttackedUnit { attacker_name, defender_name, damage_dealt, damage_taken } =>
                format!("{} → {} (dealt {}, took {})", attacker_name, defender_name, damage_dealt, damage_taken),
            GameEvent::UnitAttackedHero { attacker_name, target_player, damage } =>
                format!("{} → P{} hero: -{}", attacker_name, target_player+1, damage),
            GameEvent::HeroDamaged { player, amount, remaining_health } =>
                format!("P{} -{} HP  ({})", player+1, amount, remaining_health),
            GameEvent::UnitDied { player, card_name } => format!("✝ {} (P{})", card_name, player+1),
            GameEvent::ShieldedBlocked { card_name } => format!("🛡 {}'s shield absorbed hit", card_name),
            GameEvent::UnitBuffed { card_name, attack_delta, health_delta } =>
                format!("{} +{}/+{}", card_name, attack_delta, health_delta),
            GameEvent::ChampionPowerUsed { player, champion_name, .. } =>
                format!("P{} power: {}", player+1, champion_name),
            GameEvent::GameOver { winner } => format!("🏆 PLAYER {} WINS!", winner+1),
            GameEvent::CardDrawn { player, card_name } => format!("P{} draws {}", player+1, card_name),
            _ => continue,
        };
        log.push(line);
        if log.len() > 40 { log.remove(0); }
    }
}

// ─── Hand card hover scale ───────────────────────────────────────────────────

/// Smooth Hearthstone-style hand card pop-out.
///
/// Bevy 0.18's picking system gives `Interaction::Hovered` only to the DEEPEST node
/// under the cursor — child nodes (art, text) consume it before the card root sees it.
/// So we bypass `Interaction` entirely and compare the cursor position directly against
/// each card's computed layout bounds via `ComputedNode` + `GlobalTransform`.
///
/// Bevy 0.18 UI `GlobalTransform` and `Window::cursor_position()` both use
/// screen-space: (0,0) = top-left, X+ = right, Y+ = down. No conversion needed.
/// Apply each hand card's fan position the first frame it appears.
/// Uses `Added<HandFanData>` so it runs only once per spawn — subsequent
/// frames are owned by `animate_hand_cards` (hover lerp).
fn init_hand_fan(mut q: Query<(&mut Node, &HandFanData), Added<HandFanData>>) {
    for (mut node, fan) in q.iter_mut() {
        node.margin.left = Val::Px(fan.rest_mx_left);
        node.margin.top  = Val::Px(fan.rest_my);
    }
}

/// Smooth Hearthstone-style hand card pop-out.
/// On hover, lerp the card from compact (CARD_W×CARD_H) up to full board size
/// (BOARD_CARD_W×BOARD_CARD_H) and lift it with a negative top margin.
///
/// Bevy 0.18 UI doesn't respect Transform translation for layout — position
/// lives in the layout system, not in GlobalTransform. So we mutate Node
/// directly. Picking is reliable because every child of the card has
/// `Pickable::IGNORE`, letting `Interaction::Hovered` land on the card root.
fn animate_hand_cards(
    mut q: Query<
        (&Interaction, &mut Node, Option<&HandFanData>, &mut ZIndex),
        With<HandCardHover>,
    >,
    time: Res<Time>,
) {
    let speed = 14.0_f32;
    let dt    = (speed * time.delta_secs()).min(1.0);

    let rest_w  = CARD_W;
    let rest_h  = CARD_H;
    let hov_w   = BOARD_CARD_W;
    let hov_h   = BOARD_CARD_H;
    let hov_my  = -100.0_f32; // lift so popped card clears the tray edge cleanly

    for (intr, mut node, fan, mut zindex) in q.iter_mut() {
        let hovered  = matches!(intr, Interaction::Hovered | Interaction::Pressed);
        // Resting Y comes from the fan layout; falls back to 0 for non-hand cards.
        let rest_my  = fan.map(|f| f.rest_my).unwrap_or(0.0);
        let (tw, th, tmy) = if hovered {
            (hov_w, hov_h, hov_my)
        } else {
            (rest_w, rest_h, rest_my)
        };

        // Float hovered card above its siblings in the hand fan.
        // Later children render over earlier ones by default, so without this
        // the right-side neighbor would draw over the bottom of the popped card.
        *zindex = if hovered { ZIndex(100) } else { ZIndex(0) };

        let cur_w  = if let Val::Px(v) = node.width      { v } else { rest_w  };
        let cur_h  = if let Val::Px(v) = node.height     { v } else { rest_h  };
        let cur_my = if let Val::Px(v) = node.margin.top { v } else { rest_my };

        node.width      = Val::Px(cur_w  + (tw  - cur_w ) * dt);
        node.height     = Val::Px(cur_h  + (th  - cur_h ) * dt);
        node.margin.top = Val::Px(cur_my + (tmy - cur_my) * dt);
    }
}

// ─── Attack arrow (dotted line from attacker → cursor) ───────────────────────

/// Each frame while `Pending::AttackWith` is active, despawn previous dots and
/// spawn fresh ones along the line between the attacker's center and the
/// cursor. Cheap to do this naively given how few dots there are.
fn update_attack_arrow(
    game: Res<TcgGame>,
    mouse_pos: Res<MousePos>,
    window_q: Query<&Window>,
    units_q: Query<(&UiGlobalTransform, &YourUnitMarker)>,
    dots_q: Query<Entity, With<AttackArrowDot>>,
    overlay_q: Query<Entity, With<UiOverlayRoot>>,
    mut commands: Commands,
) {
    if game.needs_rebuild { return; }

    for e in dots_q.iter() {
        commands.entity(e).despawn();
    }

    let Pending::AttackWith { board_idx } = game.pending else { return; };

    // `UiGlobalTransform.translation` is in PHYSICAL pixels — on Retina that's
    // 2× the logical screen size that `Window::cursor_position()` reports.
    // Divide by the window's scale_factor to put both in the same coord space.
    let scale = window_q.iter().next().map(|w| w.scale_factor() as f32).unwrap_or(1.0);

    let mut from = None;
    for (uigt, marker) in units_q.iter() {
        if marker.0 == board_idx {
            from = Some(uigt.translation / scale);
            break;
        }
    }
    let Some(from) = from else { return; };
    let to        = mouse_pos.0;
    let diff      = to - from;
    let length    = diff.length();
    if length < 40.0 { return; } // don't draw nubs

    let Some(overlay) = overlay_q.iter().next() else { return; };

    // Dots get larger toward the cursor (the "arrowhead" end).
    let n_dots = ((length / 30.0) as usize).clamp(3, 14);
    for i in 1..=n_dots {
        let t   = i as f32 / (n_dots as f32 + 1.0);
        let p   = from.lerp(to, t);
        let sz  = 6.0 + 8.0 * t;  // 6→14 px
        let dot = commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left:   Val::Px(p.x - sz * 0.5),
                top:    Val::Px(p.y - sz * 0.5),
                width:  Val::Px(sz),
                height: Val::Px(sz),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.30, 0.20, 0.90)),
            AttackArrowDot,
            Pickable::IGNORE,
        )).id();
        commands.entity(overlay).add_child(dot);
    }

    // Arrowhead halo at the cursor position.
    let head_sz = 28.0_f32;
    let head = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left:   Val::Px(to.x - head_sz * 0.5),
            top:    Val::Px(to.y - head_sz * 0.5),
            width:  Val::Px(head_sz),
            height: Val::Px(head_sz),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Percent(50.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.30, 0.20, 0.20)),
        BorderColor::all(Color::srgb(1.0, 0.45, 0.30)),
        AttackArrowDot,
        Pickable::IGNORE,
    )).id();
    commands.entity(overlay).add_child(head);
}

// ─── Lunge animation for the attacker ────────────────────────────────────────

/// Sine bump on margin.top: 0 → ±36 → 0 over ~0.20s. Direction depends on
/// the attacker's side (human lunges up toward enemy; AI lunges down toward you).
fn animate_lunge(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Node, &mut LungeAnimation)>,
    time: Res<Time>,
) {
    const DURATION: f32 = 0.20;
    const LUNGE_PX: f32 = 36.0;

    for (e, mut node, mut anim) in q.iter_mut() {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / DURATION).min(1.0);
        let phase = (std::f32::consts::PI * t).sin(); // 0 → 1 → 0
        // UI Y+ = down. AI lunges DOWN (positive); human lunges UP (negative).
        let dy = if anim.is_ai { phase * LUNGE_PX } else { -phase * LUNGE_PX };
        node.margin.top = Val::Px(dy);

        if anim.elapsed >= DURATION {
            node.margin.top = Val::Px(0.0);
            commands.entity(e).remove::<LungeAnimation>();
        }
    }
}

/// Flash the defender's border red and dip its scale to ~93% over ~0.30s.
/// The border color and base size are captured the first frame and restored
/// at the end so we don't permanently corrupt the card's resting state.
fn animate_hit_reaction(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Node, &mut BorderColor, &mut HitReactionAnimation)>,
    time: Res<Time>,
) {
    const DURATION:  f32 = 0.30;
    const SCALE_DIP: f32 = 0.93;
    let red = Color::srgb(1.0, 0.25, 0.20);

    for (e, mut node, mut bc, mut anim) in q.iter_mut() {
        // Capture the resting border color the very first frame so we can lerp
        // back to it at the end. We sample `top` as representative — all four
        // sides start equal (the card is spawned via BorderColor::all).
        if anim.captured_border.is_none() {
            anim.captured_border = Some(bc.top);
        }
        let original = anim.captured_border.unwrap();

        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / DURATION).min(1.0);

        // Border: heavy red at the start, fading back to original (quadratic falloff).
        let red_mix = (1.0 - t).max(0.0).powi(2);
        *bc = BorderColor::all(lerp_color(original, red, red_mix));

        // Scale dip — sine wave 1.0 → SCALE_DIP → 1.0.
        let phase = (std::f32::consts::PI * t).sin();
        let scale = 1.0 - phase * (1.0 - SCALE_DIP);
        node.width  = Val::Px(anim.base_w * scale);
        node.height = Val::Px(anim.base_h * scale);

        if anim.elapsed >= DURATION {
            *bc = BorderColor::all(original);
            node.width  = Val::Px(anim.base_w);
            node.height = Val::Px(anim.base_h);
            commands.entity(e).remove::<HitReactionAnimation>();
        }
    }
}

/// Linear interpolation between two `Color`s in sRGB space.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let sa = a.to_srgba();
    let sb = b.to_srgba();
    Color::srgba(
        sa.red   + (sb.red   - sa.red  ) * t,
        sa.green + (sb.green - sa.green) * t,
        sa.blue  + (sb.blue  - sa.blue ) * t,
        sa.alpha + (sb.alpha - sa.alpha) * t,
    )
}

// ─── Slam-down animation for newly-played units ──────────────────────────────

/// Briefly oversize a freshly summoned card, then settle to its base size —
/// the Hearthstone "thunk" when a card hits the board.
fn animate_slam(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Node, &mut SlamAnimation)>,
    time: Res<Time>,
) {
    const DURATION: f32 = 0.26;
    for (e, mut node, mut anim) in q.iter_mut() {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / DURATION).min(1.0);

        // Two-phase scale curve: start oversized → undershoot → settle.
        //   1.18  → 0.94 (first 55% of the animation, the "slam")
        //   0.94  → 1.00 (remaining 45%, the "settle")
        let scale = if t < 0.55 {
            let u = t / 0.55;
            1.18 + (0.94 - 1.18) * u
        } else {
            let u = (t - 0.55) / 0.45;
            0.94 + (1.0 - 0.94) * u
        };

        node.width  = Val::Px(anim.base_w * scale);
        node.height = Val::Px(anim.base_h * scale);

        if anim.elapsed >= DURATION {
            node.width  = Val::Px(anim.base_w);
            node.height = Val::Px(anim.base_h);
            commands.entity(e).remove::<SlamAnimation>();
        }
    }
}

// ─── Mouse tracking ──────────────────────────────────────────────────────────

/// Keep MousePos updated every frame from the window's cursor position.
fn track_mouse(
    mut mouse_pos: ResMut<MousePos>,
    window_q: Query<&Window>,
) {
    if let Some(window) = window_q.iter().next() {
        if let Some(pos) = window.cursor_position() {
            mouse_pos.0 = pos;
        }
    }
}

// ─── Drag-and-drop systems ───────────────────────────────────────────────────

/// On mouse-down over an affordable hand card → start drag, spawn ghost card.
fn handle_drag_start(
    game: Res<TcgGame>,
    mut drag_state: ResMut<DragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_pos: Res<MousePos>,
    mut commands: Commands,
    root_q: Query<Entity, With<BattleUiRoot>>,
    hand_q: Query<(&Interaction, &HandCardMarker), Changed<Interaction>>,
) {
    if !buttons.just_pressed(MouseButton::Left) { return; }
    if drag_state.dragging.is_some() { return; }
    if game.engine.is_over() || game.engine.state.active_player != HUMAN { return; }
    if !matches!(game.pending, Pending::None) { return; }

    for (intr, marker) in hand_q.iter() {
        if *intr != Interaction::Pressed { continue; }
        let p0 = &game.engine.state.players[HUMAN as usize];
        let Some(cid) = p0.hand.get(marker.0).cloned() else { continue };
        let Some(def) = game.engine.registry.get(&cid).cloned() else { continue };
        if p0.mana < def.cost { continue; }

        drag_state.dragging = Some(DragPayload { hand_idx: marker.0, card_id: cid, card_def: def.clone() });

        // Spawn a ghost card (absolute-positioned, follows cursor)
        let art_color = art_for_color(&def.color);
        let border_color = border_for_color(&def.color);
        let ghost = commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(mouse_pos.0.x - BOARD_CARD_W * 0.5),
                top: Val::Px(mouse_pos.0.y - BOARD_CARD_H * 0.5),
                width: Val::Px(BOARD_CARD_W),
                height: Val::Px(BOARD_CARD_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(art_color.to_srgba().red, art_color.to_srgba().green, art_color.to_srgba().blue, 0.80)),
            BorderColor::all(Color::srgba(border_color.to_srgba().red, border_color.to_srgba().green, border_color.to_srgba().blue, 1.0)),
            DragGhost,
        )).id();
        let name_txt = commands.spawn((
            Text::new(trunc(&def.name, 14)),
            TextFont { font: game.font.clone(), font_size: 12.0, ..default() },
            TextColor(Color::WHITE),
        )).id();
        commands.entity(ghost).add_child(name_txt);

        if let Some(root) = root_q.iter().next() {
            commands.entity(root).add_child(ghost);
        }
        drag_state.ghost_entity = Some(ghost);
        break;
    }
}

/// Each frame while dragging: move the ghost card to the cursor.
fn update_drag_ghost(
    drag_state: Res<DragState>,
    mouse_pos: Res<MousePos>,
    mut ghost_q: Query<&mut Node, With<DragGhost>>,
) {
    if drag_state.dragging.is_none() { return; }
    for mut node in ghost_q.iter_mut() {
        node.left = Val::Px(mouse_pos.0.x - BOARD_CARD_W * 0.5);
        node.top  = Val::Px(mouse_pos.0.y - BOARD_CARD_H * 0.5);
    }
}

/// On mouse-up: despawn ghost, play card based on drop zone (Y position).
///
/// Drop zones (screen Y, 820px window):
///   < 30px              → turn bar, invalid
///   30 – ~285px         → enemy side  (try to target first enemy unit)
///   ~285 – ~538px       → your side   (play with no target)
///   ≥ ~538px            → hand zone, cancel
fn handle_drag_end(
    mut game: ResMut<TcgGame>,
    mut drag_state: ResMut<DragState>,
    mut just_played: ResMut<JustPlayed>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_pos: Res<MousePos>,
    mut commands: Commands,
) {
    if !buttons.just_released(MouseButton::Left) { return; }

    // Always remove the ghost
    if let Some(ghost) = drag_state.ghost_entity.take() {
        commands.entity(ghost).despawn();
    }

    if let Some(payload) = drag_state.dragging.take() {
        let y = mouse_pos.0.y;
        let board_top    = HEADER_H;
        let board_bottom = board_top + 538.0;
        let midline      = board_top + 60.0 + HERO_ROW_H + ENEMY_BOARD_H;

        if y >= board_top && y < board_bottom {
            let target = if y < midline {
                game.engine.state.players[AI as usize].board
                    .first().map(|u| u.instance_id)
            } else {
                None
            };

            // Snapshot board length before the play so we can detect a brand-new
            // unit and tag it with the slam animation on the next rebuild.
            let before_len = game.engine.state.players[HUMAN as usize].board.len();
            match game.engine.play_card(HUMAN, &payload.card_id, target) {
                Ok(evs) => {
                    log_events(&evs, &mut game.log);
                    let p0 = &game.engine.state.players[HUMAN as usize];
                    if p0.board.len() > before_len {
                        if let Some(u) = p0.board.last() {
                            just_played.0 = Some(u.instance_id);
                        }
                    }
                }
                Err(e) => game.log.push(format!("⚠ {}", e)),
            }
            game.pending = Pending::None;
            game.needs_rebuild = true;
        }
    }

    drag_state.hover_zone = DropZoneType::NoZone;
    drag_state.is_valid   = false;
}

// ─── Drag-and-drop helpers ───────────────────────────────────────────────────

/// Check if a card can be played on a given drop zone
fn can_drop_on(card: &stonepyre_tcg::CardDefinition, zone: DropZoneType) -> bool {
    match zone {
        DropZoneType::EmptyBoard => {
            // Units, Relics, Champions can be played on board
            matches!(card.card_type, CardType::Unit | CardType::Relic | CardType::Champion)
        }
        DropZoneType::EnemyUnit(_) | DropZoneType::FriendlyUnit(_) => {
            // Targeted spells can target units
            card.spell_effect.as_ref()
                .map(|e| matches!(e.targeting, stonepyre_tcg::TargetingRule::EnemyUnit
                    | stonepyre_tcg::TargetingRule::FriendlyUnit
                    | stonepyre_tcg::TargetingRule::AnyUnit))
                .unwrap_or(false)
        }
        DropZoneType::EnemyHero | DropZoneType::FriendlyHero => {
            // Targeted spells can target heroes
            card.spell_effect.as_ref()
                .map(|e| matches!(e.targeting, stonepyre_tcg::TargetingRule::EnemyHero
                    | stonepyre_tcg::TargetingRule::FriendlyHero
                    | stonepyre_tcg::TargetingRule::AnyCharacter))
                .unwrap_or(false)
        }
        DropZoneType::NoZone => false,
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let asset_root = format!("{}/../../assets", manifest);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Stonepyre TCG".to_string(),
                        resolution: WindowResolution::new(1920_u32, 1080_u32),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root.clone(),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (
            track_mouse,
            handle_drag_start,
            update_drag_ghost,
            handle_drag_end,
        ))
        .add_systems(Update, (
            animate_hand_cards,
            init_hand_fan,
            animate_slam,
            animate_lunge,
            animate_hit_reaction,
        ))
        .add_systems(Update, (
            tick_ai_turn,
            rebuild_board,
        ))
        .add_systems(Update, (
            // Order matters: interactions resolve first so cancel-on-empty
            // doesn't fire on the same click as a legitimate target hit.
            handle_interactions,
            cancel_on_empty_click,
            update_attack_arrow,
        ).chain())
        .run();
}

/// Stonepyre TCG — browser-based battle test UI.
///
/// Run from game/ directory:
///   cargo run -p stonepyre_tcg --bin battle_web
///
/// Then open http://localhost:3030 in your browser.
/// Player 1 = you. Player 2 = AI.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rand::seq::SliceRandom;
use uuid::Uuid;

use stonepyre_tcg::{
    CardColor, CardRarity, CardType,
    deck::DeckDefinition,
    engine::{GameEngine, GameEvent},
    files::load_registry_from_dir,
    match_state::PlayerId,
    CardRegistry,
};

// ─── Paths ───────────────────────────────────────────────────────────────────

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/content/tcg/cards")
}
fn decks_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/content/tcg/decks")
}

// ─── Server state ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
enum Pending {
    None,
    PlayCard { hand_idx: usize },
    AttackWith { board_idx: usize },
    ChampionPower,
}

struct WebState {
    engine: GameEngine,
    registry: CardRegistry,
    deck_names: (String, String),
    log: Vec<String>,
    pending: Pending,
    started: bool,
    available_decks: Vec<DeckDefinition>,
    p1_deck_choice: Option<usize>,
    p2_deck_choice: Option<usize>,
}

const HUMAN: PlayerId = 0;
const AI: PlayerId = 1;

// ─── AI logic ────────────────────────────────────────────────────────────────

fn run_ai_turn(engine: &mut GameEngine, log: &mut Vec<String>, registry: &CardRegistry) {
    let opponent = AI ^ 1;
    log.push("── AI turn ──".into());

    loop {
        let mana = engine.state.players[AI as usize].mana;
        let mut playable: Vec<(usize, u8)> = engine.state.players[AI as usize].hand.iter().enumerate()
            .filter_map(|(i, cid)| registry.get(cid).filter(|d| d.cost <= mana).map(|d| (i, d.cost)))
            .collect();
        playable.sort_by_key(|&(_, c)| c);

        let Some((idx, _)) = playable.first().copied() else { break };

        let card_id = engine.state.players[AI as usize].hand[idx].clone();
        let def = registry.get(&card_id).unwrap().clone();

        let target: Option<Uuid> = if def.is_spell() {
            def.spell_effect.as_ref().and_then(|fx| {
                if !fx.targeting.requires_target() { return None; }
                match fx.targeting {
                    stonepyre_tcg::TargetingRule::EnemyUnit | stonepyre_tcg::TargetingRule::AnyUnit =>
                        engine.state.players[opponent as usize].board.first().map(|u| u.instance_id),
                    stonepyre_tcg::TargetingRule::FriendlyUnit =>
                        engine.state.players[AI as usize].board.iter().min_by_key(|u| u.current_health).map(|u| u.instance_id),
                    _ => None,
                }
            })
        } else { None };

        match engine.play_card(AI, &card_id, target) {
            Ok(evs) => log_events(&evs, log),
            Err(_) => break,
        }
        if engine.is_over() { return; }
    }

    // Champion power
    if let Some(champ) = &engine.state.players[AI as usize].champion_in_play.clone() {
        if let Some(def) = registry.get(&champ.card_def_id).cloned() {
            if let Some(power) = &def.champion_power {
                let mana = engine.state.players[AI as usize].mana;
                if mana >= power.cost && !engine.state.players[AI as usize].champion_power_used {
                    let target: Option<Uuid> = if power.targeting.requires_target() {
                        match power.targeting {
                            stonepyre_tcg::TargetingRule::EnemyUnit | stonepyre_tcg::TargetingRule::AnyUnit =>
                                engine.state.players[opponent as usize].board.first().map(|u| u.instance_id),
                            stonepyre_tcg::TargetingRule::FriendlyUnit =>
                                engine.state.players[AI as usize].board.iter().min_by_key(|u| u.current_health).map(|u| u.instance_id),
                            _ => None,
                        }
                    } else { None };
                    if let Ok(evs) = engine.use_champion_power(AI, target) { log_events(&evs, log); }
                    if engine.is_over() { return; }
                }
            }
        }
    }

    // Attack
    loop {
        let attackers: Vec<Uuid> = engine.state.players[AI as usize].board.iter()
            .filter(|u| u.can_attack()).map(|u| u.instance_id).collect();
        let Some(atk_id) = attackers.first().copied() else { break };

        let result = if let Some(gid) = engine.state.players[opponent as usize].board.iter().find(|u| u.has_guard()).map(|u| u.instance_id) {
            engine.attack_unit(AI, atk_id, gid)
        } else if let Some(weakest) = engine.state.players[opponent as usize].board.iter().min_by_key(|u| u.current_health).map(|u| u.instance_id) {
            engine.attack_unit(AI, atk_id, weakest)
        } else {
            engine.attack_hero(AI, atk_id)
        };

        match result {
            Ok(evs) => log_events(&evs, log),
            Err(_) => break,
        }
        if engine.is_over() { return; }
    }

    if let Ok(evs) = engine.end_turn(AI) { log_events(&evs, log); }
}

fn log_events(events: &[GameEvent], log: &mut Vec<String>) {
    for ev in events {
        let line = match ev {
            GameEvent::CardPlayed { player, card_name, .. } => format!("P{} plays {}", player+1, card_name),
            GameEvent::UnitSummoned { player, card_name, attack, health, .. } => format!("P{} summons {} ({}/{})", player+1, card_name, attack, health),
            GameEvent::TokenSummoned { player, name, attack, health, .. } => format!("P{} summons {} token ({}/{})", player+1, name, attack, health),
            GameEvent::SpellCast { player, card_name } => format!("P{} casts {}", player+1, card_name),
            GameEvent::RelicPlayed { player, card_name, durability } => format!("P{} plays relic {} [{}]", player+1, card_name, durability),
            GameEvent::ChampionPlayed { player, card_name } => format!("P{} plays champion {}!", player+1, card_name),
            GameEvent::UnitAttackedUnit { attacker_name, defender_name, damage_dealt, damage_taken } =>
                format!("{} → {} (dealt {}, took {})", attacker_name, defender_name, damage_dealt, damage_taken),
            GameEvent::UnitAttackedHero { attacker_name, target_player, damage } =>
                format!("{} attacks P{} hero for {}", attacker_name, target_player+1, damage),
            GameEvent::HeroDamaged { player, amount, remaining_health } =>
                format!("P{} takes {} dmg → {} HP", player+1, amount, remaining_health),
            GameEvent::HeroHealed { player, amount, new_health } =>
                format!("P{} healed {} → {} HP", player+1, amount, new_health),
            GameEvent::UnitDied { player, card_name } => format!("✝ {} (P{})", card_name, player+1),
            GameEvent::UnitBuffed { card_name, attack_delta, health_delta } =>
                format!("{} +{}/{}", card_name, attack_delta, health_delta),
            GameEvent::ShieldedBlocked { card_name } => format!("🛡 {}'s shield absorbed the hit", card_name),
            GameEvent::ChampionPowerUsed { player, champion_name, .. } => format!("P{} uses {}'s power", player+1, champion_name),
            GameEvent::TurnStarted { player, turn, mana } => format!("── Turn {} · P{} · {} mana ──", turn, player+1, mana),
            GameEvent::TurnEnded { player } => format!("P{} ends turn", player+1),
            GameEvent::CardDrawn { player, card_name } => format!("P{} draws {}", player+1, card_name),
            GameEvent::GameOver { winner } => format!("🏆 Player {} wins!", winner+1),
            GameEvent::SpellEffectUnimplemented { card_name, .. } => format!("[!] {}: effect not yet implemented", card_name),
            _ => continue,
        };
        log.push(line);
    }
    // Keep log at most 30 entries
    if log.len() > 30 { let drain = log.len() - 30; log.drain(0..drain); }
}

// ─── CSS ─────────────────────────────────────────────────────────────────────

const CSS: &str = r#"
* { box-sizing: border-box; margin: 0; padding: 0; }
body { background: #0a0a14; color: #eee; font-family: 'Segoe UI', sans-serif; font-size: 13px; min-height: 100vh; }

/* ── Layout ── */
.page { display: flex; flex-direction: column; gap: 0; min-height: 100vh; }
.header { background: #111122; border-bottom: 2px solid #333; padding: 10px 16px; display: flex; gap: 24px; align-items: center; }
.header h1 { font-size: 16px; color: #ffd700; letter-spacing: 2px; }
.hpbar { display: flex; align-items: center; gap: 8px; }
.hpbar span { font-size: 12px; color: #aaa; }
.hp-fill { height: 14px; border-radius: 6px; background: linear-gradient(90deg, #27ae60, #2ecc71); border: 1px solid #1a7a40; transition: width 0.3s; }
.hp-bg { width: 120px; background: #1a1a2e; border: 1px solid #333; border-radius: 6px; overflow: hidden; }
.mana-pips { display: flex; gap: 3px; }
.pip { width: 12px; height: 12px; border-radius: 50%; background: #1a5ca8; border: 1px solid #2980b9; }
.pip.used { background: #1a2a3a; border-color: #333; }
.turn-label { font-size: 11px; color: #ffd700; font-weight: bold; }
.zone-label { font-size: 10px; color: #888; padding: 6px 8px 2px; text-transform: uppercase; letter-spacing: 1px; }

.board-zone { min-height: 230px; padding: 8px; display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-end; border-bottom: 1px solid #1a1a2e; }
.board-zone.enemy { background: rgba(120,0,0,0.08); border-bottom: 2px solid #2a2a2a; }
.board-zone.yours  { background: rgba(0,60,120,0.08); }
.hand-zone  { min-height: 230px; padding: 8px; display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-end; background: rgba(0,0,0,0.3); border-top: 2px solid #333; }

.footer { display: flex; gap: 12px; padding: 10px 12px; background: #111122; border-top: 2px solid #333; align-items: center; flex-wrap: wrap; }

/* ── Buttons ── */
.btn { padding: 6px 14px; border: none; border-radius: 5px; cursor: pointer; font-size: 12px; font-weight: bold; text-decoration: none; display: inline-block; }
.btn-end   { background: #c0392b; color: #fff; }
.btn-power { background: #f39c12; color: #000; }
.btn-play  { background: #27ae60; color: #fff; margin-top: 4px; width: 100%; text-align: center; }
.btn-attack { background: #e74c3c; color: #fff; margin-top: 4px; width: 100%; text-align: center; }
.btn-target { background: #8e44ad; color: #fff; margin-top: 4px; width: 100%; text-align: center; }
.btn-cancel { background: #555; color: #fff; }
.btn:hover { opacity: 0.85; }
.btn:disabled, .btn.disabled { opacity: 0.4; cursor: not-allowed; }

/* ── Card template ── */
.card-wrap { display: flex; flex-direction: column; align-items: center; }
.card {
  width: 120px; height: 180px;
  border-radius: 10px;
  border: 3px solid #555;
  background: #0d0d1a;
  display: flex; flex-direction: column;
  position: relative;
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0,0,0,0.6);
  transition: transform 0.15s, box-shadow 0.15s;
  cursor: default;
}
.card:hover { transform: translateY(-6px) scale(1.04); box-shadow: 0 10px 24px rgba(0,0,0,0.8); }
.card.exhausted { opacity: 0.55; }
.card.can-act { box-shadow: 0 0 12px rgba(255,215,0,0.5), 0 4px 12px rgba(0,0,0,0.6); }
.card.targeted { box-shadow: 0 0 14px rgba(200,60,200,0.7), 0 4px 12px rgba(0,0,0,0.6); }

/* Mana gem */
.card .mana {
  position: absolute; top: 5px; left: 5px; z-index: 10;
  width: 24px; height: 24px; border-radius: 50%;
  background: #1a4a8a; border: 2px solid #3a80e8;
  display: flex; align-items: center; justify-content: center;
  font-size: 14px; font-weight: bold; color: #fff;
  text-shadow: 0 1px 3px #000;
}

/* Art placeholder */
.card .art {
  height: 60px;
  margin: 8px 6px 0 6px;
  border-radius: 5px;
  border: 1px solid rgba(255,255,255,0.1);
  position: relative;
  overflow: hidden;
}
.card .art img { width: 100%; height: 100%; object-fit: cover; border-radius: 4px; }

/* Name */
.card .name {
  padding: 3px 6px 1px;
  font-size: 9.5px; font-weight: bold;
  color: #f5d060;
  text-align: center;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  text-shadow: 0 1px 2px #000;
}

/* Type label */
.card .type-line {
  text-align: center;
  font-size: 7px; color: #999;
  font-style: italic;
  padding: 0 4px 1px;
}

/* Rules text */
.card .rules {
  padding: 2px 5px;
  font-size: 7.5px; color: #ccc;
  flex: 1; overflow: hidden;
  line-height: 1.4;
}

/* Stats footer */
.card .stats {
  display: flex; justify-content: space-between; align-items: center;
  padding: 3px 5px 4px; margin-top: auto;
}
.sc {
  width: 22px; height: 22px; border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  font-size: 13px; font-weight: bold; color: #fff;
  text-shadow: 0 1px 2px #000;
}
.sc-atk { background: #c0392b; border: 2px solid #e74c3c; }
.sc-hp  { background: #1e8449; border: 2px solid #27ae60; }
.sc-dur { background: #b7770d; border: 2px solid #e67e22; }

/* Shielded indicator */
.shield-badge {
  position: absolute; top: 5px; right: 5px; z-index: 10;
  font-size: 14px;
}

/* ── Color borders and art gradients ── */
.c-red    { border-color: #e74c3c; }
.c-green  { border-color: #27ae60; }
.c-black  { border-color: #8e44ad; }
.c-white  { border-color: #ecf0f1; }
.c-blue   { border-color: #2980b9; }
.c-purple { border-color: #9b59b6; }
.c-neutral { border-color: #7f8c8d; }

.c-red    .art { background: linear-gradient(135deg, #7f1010, #c0392b, #e74c3c); }
.c-green  .art { background: linear-gradient(135deg, #1a4a1a, #196f3d, #27ae60); }
.c-black  .art { background: linear-gradient(135deg, #1a0033, #4a235a, #6c3483); }
.c-white  .art { background: linear-gradient(135deg, #5d4037, #a0700a, #d4ac0d); }
.c-blue   .art { background: linear-gradient(135deg, #002244, #1a5276, #2980b9); }
.c-purple .art { background: linear-gradient(135deg, #2d0050, #6c2680, #9b59b6); }
.c-neutral .art { background: linear-gradient(135deg, #1c2833, #2c3e50, #7f8c8d); }

/* ── Card type overlays ── */
.t-champion { box-shadow: 0 0 14px rgba(255,215,0,0.5), 0 4px 12px rgba(0,0,0,0.6) !important; border-width: 3px; }
.t-champion .art::after {
  content: '★'; position: absolute; bottom: 2px; right: 4px;
  font-size: 12px; color: rgba(255,215,0,0.6);
}
.t-relic .art::after {
  content: '◈'; position: absolute; bottom: 2px; right: 4px;
  font-size: 12px; color: rgba(255,165,0,0.6);
}
.t-spell .art::after {
  content: '✦'; position: absolute; bottom: 2px; right: 4px;
  font-size: 12px; color: rgba(200,200,255,0.6);
}

/* Rarity pips */
.rarity-bar { text-align: center; font-size: 8px; letter-spacing: 2px; padding-bottom: 2px; color: #666; }
.r-common    .rarity-bar { color: #888; }
.r-uncommon  .rarity-bar { color: #3498db; }
.r-rare      .rarity-bar { color: #9b59b6; }
.r-epic      .rarity-bar { color: #e74c3c; }
.r-champion  .rarity-bar { color: #f1c40f; }

/* Pending target mode banner */
.target-banner {
  background: #4a1080; border: 2px solid #9b59b6;
  padding: 8px 14px; border-radius: 8px;
  font-size: 13px; color: #e8c4ff;
  display: flex; align-items: center; gap: 12px;
}

/* Game log */
.log-box {
  max-height: 200px; overflow-y: auto;
  background: #0a0a14; border: 1px solid #222;
  padding: 6px 10px;
  font-size: 11px; color: #aaa;
  border-radius: 6px; width: 320px;
  flex-shrink: 0;
}
.log-box div { border-bottom: 1px solid #151525; padding: 2px 0; }
.log-box div:last-child { color: #fff; }

/* Game over overlay */
.game-over-banner {
  position: fixed; top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(0,0,0,0.85);
  display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  z-index: 100;
}
.game-over-banner h2 { font-size: 48px; color: #ffd700; margin-bottom: 24px; }
.game-over-banner .deck-select-form { background: #111122; padding: 32px; border-radius: 12px; border: 2px solid #333; min-width: 400px; }
.form-row { margin-bottom: 14px; }
.form-row label { display: block; color: #aaa; font-size: 12px; margin-bottom: 4px; }
.form-row select { width: 100%; padding: 6px; background: #1a1a2e; border: 1px solid #333; color: #eee; border-radius: 5px; font-size: 13px; }
"#;

// ─── HTML card renderer ───────────────────────────────────────────────────────

fn color_class(c: &CardColor) -> &'static str {
    match c {
        CardColor::Red => "c-red", CardColor::Green => "c-green", CardColor::Black => "c-black",
        CardColor::White => "c-white", CardColor::Blue => "c-blue", CardColor::Purple => "c-purple",
        CardColor::Neutral => "c-neutral",
    }
}
fn type_class(t: &CardType) -> &'static str {
    match t { CardType::Unit => "t-unit", CardType::Spell => "t-spell", CardType::Relic => "t-relic", CardType::Champion => "t-champion" }
}
fn type_label(t: &CardType) -> &'static str {
    match t { CardType::Unit => "Unit", CardType::Spell => "Spell", CardType::Relic => "Relic", CardType::Champion => "Champion" }
}
fn rarity_class(r: &CardRarity) -> &'static str {
    match r { CardRarity::Common => "r-common", CardRarity::Uncommon => "r-uncommon", CardRarity::Rare => "r-rare", CardRarity::Epic => "r-epic", CardRarity::Champion => "r-champion" }
}
fn rarity_pips(r: &CardRarity) -> &'static str {
    match r { CardRarity::Common => "●", CardRarity::Uncommon => "●●", CardRarity::Rare => "●●●", CardRarity::Epic => "◆◆◆◆", CardRarity::Champion => "★" }
}

fn render_card_def(def: &stonepyre_tcg::CardDefinition, extra_classes: &str, action_html: &str) -> String {
    let cc = color_class(&def.color);
    let tc = type_class(&def.card_type);
    let rc = rarity_class(&def.rarity);
    let tl = type_label(&def.card_type);

    let stats_html = match def.card_type {
        CardType::Unit | CardType::Champion => {
            let atk = def.attack.unwrap_or(0);
            let hp  = def.health.unwrap_or(0);
            format!(r#"<div class="stats"><div class="sc sc-atk">{atk}</div><div class="sc sc-hp">{hp}</div></div>"#)
        }
        CardType::Relic => {
            let dur = def.durability.unwrap_or(0);
            format!(r#"<div class="stats"><div></div><div class="sc sc-dur">{dur}</div></div>"#)
        }
        CardType::Spell => r#"<div class="stats"></div>"#.to_string(),
    };

    format!(r#"
<div class="card-wrap">
  <div class="card {cc} {tc} {rc} {extra_classes}">
    <div class="mana">{cost}</div>
    <div class="art"></div>
    <div class="name" title="{name}">{name}</div>
    <div class="type-line">{tl}</div>
    <div class="rules">{rules}</div>
    {stats_html}
    <div class="rarity-bar">{pips}</div>
  </div>
  {action_html}
</div>"#,
        cost = def.cost,
        name = html_escape(&def.name),
        rules = html_escape(&def.rules_text),
        pips = rarity_pips(&def.rarity),
    )
}

fn render_unit_in_play(
    unit: &stonepyre_tcg::UnitInPlay,
    registry: &CardRegistry,
    board_idx: usize,
    is_yours: bool,
    pending: &Pending,
) -> String {
    let def = registry.get(&unit.card_def_id);
    let cc = def.map(|d| color_class(&d.color)).unwrap_or("c-neutral");
    let rarity_html = def.map(|d| format!(r#"<div class="rarity-bar">{}</div>"#, rarity_pips(&d.rarity))).unwrap_or_default();
    let rules = def.map(|d| html_escape(&d.rules_text)).unwrap_or_default();

    let extra = if unit.is_shielded { "targeted" } else { "" };
    let can_act = is_yours && unit.can_attack();
    let act_class = if can_act { "can-act" } else if is_yours && unit.has_attacked { "exhausted" } else { "" };

    let shield_badge = if unit.is_shielded { r#"<div class="shield-badge">🛡</div>"# } else { "" };

    let kws: Vec<&str> = unit.keywords.iter().map(|k| k.display_name()).collect();
    let kw_text = if kws.is_empty() { rules } else { format!("{} ({})", rules, kws.join(", ")) };

    let action_html = if is_yours {
        match pending {
            Pending::None if can_act =>
                format!(r#"<a class="btn btn-attack" href="/action?cmd=attack_select&atk={board_idx}">⚔ Attack</a>"#),
            _ => String::new(),
        }
    } else {
        match pending {
            Pending::AttackWith { board_idx: atk_idx } =>
                format!(r#"<a class="btn btn-target" href="/action?cmd=attack_unit&atk={atk_idx}&def={board_idx}">🎯 Target</a>"#),
            Pending::PlayCard { hand_idx } => {
                format!(r#"<a class="btn btn-target" href="/action?cmd=play_targeted&n={hand_idx}&target=e{board_idx}">🎯 Target</a>"#)
            }
            Pending::ChampionPower =>
                format!(r#"<a class="btn btn-target" href="/action?cmd=power_targeted&target=e{board_idx}">🎯 Target</a>"#),
            _ => String::new(),
        }
    };

    // Friendly targeting (heal / buff)
    let friendly_action = if is_yours {
        match pending {
            Pending::PlayCard { hand_idx } =>
                format!(r#"<a class="btn btn-target" href="/action?cmd=play_targeted&n={hand_idx}&target=f{board_idx}">🎯 Target</a>"#),
            Pending::ChampionPower =>
                format!(r#"<a class="btn btn-target" href="/action?cmd=power_targeted&target=f{board_idx}">🎯 Target</a>"#),
            _ => String::new(),
        }
    } else { String::new() };

    let combined_action = format!("{action_html}{friendly_action}");

    format!(r#"
<div class="card-wrap">
  <div class="card {cc} {act_class} {extra}">
    {shield_badge}
    <div class="mana">{cost}</div>
    <div class="art"></div>
    <div class="name" title="{name}">{name}</div>
    <div class="type-line">Unit</div>
    <div class="rules">{kw_text}</div>
    <div class="stats">
      <div class="sc sc-atk">{atk}</div>
      <div class="sc sc-hp">{hp}</div>
    </div>
    {rarity_html}
  </div>
  {combined_action}
</div>"#,
        cost = def.map(|d| d.cost).unwrap_or(0),
        name = html_escape(&unit.display_name),
        atk = unit.current_attack,
        hp = unit.current_health,
    )
}

fn render_hand_card(card_id: &str, hand_idx: usize, registry: &CardRegistry, mana: u8, pending: &Pending) -> String {
    let Some(def) = registry.get(card_id) else { return String::new(); };
    let affordable = mana >= def.cost;
    let extra = if !affordable { "exhausted" } else { "" };

    let action_html = if affordable {
        match pending {
            Pending::None => {
                // Check if spell needs a target
                let needs_target = def.is_spell() && def.spell_effect.as_ref().map(|fx| fx.targeting.requires_target()).unwrap_or(false);
                if needs_target {
                    format!(r#"<a class="btn btn-play" href="/action?cmd=play_select&n={hand_idx}">▶ Play</a>"#)
                } else {
                    format!(r#"<a class="btn btn-play" href="/action?cmd=play&n={hand_idx}">▶ Play</a>"#)
                }
            }
            _ => String::new(),
        }
    } else { String::new() };

    render_card_def(def, extra, &action_html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ─── Full page renderer ───────────────────────────────────────────────────────

fn render_page(state: &WebState) -> String {
    let engine = &state.engine;
    let registry = &state.registry;
    let p1 = &engine.state.players[0];
    let p2 = &engine.state.players[1];
    let active = engine.state.active_player;

    // HP bars
    let hp_pct = |hp: i32, max: i32| (hp.max(0) as f32 / max as f32 * 100.0) as u32;

    let mana_pips = |current: u8, max: u8| -> String {
        (0..max).map(|i| {
            if i < current { r#"<div class="pip"></div>"# } else { r#"<div class="pip used"></div>"# }
        }).collect()
    };

    let turn_label = if engine.is_over() {
        format!("🏆 Player {} Wins!", engine.winner().unwrap() + 1)
    } else {
        format!("Turn {}  ·  Player {} active", engine.state.turn, active + 1)
    };

    // ── Header ──
    let header = format!(r#"
<div class="header">
  <h1>STONEPYRE TCG</h1>
  <div class="turn-label">{turn_label}</div>
  <div class="hpbar">
    <span>P1 <b style="color:#eee">{p1hp}/{p1mhp}</b></span>
    <div class="hp-bg"><div class="hp-fill" style="width:{p1pct}%"></div></div>
  </div>
  <div class="mana-pips" title="P1 mana: {p1mana}/{p1mmana}">
    {p1pips}
  </div>
  <div style="width:1px;background:#333;height:30px;margin:0 4px;"></div>
  <div class="hpbar">
    <span>P2(AI) <b style="color:#eee">{p2hp}/{p2mhp}</b></span>
    <div class="hp-bg"><div class="hp-fill" style="width:{p2pct}%;background:linear-gradient(90deg,#e74c3c,#c0392b)"></div></div>
  </div>
  <div class="mana-pips" title="P2 mana: {p2mana}/{p2mmana}">
    {p2pips}
  </div>
  <div style="margin-left:auto;color:#666;font-size:11px">{p1name} vs {p2name}</div>
</div>"#,
        p1hp = p1.health, p1mhp = p1.max_health, p1pct = hp_pct(p1.health, p1.max_health),
        p1mana = p1.mana, p1mmana = p1.max_mana, p1pips = mana_pips(p1.mana, p1.max_mana.max(1)),
        p2hp = p2.health, p2mhp = p2.max_health, p2pct = hp_pct(p2.health, p2.max_health),
        p2mana = p2.mana, p2mmana = p2.max_mana, p2pips = mana_pips(p2.mana, p2.max_mana.max(1)),
        p1name = html_escape(&state.deck_names.0),
        p2name = html_escape(&state.deck_names.1),
    );

    // ── Enemy board ──
    let mut enemy_cards = String::new();
    for (i, unit) in p2.board.iter().enumerate() {
        enemy_cards.push_str(&render_unit_in_play(unit, registry, i, false, &state.pending));
    }
    if let Some(champ) = &p2.champion_in_play {
        if let Some(def) = registry.get(&champ.card_def_id) {
            let cc = color_class(&def.color);
            enemy_cards.push_str(&format!(r#"
<div class="card-wrap">
  <div class="card {cc} t-champion" title="Champion in play">
    <div class="mana">{cost}</div>
    <div class="art"></div>
    <div class="name">★ {name}</div>
    <div class="type-line">Champion</div>
    <div class="rules">{rules}</div>
    <div class="stats"><div class="sc sc-atk">{atk}</div><div class="sc sc-hp">{hp}</div></div>
  </div>
</div>"#,
                cost = def.cost,
                name = html_escape(&champ.display_name),
                rules = html_escape(&def.rules_text),
                atk = champ.current_attack.unwrap_or(0),
                hp = champ.current_health.unwrap_or(0),
            ));
        }
    }
    for relic in &p2.relics {
        if let Some(def) = registry.get(&relic.card_def_id) {
            let cc = color_class(&def.color);
            enemy_cards.push_str(&format!(r#"
<div class="card-wrap">
  <div class="card {cc} t-relic" style="opacity:0.8">
    <div class="art"></div>
    <div class="name">{name}</div>
    <div class="type-line">Relic</div>
    <div class="rules">{rules}</div>
    <div class="stats"><div></div><div class="sc sc-dur">{dur}</div></div>
  </div>
</div>"#,
                name = html_escape(&relic.display_name),
                rules = html_escape(&def.rules_text),
                dur = relic.remaining_durability,
            ));
        }
    }
    // Attack hero button when in AttackWith mode
    let attack_hero_btn = if let Pending::AttackWith { board_idx: atk_idx } = &state.pending {
        format!(r#"<a class="btn btn-attack" style="height:40px;display:flex;align-items:center" href="/action?cmd=attack_hero&atk={atk_idx}">⚔ Attack Hero</a>"#)
    } else { String::new() };

    // ── Your board ──
    let mut your_cards = String::new();
    for (i, unit) in p1.board.iter().enumerate() {
        your_cards.push_str(&render_unit_in_play(unit, registry, i, true, &state.pending));
    }
    if let Some(champ) = &p1.champion_in_play {
        if let Some(def) = registry.get(&champ.card_def_id) {
            let cc = color_class(&def.color);
            your_cards.push_str(&format!(r#"
<div class="card-wrap">
  <div class="card {cc} t-champion can-act" title="Champion in play">
    <div class="mana">{cost}</div>
    <div class="art"></div>
    <div class="name">★ {name}</div>
    <div class="type-line">Champion</div>
    <div class="rules">{rules}</div>
    <div class="stats"><div class="sc sc-atk">{atk}</div><div class="sc sc-hp">{hp}</div></div>
  </div>
</div>"#,
                cost = def.cost,
                name = html_escape(&champ.display_name),
                rules = html_escape(&def.rules_text),
                atk = champ.current_attack.unwrap_or(0),
                hp = champ.current_health.unwrap_or(0),
            ));
        }
    }
    for relic in &p1.relics {
        if let Some(def) = registry.get(&relic.card_def_id) {
            let cc = color_class(&def.color);
            your_cards.push_str(&format!(r#"
<div class="card-wrap">
  <div class="card {cc} t-relic" style="opacity:0.8">
    <div class="art"></div>
    <div class="name">{name}</div>
    <div class="type-line">Relic</div>
    <div class="rules">{rules}</div>
    <div class="stats"><div></div><div class="sc sc-dur">{dur}</div></div>
  </div>
</div>"#,
                name = html_escape(&relic.display_name),
                rules = html_escape(&def.rules_text),
                dur = relic.remaining_durability,
            ));
        }
    }

    // ── Hand ──
    let mut hand_cards = String::new();
    for (i, card_id) in p1.hand.iter().enumerate() {
        hand_cards.push_str(&render_hand_card(card_id, i, registry, p1.mana, &state.pending));
    }

    // ── Pending banner ──
    let pending_banner = match &state.pending {
        Pending::PlayCard { hand_idx } => {
            let name = p1.hand.get(*hand_idx).and_then(|id| registry.get(id)).map(|d| d.name.as_str()).unwrap_or("?");
            format!(r#"<div class="target-banner">🎯 Select a target for <b>{}</b>  <a class="btn btn-cancel" href="/action?cmd=cancel">Cancel</a></div>"#, html_escape(name))
        }
        Pending::AttackWith { board_idx } => {
            let name = p1.board.get(*board_idx).map(|u| u.display_name.as_str()).unwrap_or("?");
            format!(r#"<div class="target-banner">⚔ Attacking with <b>{}</b> — select target or Attack Hero  <a class="btn btn-cancel" href="/action?cmd=cancel">Cancel</a></div>"#, html_escape(name))
        }
        Pending::ChampionPower => {
            r#"<div class="target-banner">★ Select a target for Champion Power  <a class="btn btn-cancel" href="/action?cmd=cancel">Cancel</a></div>"#.to_string()
        }
        Pending::None => String::new(),
    };

    // ── Champion power button ──
    let power_btn = if let Some(champ) = &p1.champion_in_play {
        if let Some(def) = registry.get(&champ.card_def_id) {
            if let Some(power) = &def.champion_power {
                let usable = p1.mana >= power.cost && !p1.champion_power_used && !engine.is_over();
                let disabled = if usable { "" } else { "disabled" };
                let href = if usable {
                    if power.targeting.requires_target() {
                        "/action?cmd=power_select".to_string()
                    } else {
                        "/action?cmd=power".to_string()
                    }
                } else { "#".to_string() };
                let used_label = if p1.champion_power_used { " [USED]" } else { "" };
                format!(r#"<a class="btn btn-power {disabled}" href="{href}">★ Power ({cost} mana): {desc}{used_label}</a>"#,
                    cost = power.cost, desc = html_escape(&power.description))
            } else { String::new() }
        } else { String::new() }
    } else { String::new() };

    // ── Footer buttons ──
    let end_btn = if !engine.is_over() && active == HUMAN && state.pending == Pending::None {
        r#"<a class="btn btn-end" href="/action?cmd=end">⏭ End Turn</a>"#.to_string()
    } else { String::new() };

    // ── Log ──
    let log_html: String = state.log.iter().rev().take(20).map(|l| format!("<div>{}</div>", html_escape(l))).collect();

    // ── Game over overlay ──
    let overlay = if engine.is_over() {
        let winner = engine.winner().unwrap();
        let deck_options: String = state.available_decks.iter().enumerate()
            .map(|(i, d)| format!(r#"<option value="{i}">{} ({} cards)</option>"#, html_escape(&d.name), d.card_ids.len()))
            .collect();
        format!(r#"
<div class="game-over-banner">
  <h2>Player {} Wins!</h2>
  <form class="deck-select-form" action="/start" method="get">
    <div class="form-row"><label>Your Deck</label>
      <select name="p1">{deck_options}</select></div>
    <div class="form-row"><label>AI Deck</label>
      <select name="p2">{deck_options}</select></div>
    <button class="btn btn-play" type="submit">&#9654; Play Again</button>
  </form>
</div>"#, winner + 1)
    } else { String::new() };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Stonepyre TCG</title>
<style>{CSS}</style>
</head>
<body>
<div class="page">
  {header}
  {pending_banner}
  <div class="zone-label">Enemy Board — Player 2</div>
  <div class="board-zone enemy">
    {enemy_cards}
    {attack_hero_btn}
  </div>
  <div class="zone-label">Your Board — Player 1</div>
  <div class="board-zone yours">
    {your_cards}
  </div>
  <div class="zone-label">Your Hand ({hand_size} cards · {mana}/{max_mana} mana)</div>
  <div class="hand-zone">
    {hand_cards}
  </div>
  <div class="footer">
    {end_btn}
    {power_btn}
    <div class="log-box">{log_html}</div>
  </div>
</div>
{overlay}
</body>
</html>"#,
        hand_size = p1.hand.len(),
        mana = p1.mana,
        max_mana = p1.max_mana,
    )
}

// ─── HTTP server ─────────────────────────────────────────────────────────────

fn parse_query(qs: &str) -> HashMap<String, String> {
    qs.split('&').filter_map(|pair| {
        let mut p = pair.splitn(2, '=');
        let k = p.next()?.to_string();
        let v = p.next().unwrap_or("").to_string();
        Some((k, v))
    }).collect()
}

fn send_response(mut stream: TcpStream, status: &str, content_type: &str, body: &str) {
    let _ = write!(stream,
        "HTTP/1.1 {}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, content_type, body.len(), body);
}

fn redirect(mut stream: TcpStream, location: &str) {
    let _ = write!(stream,
        "HTTP/1.1 303 See Other\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        location);
}

fn handle_connection(stream: TcpStream, state: Arc<Mutex<WebState>>) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() { return; }
    let request_line = request_line.trim().to_string();

    // Drain headers
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() { break; }
        if line.trim().is_empty() { break; }
    }

    let parts: Vec<&str> = request_line.splitn(3, ' ').collect();
    if parts.len() < 2 { return; }
    let path = parts[1];

    let (route, query_str) = if let Some(q) = path.find('?') {
        (&path[..q], &path[q+1..])
    } else {
        (path, "")
    };

    let params = parse_query(query_str);

    match route {
        "/" => {
            let s = state.lock().unwrap();
            let html = render_page(&s);
            drop(s);
            send_response(stream, "200 OK", "text/html", &html);
        }
        "/start" => {
            let p1_idx = params.get("p1").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            let p2_idx = params.get("p2").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
            {
                let mut s = state.lock().unwrap();
                start_game(&mut s, p1_idx, p2_idx);
            }
            redirect(stream, "/");
        }
        "/action" => {
            let cmd = params.get("cmd").map(|s| s.as_str()).unwrap_or("");
            let mut s = state.lock().unwrap();

            if s.engine.is_over() { drop(s); redirect(stream, "/"); return; }
            if s.engine.state.active_player != HUMAN && cmd != "cancel" { drop(s); redirect(stream, "/"); return; }

            match cmd {
                "end" => {
                    s.pending = Pending::None;
                    if let Ok(evs) = s.engine.end_turn(HUMAN) { log_events(&evs, &mut s.log); }
                    if !s.engine.is_over() {
                        let ws = &mut *s;
                        let reg = ws.registry.clone();
                        run_ai_turn(&mut ws.engine, &mut ws.log, &reg);
                    }
                }
                "play" => {
                    if let Some(n) = params.get("n").and_then(|v| v.parse::<usize>().ok()) {
                        let card_id = s.engine.state.players[HUMAN as usize].hand.get(n).cloned();
                        if let Some(cid) = card_id {
                            match s.engine.play_card(HUMAN, &cid, None) {
                                Ok(evs) => log_events(&evs, &mut s.log),
                                Err(e) => s.log.push(format!("⚠ {e}")),
                            }
                        }
                    }
                    s.pending = Pending::None;
                }
                "play_select" => {
                    if let Some(n) = params.get("n").and_then(|v| v.parse::<usize>().ok()) {
                        s.pending = Pending::PlayCard { hand_idx: n };
                    }
                }
                "play_targeted" => {
                    let n = params.get("n").and_then(|v| v.parse::<usize>().ok());
                    let target_str = params.get("target").cloned().unwrap_or_default();
                    if let Some(n) = n {
                        let card_id = s.engine.state.players[HUMAN as usize].hand.get(n).cloned();
                        if let Some(cid) = card_id {
                            let target = resolve_target(&target_str, &s.engine, HUMAN);
                            match s.engine.play_card(HUMAN, &cid, target) {
                                Ok(evs) => log_events(&evs, &mut s.log),
                                Err(e) => s.log.push(format!("⚠ {e}")),
                            }
                        }
                    }
                    s.pending = Pending::None;
                }
                "attack_select" => {
                    if let Some(idx) = params.get("atk").and_then(|v| v.parse::<usize>().ok()) {
                        s.pending = Pending::AttackWith { board_idx: idx };
                    }
                }
                "attack_unit" => {
                    let atk_idx = params.get("atk").and_then(|v| v.parse::<usize>().ok());
                    let def_idx = params.get("def").and_then(|v| v.parse::<usize>().ok());
                    if let (Some(ai), Some(di)) = (atk_idx, def_idx) {
                        let atk_id = s.engine.state.players[HUMAN as usize].board.get(ai).map(|u| u.instance_id);
                        let def_id = s.engine.state.players[(HUMAN^1) as usize].board.get(di).map(|u| u.instance_id);
                        if let (Some(a), Some(d)) = (atk_id, def_id) {
                            match s.engine.attack_unit(HUMAN, a, d) {
                                Ok(evs) => log_events(&evs, &mut s.log),
                                Err(e) => s.log.push(format!("⚠ {e}")),
                            }
                        }
                    }
                    s.pending = Pending::None;
                }
                "attack_hero" => {
                    if let Some(ai) = params.get("atk").and_then(|v| v.parse::<usize>().ok()) {
                        let atk_id = s.engine.state.players[HUMAN as usize].board.get(ai).map(|u| u.instance_id);
                        if let Some(a) = atk_id {
                            match s.engine.attack_hero(HUMAN, a) {
                                Ok(evs) => log_events(&evs, &mut s.log),
                                Err(e) => s.log.push(format!("⚠ {e}")),
                            }
                        }
                    }
                    s.pending = Pending::None;
                }
                "power" => {
                    match s.engine.use_champion_power(HUMAN, None) {
                        Ok(evs) => log_events(&evs, &mut s.log),
                        Err(e) => s.log.push(format!("⚠ {e}")),
                    }
                    s.pending = Pending::None;
                }
                "power_select" => { s.pending = Pending::ChampionPower; }
                "power_targeted" => {
                    let target_str = params.get("target").cloned().unwrap_or_default();
                    let target = resolve_target(&target_str, &s.engine, HUMAN);
                    match s.engine.use_champion_power(HUMAN, target) {
                        Ok(evs) => log_events(&evs, &mut s.log),
                        Err(e) => s.log.push(format!("⚠ {e}")),
                    }
                    s.pending = Pending::None;
                }
                "cancel" => { s.pending = Pending::None; }
                _ => {}
            }
            drop(s);
            redirect(stream, "/");
        }
        _ => {
            send_response(stream, "404 Not Found", "text/plain", "Not found");
        }
    }
}

fn resolve_target(target_str: &str, engine: &GameEngine, human: PlayerId) -> Option<Uuid> {
    let enemy = human ^ 1;
    if let Some(rest) = target_str.strip_prefix('e') {
        if let Ok(n) = rest.parse::<usize>() {
            return engine.state.players[enemy as usize].board.get(n).map(|u| u.instance_id);
        }
    }
    if let Some(rest) = target_str.strip_prefix('f') {
        if let Ok(n) = rest.parse::<usize>() {
            return engine.state.players[human as usize].board.get(n).map(|u| u.instance_id);
        }
    }
    None
}

fn start_game(s: &mut WebState, p1_idx: usize, p2_idx: usize) {
    let p1_deck_def = s.available_decks.get(p1_idx).cloned()
        .unwrap_or_else(|| s.available_decks[0].clone());
    let p2_deck_def = s.available_decks.get(p2_idx).cloned()
        .unwrap_or_else(|| s.available_decks.last().unwrap().clone());

    s.deck_names = (p1_deck_def.name.clone(), p2_deck_def.name.clone());

    let mut rng = rand::rng();
    let mut p1_ids = p1_deck_def.card_ids.clone();
    let mut p2_ids = p2_deck_def.card_ids.clone();
    p1_ids.shuffle(&mut rng);
    p2_ids.shuffle(&mut rng);

    let d0 = DeckDefinition { id: p1_deck_def.id.clone(), name: p1_deck_def.name.clone(), card_ids: p1_ids };
    let d1 = DeckDefinition { id: p2_deck_def.id.clone(), name: p2_deck_def.name.clone(), card_ids: p2_ids };

    s.engine = GameEngine::new(s.registry.clone(), d0, d1);
    s.log.clear();
    s.pending = Pending::None;
    s.started = true;

    let evs = s.engine.begin_game();
    log_events(&evs, &mut s.log);
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let cards_path = cards_dir();
    let registry = match load_registry_from_dir(&cards_path) {
        Ok(r) => { println!("Loaded {} cards.", r.len()); r }
        Err(e) => { eprintln!("Error loading cards: {e}"); std::process::exit(1); }
    };

    let mut available_decks: Vec<DeckDefinition> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(decks_dir()) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(entry.path()) {
                    if let Ok(deck) = serde_json::from_str::<DeckDefinition>(&text) {
                        available_decks.push(deck);
                    }
                }
            }
        }
    }
    if available_decks.is_empty() { eprintln!("No decks found"); std::process::exit(1); }

    // Build initial game
    let mut rng = rand::rng();
    let mut p1_ids = available_decks[0].card_ids.clone();
    let mut p2_ids = available_decks.last().unwrap().card_ids.clone();
    p1_ids.shuffle(&mut rng);
    p2_ids.shuffle(&mut rng);

    let d0 = DeckDefinition { id: available_decks[0].id.clone(), name: available_decks[0].name.clone(), card_ids: p1_ids };
    let d1 = DeckDefinition { id: available_decks.last().unwrap().id.clone(), name: available_decks.last().unwrap().name.clone(), card_ids: p2_ids };

    let engine = GameEngine::new(registry.clone(), d0, d1);
    let deck_names = (available_decks[0].name.clone(), available_decks.last().unwrap().name.clone());

    let mut init_log = Vec::new();
    let mut engine = engine;
    let evs = engine.begin_game();
    log_events(&evs, &mut init_log);

    let web_state = Arc::new(Mutex::new(WebState {
        engine,
        registry,
        deck_names,
        log: init_log,
        pending: Pending::None,
        started: true,
        available_decks,
        p1_deck_choice: Some(0),
        p2_deck_choice: Some(1),
    }));

    let listener = TcpListener::bind("127.0.0.1:3030").expect("Failed to bind 127.0.0.1:3030");
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  Stonepyre TCG web UI → http://localhost:3030    ║");
    println!("║  Opening in your browser…                        ║");
    println!("╚══════════════════════════════════════════════════╝");

    // Try to open in browser
    let _ = std::process::Command::new("open").arg("http://localhost:3030").spawn();

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&web_state);
                std::thread::spawn(move || handle_connection(s, state));
            }
            Err(_) => {}
        }
    }
}

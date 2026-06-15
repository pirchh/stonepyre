/// Stonepyre TCG — standalone battle test binary.
///
/// Run from the `game/` workspace directory:
///   cargo run -p stonepyre_tcg --bin battle
///
/// Player 1 = you (human). Player 2 = simple AI.
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rand::seq::SliceRandom;
use uuid::Uuid;

use stonepyre_tcg::{
    deck::DeckDefinition, engine::{EngineError, GameEngine, GameEvent}, files::load_registry_from_dir,
    match_state::PlayerId, CardRegistry,
};

// ─── Paths ───────────────────────────────────────────────────────────────────

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/content/tcg/cards")
}

fn decks_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/content/tcg/decks")
}

// ─── Display helpers ─────────────────────────────────────────────────────────

const HR: &str = "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━";

fn print_events(events: &[GameEvent], registry: &CardRegistry) {
    for ev in events {
        match ev {
            GameEvent::TurnStarted { player, turn, mana } => {
                println!();
                println!("{HR}");
                println!("  TURN {turn}  ·  PLAYER {}  ·  {mana} MANA", player + 1);
                println!("{HR}");
            }
            GameEvent::TurnEnded { player } => {
                println!("  [END TURN] Player {} ends their turn.", player + 1);
            }
            GameEvent::CardDrawn { player, card_name } => {
                println!("  [DRAW] Player {} drew: {card_name}", player + 1);
            }
            GameEvent::DeckEmpty { player } => {
                println!("  [!] Player {}'s deck is empty!", player + 1);
            }
            GameEvent::CardPlayed { player, card_name, cost } => {
                println!("  [PLAY] Player {} plays {card_name} ({cost} mana)", player + 1);
            }
            GameEvent::UnitSummoned { player, card_name, attack, health, .. } => {
                println!("    ↳ {card_name} ({attack}/{health}) enters Player {}'s board.", player + 1);
            }
            GameEvent::TokenSummoned { player, name, attack, health, .. } => {
                println!("    ↳ Token: {name} ({attack}/{health}) summoned for Player {}.", player + 1);
            }
            GameEvent::RelicPlayed { player, card_name, durability } => {
                println!("    ↳ Relic: {card_name} [Dur:{durability}] placed by Player {}.", player + 1);
            }
            GameEvent::ChampionPlayed { player, card_name } => {
                println!("    ↳ Champion: {card_name} enters play for Player {}!", player + 1);
            }
            GameEvent::SpellCast { player, card_name } => {
                println!("    ↳ Spell: {card_name} cast by Player {}.", player + 1);
            }
            GameEvent::SpellEffectUnimplemented { player: _, card_name } => {
                println!("    [!] {card_name}: effect not yet implemented — no mechanical change.");
            }
            GameEvent::UnitAttackedUnit { attacker_name, defender_name, damage_dealt, damage_taken } => {
                println!("  [COMBAT] {attacker_name} → {defender_name} | dealt {damage_dealt}, took {damage_taken}");
            }
            GameEvent::UnitAttackedHero { attacker_name, target_player, damage } => {
                println!("  [ATTACK] {attacker_name} → Player {} hero for {damage} damage!", target_player + 1);
            }
            GameEvent::HeroDamaged { player, amount, remaining_health } => {
                println!("    ↳ Player {} takes {amount} damage. ({remaining_health} HP remaining)", player + 1);
            }
            GameEvent::HeroHealed { player, amount, new_health } => {
                println!("    ↳ Player {} restored {amount} HP. ({new_health} HP now)", player + 1);
            }
            GameEvent::UnitHealed { card_name, amount, new_health } => {
                println!("    ↳ {card_name} healed for {amount}. ({new_health} HP now)");
            }
            GameEvent::UnitBuffed { card_name, attack_delta, health_delta } => {
                let atk = if *attack_delta >= 0 { format!("+{}", attack_delta) } else { attack_delta.to_string() };
                let hp = if *health_delta >= 0 { format!("+{}", health_delta) } else { health_delta.to_string() };
                println!("    ↳ {card_name} buffed: {atk}/{hp}");
            }
            GameEvent::UnitDied { player, card_name } => {
                println!("    ✝  {card_name} (Player {}) died.", player + 1);
            }
            GameEvent::ShieldedBlocked { card_name } => {
                println!("    🛡  {card_name}'s Shielded absorbed the hit.");
            }
            GameEvent::ChampionPowerUsed { player, champion_name, description } => {
                println!("  [POWER] Player {} uses {champion_name}'s power: {description}", player + 1);
            }
            GameEvent::GameOver { winner } => {
                println!();
                println!("╔{HR}╗");
                println!("║           GAME OVER — Player {} wins!           ║", winner + 1);
                println!("╚{HR}╝");
            }
        }
    }
    let _ = registry; // available for future name lookups
}

fn print_state(engine: &GameEngine, registry: &CardRegistry, human_player: PlayerId) {
    let state = &engine.state;
    let p0 = &state.players[0];
    let p1 = &state.players[1];
    let active = state.active_player;

    println!();
    // Hero bars
    let hp_bar = |hp: i32, max: i32| -> String {
        let filled = ((hp.max(0) as f32 / max as f32) * 20.0) as usize;
        let empty = 20usize.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    };

    let p0_marker = if active == 0 { "►" } else { " " };
    let p1_marker = if active == 1 { "►" } else { " " };
    let p0_bar = hp_bar(p0.health, p0.max_health);
    let p1_bar = hp_bar(p1.health, p1.max_health);
    println!("{p0_marker} Player 1 {p0_bar} {}/{} HP  |  Hand: {}  Deck: {}",
        p0.health, p0.max_health, p0.hand.len(), p0.deck.len());
    println!("{p1_marker} Player 2 {p1_bar} {}/{} HP  |  Hand: {}  Deck: {}",
        p1.health, p1.max_health, p1.hand.len(), p1.deck.len());

    // Enemy board (from human's perspective, enemy = player 1)
    let enemy = human_player ^ 1;
    let your = human_player;
    println!();
    println!("── ENEMY BOARD (Player {}) ──", enemy + 1);
    if state.players[enemy as usize].board.is_empty() {
        println!("  (empty)");
    } else {
        for (i, u) in state.players[enemy as usize].board.iter().enumerate() {
            let kws = keyword_str(&u.keywords);
            let shield = if u.is_shielded { " 🛡" } else { "" };
            println!("  [{i}] {} ({}/{}){}  {}", u.display_name, u.current_attack, u.current_health, shield, kws);
        }
    }
    if let Some(champ) = &state.players[enemy as usize].champion_in_play {
        let atk = champ.current_attack.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string());
        let hp = champ.current_health.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string());
        println!("  ★ Champion: {} ({atk}/{hp})", champ.display_name);
    }
    if !state.players[enemy as usize].relics.is_empty() {
        let relics: Vec<String> = state.players[enemy as usize].relics.iter()
            .map(|r| format!("{} [{}]", r.display_name, r.remaining_durability))
            .collect();
        println!("  Relics: {}", relics.join(", "));
    }

    // Your board
    println!();
    println!("── YOUR BOARD (Player {}) ──", your + 1);
    if state.players[your as usize].board.is_empty() {
        println!("  (empty)");
    } else {
        for (i, u) in state.players[your as usize].board.iter().enumerate() {
            let kws = keyword_str(&u.keywords);
            let shield = if u.is_shielded { " 🛡" } else { "" };
            let can_atk = if u.can_attack() { " [CAN ATTACK]" } else { "" };
            println!("  [{i}] {} ({}/{}){}{}  {}", u.display_name, u.current_attack, u.current_health, shield, can_atk, kws);
        }
    }
    if let Some(champ) = &state.players[your as usize].champion_in_play {
        let atk = champ.current_attack.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string());
        let hp = champ.current_health.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string());
        println!("  ★ Champion: {} ({atk}/{hp})", champ.display_name);
    }
    if !state.players[your as usize].relics.is_empty() {
        let relics: Vec<String> = state.players[your as usize].relics.iter()
            .map(|r| format!("{} [{}]", r.display_name, r.remaining_durability))
            .collect();
        println!("  Relics: {}", relics.join(", "));
    }

    // Hand
    println!();
    let p = &state.players[your as usize];
    println!("── YOUR HAND  ({} mana available) ──", p.mana);
    if p.hand.is_empty() {
        println!("  (empty)");
    } else {
        for (i, cid) in p.hand.iter().enumerate() {
            if let Some(def) = registry.get(cid) {
                let type_label = match def.card_type {
                    stonepyre_tcg::CardType::Unit => {
                        let atk = def.attack.unwrap_or(0);
                        let hp = def.health.unwrap_or(0);
                        let kws = keyword_str(&def.keywords);
                        format!("Unit {atk}/{hp}  {kws}")
                    }
                    stonepyre_tcg::CardType::Spell => "Spell".to_string(),
                    stonepyre_tcg::CardType::Relic => format!("Relic Dur:{}", def.durability.unwrap_or(0)),
                    stonepyre_tcg::CardType::Champion => {
                        let atk = def.attack.unwrap_or(0);
                        let hp = def.health.unwrap_or(0);
                        format!("Champion {atk}/{hp}")
                    }
                };
                let affordable = if p.mana >= def.cost { "" } else { " (too costly)" };
                println!("  [{i}] ({}) {}  — {type_label}  ·  {}{}", def.cost, def.name, def.rules_text, affordable);
            }
        }
    }

    // Champion power
    if let Some(champ) = &state.players[your as usize].champion_in_play {
        if let Some(def) = registry.get(&champ.card_def_id) {
            if let Some(power) = &def.champion_power {
                let used = if state.players[your as usize].champion_power_used { " [USED]" } else { " [READY]" };
                let affordable = if p.mana >= power.cost { "" } else { " (need more mana)" };
                println!();
                println!("  ★ Champion Power ({} mana): {}{}{}", power.cost, power.description, used, affordable);
            }
        }
    }
}

fn keyword_str(kws: &[stonepyre_tcg::Keyword]) -> String {
    if kws.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = kws.iter().map(|k| k.display_name()).collect();
        names.join(", ")
    }
}

fn print_help() {
    println!();
    println!("Commands:");
    println!("  play <n>               — play card at hand index n (unit/relic/champion)");
    println!("  play <n> e<m>          — play targeted spell/power at hand[n] → enemy board[m]");
    println!("  play <n> f<m>          — play targeted spell at hand[n] → your board[m]");
    println!("  play <n> hero          — play targeted spell at hand[n] → enemy hero");
    println!("  attack <n> <m>         — your board[n] attacks enemy board[m]");
    println!("  attack <n> hero        — your board[n] attacks enemy hero");
    println!("  power                  — use champion power (no target)");
    println!("  power e<m>             — use champion power → enemy board[m]");
    println!("  power f<m>             — use champion power → your board[m]");
    println!("  end                    — end your turn");
    println!("  state                  — redraw board state");
    println!("  quit / exit            — exit battle");
}

// ─── AI ──────────────────────────────────────────────────────────────────────

fn ai_turn(engine: &mut GameEngine, ai_pid: PlayerId, registry: &CardRegistry) {
    println!();
    println!("  [AI] Player {}'s turn…", ai_pid + 1);
    let opponent = ai_pid ^ 1;

    // Play cards: greedy — cheapest first, prefer units > relics > spells
    loop {
        let p = &engine.state.players[ai_pid as usize];
        let mana = p.mana;

        // find affordable card — sort by cost ascending
        let mut playable: Vec<(usize, u8)> = p.hand.iter().enumerate()
            .filter_map(|(i, cid)| {
                registry.get(cid).filter(|d| d.cost <= mana).map(|d| (i, d.cost))
            })
            .collect();
        playable.sort_by_key(|&(_, cost)| cost);

        let Some((idx, _)) = playable.first().copied() else { break };

        let card_id = engine.state.players[ai_pid as usize].hand[idx].clone();
        let def = registry.get(&card_id).unwrap().clone();

        // Determine target for spells
        let target_unit: Option<Uuid> = if def.is_spell() {
            if let Some(fx) = &def.spell_effect {
                if fx.targeting.requires_target() {
                    match fx.targeting {
                        stonepyre_tcg::TargetingRule::EnemyUnit |
                        stonepyre_tcg::TargetingRule::AnyUnit => {
                            engine.state.players[opponent as usize].board.first().map(|u| u.instance_id)
                        }
                        stonepyre_tcg::TargetingRule::FriendlyUnit => {
                            // heal the most damaged unit
                            engine.state.players[ai_pid as usize].board.iter()
                                .min_by_key(|u| u.current_health)
                                .map(|u| u.instance_id)
                        }
                        _ => None,
                    }
                } else { None }
            } else { None }
        } else { None };

        match engine.play_card(ai_pid, &card_id, target_unit) {
            Ok(events) => print_events(&events, registry),
            Err(e) => { println!("  [AI] play error: {e}"); break; }
        }

        if engine.is_over() { return; }
    }

    // Use champion power if available
    if let Some(champ) = &engine.state.players[ai_pid as usize].champion_in_play.clone() {
        let def = registry.get(&champ.card_def_id).cloned();
        if let Some(def) = def {
            if let Some(power) = &def.champion_power {
                let mana = engine.state.players[ai_pid as usize].mana;
                if mana >= power.cost && !engine.state.players[ai_pid as usize].champion_power_used {
                    // pick a target
                    let target: Option<Uuid> = if power.targeting.requires_target() {
                        match power.targeting {
                            stonepyre_tcg::TargetingRule::EnemyUnit |
                            stonepyre_tcg::TargetingRule::AnyUnit => {
                                engine.state.players[opponent as usize].board.first().map(|u| u.instance_id)
                            }
                            stonepyre_tcg::TargetingRule::FriendlyUnit => {
                                engine.state.players[ai_pid as usize].board.iter()
                                    .min_by_key(|u| u.current_health)
                                    .map(|u| u.instance_id)
                            }
                            _ => None,
                        }
                    } else { None };

                    match engine.use_champion_power(ai_pid, target) {
                        Ok(events) => print_events(&events, registry),
                        Err(_) => {}
                    }
                    if engine.is_over() { return; }
                }
            }
        }
    }

    // Attack with all ready units
    loop {
        let attackers: Vec<Uuid> = engine.state.players[ai_pid as usize].board.iter()
            .filter(|u| u.can_attack())
            .map(|u| u.instance_id)
            .collect();

        let Some(attacker_id) = attackers.first().copied() else { break };

        // Prefer guard targets, then attack hero if no guards
        let guard_target = engine.state.players[opponent as usize].board.iter()
            .find(|u| u.has_guard()).map(|u| u.instance_id);

        let result = if let Some(gid) = guard_target {
            engine.attack_unit(ai_pid, attacker_id, gid)
        } else if let Some(first_enemy) = engine.state.players[opponent as usize].board.first().map(|u| u.instance_id) {
            // Attack weakest enemy unit or go face if board is clear
            let weakest = engine.state.players[opponent as usize].board.iter()
                .min_by_key(|u| u.current_health).map(|u| u.instance_id)
                .unwrap_or(first_enemy);
            engine.attack_unit(ai_pid, attacker_id, weakest)
        } else {
            engine.attack_hero(ai_pid, attacker_id)
        };

        match result {
            Ok(events) => print_events(&events, registry),
            Err(e) => { println!("  [AI] attack error: {e}"); break; }
        }
        if engine.is_over() { return; }
    }

    // End turn
    match engine.end_turn(ai_pid) {
        Ok(events) => print_events(&events, registry),
        Err(e) => println!("  [AI] end_turn error: {e}"),
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          STONEPYRE TCG  ·  BATTLE TEST MODE              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Load card registry
    let cards_path = cards_dir();
    print!("Loading cards from {}… ", cards_path.display());
    let _ = io::stdout().flush();
    let registry = match load_registry_from_dir(&cards_path) {
        Ok(r) => { println!("{} cards loaded.", r.len()); r }
        Err(e) => { eprintln!("Error: {e}"); std::process::exit(1); }
    };

    // Load available decks
    let decks_path = decks_dir();
    let mut available_decks: Vec<DeckDefinition> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&decks_path) {
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
    if available_decks.is_empty() {
        eprintln!("No decks found in {}", decks_path.display());
        std::process::exit(1);
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    // Deck selection
    println!("Available decks:");
    for (i, d) in available_decks.iter().enumerate() {
        println!("  [{}] {} — {} cards", i + 1, d.name, d.card_ids.len());
    }

    let pick_deck = |prompt_str: &str, lines: &mut dyn Iterator<Item = io::Result<String>>, decks: &[DeckDefinition]| -> DeckDefinition {
        loop {
            print!("{prompt_str}");
            let _ = io::stdout().flush();
            let input = lines.next().unwrap_or(Ok(String::new())).unwrap_or_default().trim().to_string();
            if let Ok(n) = input.parse::<usize>() {
                if n >= 1 && n <= decks.len() {
                    return decks[n - 1].clone();
                }
            }
            println!("Invalid choice. Enter 1–{}", decks.len());
        }
    };

    let your_deck = pick_deck("Your deck (Player 1): ", &mut lines, &available_decks);
    let ai_deck = pick_deck("AI deck (Player 2): ", &mut lines, &available_decks);

    // Shuffle decks
    let mut rng = rand::rng();
    let mut p1_ids = your_deck.card_ids.clone();
    let mut p2_ids = ai_deck.card_ids.clone();
    p1_ids.shuffle(&mut rng);
    p2_ids.shuffle(&mut rng);

    let deck0 = DeckDefinition { id: your_deck.id.clone(), name: your_deck.name.clone(), card_ids: p1_ids };
    let deck1 = DeckDefinition { id: ai_deck.id.clone(), name: ai_deck.name.clone(), card_ids: p2_ids };

    println!();
    println!("Player 1: {}  vs  Player 2 (AI): {}", your_deck.name, ai_deck.name);
    println!("{HR}");

    let mut engine = GameEngine::new(registry.clone(), deck0, deck1);
    let events = engine.begin_game();
    print_events(&events, &registry);

    const HUMAN: PlayerId = 0;
    const AI: PlayerId = 1;

    // Main game loop
    loop {
        if engine.is_over() {
            break;
        }

        if engine.state.active_player == AI {
            ai_turn(&mut engine, AI, &registry);
            continue;
        }

        // Human turn
        print_state(&engine, &registry, HUMAN);
        println!();
        print!("> ");
        let _ = io::stdout().flush();

        let input = match lines.next() {
            Some(Ok(line)) => line.trim().to_string(),
            _ => break,
        };

        if input.is_empty() { continue; }
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.as_slice() {
            ["quit"] | ["exit"] => {
                println!("Exiting battle.");
                break;
            }
            ["help"] => print_help(),
            ["state"] => { /* will redraw on next loop iteration */ }
            ["end"] => {
                match engine.end_turn(HUMAN) {
                    Ok(events) => print_events(&events, &registry),
                    Err(e) => println!("  Error: {e}"),
                }
            }
            ["play", n_str] => {
                if let Ok(n) = n_str.parse::<usize>() {
                    let hand = &engine.state.players[HUMAN as usize].hand;
                    if n >= hand.len() {
                        println!("  No card at index {n}.");
                        continue;
                    }
                    let card_id = hand[n].clone();
                    match engine.play_card(HUMAN, &card_id, None) {
                        Ok(events) => print_events(&events, &registry),
                        Err(EngineError::TargetRequired) => println!("  This card requires a target. Use: play {n} e<m>, play {n} f<m>, or play {n} hero"),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else {
                    println!("  Usage: play <hand_index>");
                }
            }
            ["play", n_str, target_str] => {
                if let Ok(n) = n_str.parse::<usize>() {
                    let hand = &engine.state.players[HUMAN as usize].hand;
                    if n >= hand.len() { println!("  No card at index {n}."); continue; }
                    let card_id = hand[n].clone();
                    let target = parse_target(target_str, &engine, HUMAN);
                    match engine.play_card(HUMAN, &card_id, target) {
                        Ok(events) => print_events(&events, &registry),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else {
                    println!("  Usage: play <hand_index> [e<n>|f<n>|hero]");
                }
            }
            ["attack", atk_str, def_str] => {
                let attacker_id = match atk_str.parse::<usize>() {
                    Ok(n) => engine.state.players[HUMAN as usize].board.get(n).map(|u| u.instance_id),
                    Err(_) => None,
                };
                let Some(atk_id) = attacker_id else {
                    println!("  No attacker at that index.");
                    continue;
                };
                if *def_str == "hero" {
                    match engine.attack_hero(HUMAN, atk_id) {
                        Ok(events) => print_events(&events, &registry),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else if let Ok(m) = def_str.parse::<usize>() {
                    let enemy = HUMAN ^ 1;
                    let def_id = engine.state.players[enemy as usize].board.get(m).map(|u| u.instance_id);
                    let Some(did) = def_id else {
                        println!("  No enemy unit at index {m}.");
                        continue;
                    };
                    match engine.attack_unit(HUMAN, atk_id, did) {
                        Ok(events) => print_events(&events, &registry),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else {
                    println!("  Usage: attack <your_board_index> <enemy_board_index|hero>");
                }
            }
            ["power"] => {
                match engine.use_champion_power(HUMAN, None) {
                    Ok(events) => print_events(&events, &registry),
                    Err(EngineError::TargetRequired) => println!("  This power requires a target. Use: power e<m> or power f<m>"),
                    Err(e) => println!("  Error: {e}"),
                }
            }
            ["power", target_str] => {
                let target = parse_target(target_str, &engine, HUMAN);
                match engine.use_champion_power(HUMAN, target) {
                    Ok(events) => print_events(&events, &registry),
                    Err(e) => println!("  Error: {e}"),
                }
            }
            // Bare number shortcut: "2" → "play 2"
            [n_str] if n_str.parse::<usize>().is_ok() => {
                let n = n_str.parse::<usize>().unwrap();
                let hand = &engine.state.players[HUMAN as usize].hand;
                if n >= hand.len() { println!("  No card at index {n}."); continue; }
                let card_id = hand[n].clone();
                match engine.play_card(HUMAN, &card_id, None) {
                    Ok(events) => print_events(&events, &registry),
                    Err(EngineError::TargetRequired) => println!("  This card needs a target. Use: play {n} e<m>, play {n} f<m>"),
                    Err(e) => println!("  Error: {e}"),
                }
            }
            _ => println!("  Unknown command. Type 'help' for the command list."),
        }

        if engine.is_over() { break; }
    }

    if !engine.is_over() {
        println!("\nBattle ended early.");
    }
}

// ─── Target parsing ──────────────────────────────────────────────────────────

/// Parse a target string:
/// - `e0`, `e1`, … → enemy board unit at index N
/// - `f0`, `f1`, … → friendly board unit at index N
/// - `hero`         → treated as no-UUID target (caller handles hero logic separately)
fn parse_target(s: &str, engine: &GameEngine, human: PlayerId) -> Option<Uuid> {
    let enemy = human ^ 1;
    if let Some(rest) = s.strip_prefix('e') {
        if let Ok(n) = rest.parse::<usize>() {
            return engine.state.players[enemy as usize].board.get(n).map(|u| u.instance_id);
        }
    }
    if let Some(rest) = s.strip_prefix('f') {
        if let Ok(n) = rest.parse::<usize>() {
            return engine.state.players[human as usize].board.get(n).map(|u| u.instance_id);
        }
    }
    None // "hero" and unknown → None (engine handles no-UUID for hero spells differently)
}

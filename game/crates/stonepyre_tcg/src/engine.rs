use std::collections::VecDeque;
use uuid::Uuid;
use crate::card::CardType;
use crate::deck::{DeckDefinition, STARTING_HAND_SIZE};
use crate::effects::{EffectDefinition, TargetingRule};
use crate::keyword::Keyword;
use crate::match_state::{ChampionInPlay, MatchState, PlayerId, RelicInPlay, UnitInPlay};
use crate::registry::CardRegistry;

// ─── Events ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GameEvent {
    TurnStarted { player: PlayerId, turn: u32, mana: u8 },
    CardDrawn { player: PlayerId, card_name: String },
    DeckEmpty { player: PlayerId },
    CardPlayed { player: PlayerId, card_name: String, cost: u8 },
    UnitSummoned { player: PlayerId, instance_id: Uuid, card_name: String, attack: i16, health: i16 },
    TokenSummoned { player: PlayerId, instance_id: Uuid, name: String, attack: i16, health: i16 },
    RelicPlayed { player: PlayerId, card_name: String, durability: u8 },
    ChampionPlayed { player: PlayerId, card_name: String },
    SpellCast { player: PlayerId, card_name: String },
    SpellEffectUnimplemented { player: PlayerId, card_name: String },
    UnitAttackedUnit {
        attacker_name: String,
        defender_name: String,
        damage_dealt: i16,
        damage_taken: i16,
    },
    UnitAttackedHero { attacker_name: String, target_player: PlayerId, damage: i16 },
    HeroDamaged { player: PlayerId, amount: i16, remaining_health: i32 },
    HeroHealed { player: PlayerId, amount: i32, new_health: i32 },
    UnitHealed { card_name: String, amount: i16, new_health: i16 },
    UnitBuffed { card_name: String, attack_delta: i16, health_delta: i16 },
    UnitDied { player: PlayerId, card_name: String },
    ShieldedBlocked { card_name: String },
    ChampionPowerUsed { player: PlayerId, champion_name: String, description: String },
    TurnEnded { player: PlayerId },
    GameOver { winner: PlayerId },
}

// ─── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("card '{0}' not found in registry")]
    CardNotFound(String),
    #[error("card '{0}' is not in your hand")]
    CardNotInHand(String),
    #[error("not enough mana: need {need}, have {have}")]
    NotEnoughMana { need: u8, have: u8 },
    #[error("no unit with that ID on the board")]
    UnitNotFound,
    #[error("this unit cannot attack this turn (already attacked or just played without Charge)")]
    CannotAttack,
    #[error("an enemy unit with Guard must be attacked first")]
    MustTargetGuard,
    #[error("champion power already used this turn")]
    PowerAlreadyUsed,
    #[error("no champion in play")]
    NoChampion,
    #[error("game is already over")]
    GameAlreadyOver,
    #[error("it is not player {0}'s turn")]
    WrongTurn(PlayerId),
    #[error("this action requires a target")]
    TargetRequired,
}

// ─── Engine ──────────────────────────────────────────────────────────────────

pub struct GameEngine {
    pub state: MatchState,
    pub registry: CardRegistry,
    events: Vec<GameEvent>,
}

impl GameEngine {
    pub fn new(registry: CardRegistry, deck0: DeckDefinition, deck1: DeckDefinition) -> Self {
        let d0: VecDeque<String> = deck0.card_ids.into();
        let d1: VecDeque<String> = deck1.card_ids.into();
        Self {
            state: MatchState::new(d0, d1),
            registry,
            events: Vec::new(),
        }
    }

    pub fn winner(&self) -> Option<PlayerId> {
        self.state.players.iter().find(|p| p.is_defeated()).map(|p| p.id ^ 1)
    }

    pub fn is_over(&self) -> bool {
        self.winner().is_some()
    }

    // ── Setup ────────────────────────────────────────────────────────────────

    /// Deal opening hands for both players and fire TurnStarted for Player 0.
    pub fn begin_game(&mut self) -> Vec<GameEvent> {
        self.events.clear();
        for pid in 0u8..2 {
            for _ in 0..STARTING_HAND_SIZE {
                self.draw_card(pid);
            }
        }
        // Start Player 0's first turn
        self.state.players[0].turns_taken = 1;
        let mana = MatchState::mana_for_player_turn(1);
        self.state.players[0].mana = mana;
        self.state.players[0].max_mana = mana;
        self.draw_card(0);
        self.emit(GameEvent::TurnStarted { player: 0, turn: 1, mana });
        std::mem::take(&mut self.events)
    }

    // ── Actions ──────────────────────────────────────────────────────────────

    /// Play a card from hand. `target_unit` is required for targeted spells/powers.
    pub fn play_card(
        &mut self,
        player_id: PlayerId,
        card_id: &str,
        target_unit: Option<Uuid>,
    ) -> Result<Vec<GameEvent>, EngineError> {
        self.guard(player_id)?;

        let hand = &self.state.players[player_id as usize].hand;
        if !hand.contains(&card_id.to_string()) {
            return Err(EngineError::CardNotInHand(card_id.to_string()));
        }

        let def = self.registry.get(card_id)
            .ok_or_else(|| EngineError::CardNotFound(card_id.to_string()))?
            .clone();

        let mana = self.state.players[player_id as usize].mana;
        if mana < def.cost {
            return Err(EngineError::NotEnoughMana { need: def.cost, have: mana });
        }

        self.events.clear();

        // Remove from hand, spend mana
        self.state.players[player_id as usize].hand.retain(|id| id != card_id);
        self.state.players[player_id as usize].mana -= def.cost;

        self.emit(GameEvent::CardPlayed {
            player: player_id,
            card_name: def.name.clone(),
            cost: def.cost,
        });

        match def.card_type {
            CardType::Unit => self.summon_unit(player_id, &def),
            CardType::Champion => self.play_champion(player_id, &def),
            CardType::Relic => self.place_relic(player_id, &def),
            CardType::Spell => self.cast_spell(player_id, &def, target_unit),
        }

        self.check_game_over();
        Ok(std::mem::take(&mut self.events))
    }

    pub fn attack_unit(
        &mut self,
        player_id: PlayerId,
        attacker_id: Uuid,
        defender_id: Uuid,
    ) -> Result<Vec<GameEvent>, EngineError> {
        self.guard(player_id)?;

        let opponent_id = player_id ^ 1;

        // Validate attacker
        let attacker = self.state.players[player_id as usize]
            .board.iter().find(|u| u.instance_id == attacker_id)
            .ok_or(EngineError::UnitNotFound)?;
        if !attacker.can_attack() {
            return Err(EngineError::CannotAttack);
        }

        // Guard requirement
        if self.state.players[opponent_id as usize].has_guard_unit() {
            let target_has_guard = self.state.players[opponent_id as usize]
                .board.iter()
                .find(|u| u.instance_id == defender_id)
                .map(|u| u.has_guard())
                .unwrap_or(false);
            if !target_has_guard {
                return Err(EngineError::MustTargetGuard);
            }
        }

        if !self.state.players[opponent_id as usize].board.iter().any(|u| u.instance_id == defender_id) {
            return Err(EngineError::UnitNotFound);
        }

        // Snapshot combat values before mutating
        let atk_attack = self.find_unit(player_id, attacker_id).current_attack;
        let atk_has_drain = self.find_unit(player_id, attacker_id).keywords.contains(&Keyword::Drain);
        let atk_shielded = self.find_unit(player_id, attacker_id).is_shielded;
        let atk_name = self.find_unit(player_id, attacker_id).display_name.clone();

        let def_attack = self.find_unit(opponent_id, defender_id).current_attack;
        let def_has_thorns = self.find_unit(opponent_id, defender_id).keywords.contains(&Keyword::Thorns);
        let def_shielded = self.find_unit(opponent_id, defender_id).is_shielded;
        let def_name = self.find_unit(opponent_id, defender_id).display_name.clone();

        self.events.clear();

        // Calculate damage after shield checks
        let dmg_to_defender = if def_shielded {
            self.find_unit_mut(opponent_id, defender_id).is_shielded = false;
            self.emit(GameEvent::ShieldedBlocked { card_name: def_name.clone() });
            0
        } else {
            atk_attack
        };

        let dmg_to_attacker = if atk_shielded {
            self.find_unit_mut(player_id, attacker_id).is_shielded = false;
            self.emit(GameEvent::ShieldedBlocked { card_name: atk_name.clone() });
            0
        } else {
            def_attack + if def_has_thorns { 1 } else { 0 }
        };

        // Apply damage
        if dmg_to_defender > 0 {
            self.find_unit_mut(opponent_id, defender_id).current_health -= dmg_to_defender;
        }
        if dmg_to_attacker > 0 {
            self.find_unit_mut(player_id, attacker_id).current_health -= dmg_to_attacker;
        }

        // Mark attacked
        self.find_unit_mut(player_id, attacker_id).has_attacked = true;

        // Drain heal
        if atk_has_drain && dmg_to_defender > 0 {
            self.heal_hero(player_id, dmg_to_defender as i32);
        }

        self.emit(GameEvent::UnitAttackedUnit {
            attacker_name: atk_name.clone(),
            defender_name: def_name.clone(),
            damage_dealt: dmg_to_defender,
            damage_taken: dmg_to_attacker,
        });

        // Remove dead units
        self.remove_dead(player_id);
        self.remove_dead(opponent_id);

        self.check_game_over();
        Ok(std::mem::take(&mut self.events))
    }

    pub fn attack_hero(
        &mut self,
        player_id: PlayerId,
        attacker_id: Uuid,
    ) -> Result<Vec<GameEvent>, EngineError> {
        self.guard(player_id)?;

        let opponent_id = player_id ^ 1;

        if self.state.players[opponent_id as usize].has_guard_unit() {
            return Err(EngineError::MustTargetGuard);
        }

        let attacker = self.state.players[player_id as usize]
            .board.iter().find(|u| u.instance_id == attacker_id)
            .ok_or(EngineError::UnitNotFound)?;
        if !attacker.can_attack() {
            return Err(EngineError::CannotAttack);
        }

        let damage = attacker.current_attack;
        let has_drain = attacker.keywords.contains(&Keyword::Drain);
        let atk_name = attacker.display_name.clone();

        self.events.clear();

        self.state.players[opponent_id as usize].health -= damage as i32;
        let remaining = self.state.players[opponent_id as usize].health;

        self.find_unit_mut(player_id, attacker_id).has_attacked = true;

        self.emit(GameEvent::UnitAttackedHero {
            attacker_name: atk_name,
            target_player: opponent_id,
            damage,
        });
        self.emit(GameEvent::HeroDamaged { player: opponent_id, amount: damage, remaining_health: remaining });

        if has_drain && damage > 0 {
            self.heal_hero(player_id, damage as i32);
        }

        self.check_game_over();
        Ok(std::mem::take(&mut self.events))
    }

    pub fn use_champion_power(
        &mut self,
        player_id: PlayerId,
        target_unit: Option<Uuid>,
    ) -> Result<Vec<GameEvent>, EngineError> {
        self.guard(player_id)?;

        let p = &self.state.players[player_id as usize];
        if p.champion_power_used {
            return Err(EngineError::PowerAlreadyUsed);
        }

        let champ_def_id = p.champion_in_play.as_ref()
            .ok_or(EngineError::NoChampion)?
            .card_def_id.clone();

        let def = self.registry.get(&champ_def_id)
            .ok_or_else(|| EngineError::CardNotFound(champ_def_id.clone()))?
            .clone();

        let power = def.champion_power.as_ref().ok_or(EngineError::NoChampion)?.clone();

        if p.mana < power.cost {
            return Err(EngineError::NotEnoughMana { need: power.cost, have: p.mana });
        }

        if power.targeting.requires_target() && target_unit.is_none() {
            return Err(EngineError::TargetRequired);
        }

        self.events.clear();

        self.state.players[player_id as usize].mana -= power.cost;
        self.state.players[player_id as usize].champion_power_used = true;

        self.emit(GameEvent::ChampionPowerUsed {
            player: player_id,
            champion_name: def.name.clone(),
            description: power.description.clone(),
        });

        let effect = power.effect.clone();
        let targeting = power.targeting.clone();
        let opponent_id = player_id ^ 1;

        match targeting {
            TargetingRule::AllEnemyUnits => {
                let targets: Vec<Uuid> = self.state.players[opponent_id as usize]
                    .board.iter().map(|u| u.instance_id).collect();
                for t in targets {
                    self.apply_effect_to_unit(opponent_id, t, &effect.clone());
                }
            }
            TargetingRule::AllFriendlyUnits => {
                let targets: Vec<Uuid> = self.state.players[player_id as usize]
                    .board.iter().map(|u| u.instance_id).collect();
                for t in targets {
                    self.apply_effect_to_unit(player_id, t, &effect.clone());
                }
            }
            TargetingRule::NoTarget => {
                self.apply_effect_no_target(player_id, &effect);
            }
            TargetingRule::FriendlyUnit => {
                if let Some(tid) = target_unit {
                    self.apply_effect_to_unit(player_id, tid, &effect);
                }
            }
            TargetingRule::EnemyUnit => {
                if let Some(tid) = target_unit {
                    self.apply_effect_to_unit(opponent_id, tid, &effect);
                }
            }
            TargetingRule::AnyUnit | TargetingRule::AnyCharacter => {
                if let Some(tid) = target_unit {
                    // figure out which side owns it
                    let target_player = if self.state.players[player_id as usize].board.iter().any(|u| u.instance_id == tid) {
                        player_id
                    } else {
                        opponent_id
                    };
                    self.apply_effect_to_unit(target_player, tid, &effect);
                }
            }
            TargetingRule::FriendlyHero => {
                self.apply_effect_hero(player_id, &effect);
            }
            TargetingRule::EnemyHero => {
                self.apply_effect_hero(opponent_id, &effect);
            }
        }

        self.check_game_over();
        Ok(std::mem::take(&mut self.events))
    }

    pub fn end_turn(&mut self, player_id: PlayerId) -> Result<Vec<GameEvent>, EngineError> {
        self.guard(player_id)?;

        self.events.clear();
        self.emit(GameEvent::TurnEnded { player: player_id });

        self.state.active_player ^= 1;
        self.state.turn += 1;

        let next = self.state.active_player;
        let turn = self.state.turn;

        // Reset combat flags for incoming player's board
        for unit in &mut self.state.players[next as usize].board {
            unit.played_this_turn = false;
            unit.has_attacked = false;
        }
        self.state.players[next as usize].champion_power_used = false;

        // Increment this player's personal turn counter and set mana
        self.state.players[next as usize].turns_taken += 1;
        let new_mana = MatchState::mana_for_player_turn(self.state.players[next as usize].turns_taken);
        self.state.players[next as usize].max_mana = new_mana;
        self.state.players[next as usize].mana = new_mana;

        // Draw
        self.draw_card(next);

        self.emit(GameEvent::TurnStarted { player: next, turn, mana: new_mana });

        Ok(std::mem::take(&mut self.events))
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn guard(&self, player_id: PlayerId) -> Result<(), EngineError> {
        if self.is_over() {
            return Err(EngineError::GameAlreadyOver);
        }
        if self.state.active_player != player_id {
            return Err(EngineError::WrongTurn(player_id));
        }
        Ok(())
    }

    fn draw_card(&mut self, pid: PlayerId) {
        if let Some(card_id) = self.state.players[pid as usize].deck.pop_front() {
            let name = self.registry.get(&card_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| card_id.clone());
            self.state.players[pid as usize].hand.push(card_id);
            self.emit(GameEvent::CardDrawn { player: pid, card_name: name });
        } else {
            self.emit(GameEvent::DeckEmpty { player: pid });
        }
    }

    fn summon_unit(&mut self, pid: PlayerId, def: &crate::card::CardDefinition) {
        let attack = def.attack.unwrap_or(0);
        let health = def.health.unwrap_or(1);
        let instance_id = Uuid::new_v4();
        let unit = UnitInPlay {
            instance_id,
            card_def_id: def.id.clone(),
            display_name: def.name.clone(),
            current_attack: attack,
            current_health: health,
            max_health: health,
            keywords: def.keywords.clone(),
            played_this_turn: true,
            has_attacked: false,
            is_shielded: def.keywords.contains(&Keyword::Shielded),
        };
        self.state.players[pid as usize].board.push(unit);
        self.emit(GameEvent::UnitSummoned { player: pid, instance_id, card_name: def.name.clone(), attack, health });
    }

    fn play_champion(&mut self, pid: PlayerId, def: &crate::card::CardDefinition) {
        let instance_id = Uuid::new_v4();
        let champ = ChampionInPlay {
            instance_id,
            card_def_id: def.id.clone(),
            display_name: def.name.clone(),
            current_attack: def.attack,
            current_health: def.health,
            max_health: def.health,
        };
        self.state.players[pid as usize].champion_in_play = Some(champ);
        self.emit(GameEvent::ChampionPlayed { player: pid, card_name: def.name.clone() });
    }

    fn place_relic(&mut self, pid: PlayerId, def: &crate::card::CardDefinition) {
        let instance_id = Uuid::new_v4();
        let durability = def.durability.unwrap_or(0);
        let relic = RelicInPlay {
            instance_id,
            card_def_id: def.id.clone(),
            display_name: def.name.clone(),
            remaining_durability: durability,
        };
        self.state.players[pid as usize].relics.push(relic);
        self.emit(GameEvent::RelicPlayed { player: pid, card_name: def.name.clone(), durability });
    }

    fn cast_spell(&mut self, pid: PlayerId, def: &crate::card::CardDefinition, target_unit: Option<Uuid>) {
        self.emit(GameEvent::SpellCast { player: pid, card_name: def.name.clone() });
        let opponent_id = pid ^ 1;

        if let Some(spell_fx) = def.spell_effect.clone() {
            let effect = spell_fx.effect.clone();
            match spell_fx.targeting {
                TargetingRule::AllEnemyUnits => {
                    let targets: Vec<Uuid> = self.state.players[opponent_id as usize]
                        .board.iter().map(|u| u.instance_id).collect();
                    for t in targets {
                        self.apply_effect_to_unit(opponent_id, t, &effect.clone());
                    }
                }
                TargetingRule::AllFriendlyUnits => {
                    let targets: Vec<Uuid> = self.state.players[pid as usize]
                        .board.iter().map(|u| u.instance_id).collect();
                    for t in targets {
                        self.apply_effect_to_unit(pid, t, &effect.clone());
                    }
                }
                TargetingRule::NoTarget => self.apply_effect_no_target(pid, &effect),
                TargetingRule::FriendlyUnit => {
                    if let Some(tid) = target_unit {
                        self.apply_effect_to_unit(pid, tid, &effect);
                    }
                }
                TargetingRule::EnemyUnit => {
                    if let Some(tid) = target_unit {
                        self.apply_effect_to_unit(opponent_id, tid, &effect);
                    }
                }
                TargetingRule::AnyUnit | TargetingRule::AnyCharacter => {
                    if let Some(tid) = target_unit {
                        let tp = if self.state.players[pid as usize].board.iter().any(|u| u.instance_id == tid) {
                            pid
                        } else {
                            opponent_id
                        };
                        self.apply_effect_to_unit(tp, tid, &effect);
                    }
                }
                TargetingRule::FriendlyHero => self.apply_effect_hero(pid, &effect),
                TargetingRule::EnemyHero => self.apply_effect_hero(opponent_id, &effect),
            }
        } else {
            self.emit(GameEvent::SpellEffectUnimplemented { player: pid, card_name: def.name.clone() });
        }

        self.state.players[pid as usize].discard.push(def.id.clone());
    }

    fn apply_effect_to_unit(&mut self, target_player: PlayerId, target_id: Uuid, effect: &EffectDefinition) {
        match effect {
            EffectDefinition::DealDamage { amount } => {
                let amt = *amount as i16;
                if let Some(u) = self.state.players[target_player as usize].board.iter_mut().find(|u| u.instance_id == target_id) {
                    if u.is_shielded {
                        u.is_shielded = false;
                        let name = u.display_name.clone();
                        self.emit(GameEvent::ShieldedBlocked { card_name: name });
                    } else {
                        u.current_health -= amt;
                    }
                }
                self.remove_dead(target_player);
            }
            EffectDefinition::Heal { amount } => {
                let amt = *amount as i16;
                if let Some(u) = self.state.players[target_player as usize].board.iter_mut().find(|u| u.instance_id == target_id) {
                    let before = u.current_health;
                    u.current_health = (u.current_health + amt).min(u.max_health);
                    let healed = u.current_health - before;
                    let name = u.display_name.clone();
                    let new_hp = u.current_health;
                    if healed > 0 {
                        self.emit(GameEvent::UnitHealed { card_name: name, amount: healed, new_health: new_hp });
                    }
                }
            }
            EffectDefinition::BuffUnit { attack_delta, health_delta } => {
                if let Some(u) = self.state.players[target_player as usize].board.iter_mut().find(|u| u.instance_id == target_id) {
                    u.current_attack += attack_delta;
                    u.current_health += health_delta;
                    u.max_health += health_delta;
                    let name = u.display_name.clone();
                    self.emit(GameEvent::UnitBuffed { card_name: name, attack_delta: *attack_delta, health_delta: *health_delta });
                }
            }
            EffectDefinition::GiveShielded => {
                if let Some(u) = self.state.players[target_player as usize].board.iter_mut().find(|u| u.instance_id == target_id) {
                    u.is_shielded = true;
                }
            }
            EffectDefinition::GiveKeyword { keyword } => {
                if let Some(u) = self.state.players[target_player as usize].board.iter_mut().find(|u| u.instance_id == target_id) {
                    if !u.keywords.contains(keyword) {
                        u.keywords.push(keyword.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_effect_no_target(&mut self, pid: PlayerId, effect: &EffectDefinition) {
        match effect {
            EffectDefinition::DrawCards { count } => {
                for _ in 0..*count {
                    self.draw_card(pid);
                }
            }
            EffectDefinition::FilterTopCard { depth } => {
                // In CLI, the binary will handle the "look and decide" interaction.
                // Engine just surfaces the top card info via a draw here for simplicity.
                let _ = depth;
            }
            EffectDefinition::SummonToken { token } => {
                let instance_id = Uuid::new_v4();
                let unit = UnitInPlay {
                    instance_id,
                    card_def_id: format!("token_{}", token.name.to_lowercase().replace(' ', "_")),
                    display_name: token.name.clone(),
                    current_attack: token.attack,
                    current_health: token.health,
                    max_health: token.health,
                    keywords: token.keywords.clone(),
                    played_this_turn: true,
                    has_attacked: false,
                    is_shielded: false,
                };
                self.state.players[pid as usize].board.push(unit);
                self.emit(GameEvent::TokenSummoned {
                    player: pid,
                    instance_id,
                    name: token.name.clone(),
                    attack: token.attack,
                    health: token.health,
                });
            }
            _ => {}
        }
    }

    fn apply_effect_hero(&mut self, target_player: PlayerId, effect: &EffectDefinition) {
        match effect {
            EffectDefinition::Heal { amount } => {
                self.heal_hero(target_player, *amount as i32);
            }
            EffectDefinition::DealDamage { amount } => {
                let dmg = *amount as i16;
                self.state.players[target_player as usize].health -= dmg as i32;
                let hp = self.state.players[target_player as usize].health;
                self.emit(GameEvent::HeroDamaged { player: target_player, amount: dmg, remaining_health: hp });
                self.check_game_over();
            }
            EffectDefinition::Drain { damage, healing } => {
                let acting = target_player ^ 1; // drain targets enemy, heals self
                let dmg = *damage as i32;
                let heal = *healing as i32;
                self.state.players[target_player as usize].health -= dmg;
                let hp = self.state.players[target_player as usize].health;
                self.emit(GameEvent::HeroDamaged { player: target_player, amount: dmg as i16, remaining_health: hp });
                self.heal_hero(acting, heal);
                self.check_game_over();
            }
            _ => {}
        }
    }

    fn heal_hero(&mut self, pid: PlayerId, amount: i32) {
        let p = &mut self.state.players[pid as usize];
        let new_health = (p.health + amount).min(p.max_health);
        let actual = new_health - p.health;
        p.health = new_health;
        if actual > 0 {
            self.emit(GameEvent::HeroHealed { player: pid, amount: actual, new_health });
        }
    }

    fn remove_dead(&mut self, pid: PlayerId) {
        let dead: Vec<(Uuid, String)> = self.state.players[pid as usize].board.iter()
            .filter(|u| u.current_health <= 0)
            .map(|u| (u.instance_id, u.display_name.clone()))
            .collect();
        for (id, name) in dead {
            let card_id = self.state.players[pid as usize].board.iter()
                .find(|u| u.instance_id == id).unwrap().card_def_id.clone();
            self.state.players[pid as usize].board.retain(|u| u.instance_id != id);
            self.state.players[pid as usize].discard.push(card_id);
            self.emit(GameEvent::UnitDied { player: pid, card_name: name });
        }
    }

    fn check_game_over(&mut self) {
        if let Some(winner) = self.winner() {
            self.emit(GameEvent::GameOver { winner });
        }
    }

    fn emit(&mut self, event: GameEvent) {
        self.events.push(event);
    }

    fn find_unit(&self, pid: PlayerId, id: Uuid) -> &UnitInPlay {
        self.state.players[pid as usize].board.iter().find(|u| u.instance_id == id).unwrap()
    }

    fn find_unit_mut(&mut self, pid: PlayerId, id: Uuid) -> &mut UnitInPlay {
        self.state.players[pid as usize].board.iter_mut().find(|u| u.instance_id == id).unwrap()
    }
}

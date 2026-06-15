use std::collections::HashMap;
use crate::card::{CardDefinition, CardType};
use crate::color::CardColor;
use crate::deck::{DeckDefinition, DECK_SIZE, MAX_CHAMPIONS_PER_DECK, MAX_COPIES_PER_CARD, MAX_COPIES_PER_CHAMPION};
use crate::registry::CardRegistry;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DeckValidationError {
    #[error("deck has {actual} cards, expected exactly {expected}")]
    WrongSize { expected: usize, actual: usize },
    #[error("card '{id}' not found in registry")]
    UnknownCard { id: String },
    #[error("card '{id}' appears {copies} times (max {max})")]
    TooManyCopies { id: String, copies: usize, max: usize },
    #[error("deck contains {count} champion(s), maximum is {max}")]
    TooManyChampions { count: usize, max: usize },
    #[error("card '{id}' (color {card_color}) is not allowed by the champion's color identity")]
    ColorIdentityViolation { id: String, card_color: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CardValidationError {
    #[error("unit card '{id}' is missing attack value")]
    UnitMissingAttack { id: String },
    #[error("unit card '{id}' is missing health value")]
    UnitMissingHealth { id: String },
    #[error("spell card '{id}' should not have attack or health values")]
    SpellHasStats { id: String },
    #[error("relic card '{id}' is missing durability value")]
    RelicMissingDurability { id: String },
    #[error("champion card '{id}' is missing champion_power")]
    ChampionMissingPower { id: String },
}

pub fn validate_card(card: &CardDefinition) -> Vec<CardValidationError> {
    let mut errors = Vec::new();

    match card.card_type {
        CardType::Unit => {
            if card.attack.is_none() {
                errors.push(CardValidationError::UnitMissingAttack { id: card.id.clone() });
            }
            if card.health.is_none() {
                errors.push(CardValidationError::UnitMissingHealth { id: card.id.clone() });
            }
        }
        CardType::Spell => {
            if card.attack.is_some() || card.health.is_some() {
                errors.push(CardValidationError::SpellHasStats { id: card.id.clone() });
            }
        }
        CardType::Relic => {
            if card.durability.is_none() {
                errors.push(CardValidationError::RelicMissingDurability { id: card.id.clone() });
            }
        }
        CardType::Champion => {
            if card.champion_power.is_none() {
                errors.push(CardValidationError::ChampionMissingPower { id: card.id.clone() });
            }
        }
    }

    errors
}

pub fn validate_deck(deck: &DeckDefinition, registry: &CardRegistry) -> Vec<DeckValidationError> {
    let mut errors = Vec::new();

    if deck.card_ids.len() != DECK_SIZE {
        errors.push(DeckValidationError::WrongSize {
            expected: DECK_SIZE,
            actual: deck.card_ids.len(),
        });
    }

    let mut copy_counts: HashMap<&str, usize> = HashMap::new();
    let mut champion_count = 0usize;
    let mut champion_def: Option<&CardDefinition> = None;

    for id in &deck.card_ids {
        let Some(card) = registry.get(id) else {
            errors.push(DeckValidationError::UnknownCard { id: id.clone() });
            continue;
        };

        *copy_counts.entry(id.as_str()).or_insert(0) += 1;

        if card.is_champion() {
            champion_count += 1;
            champion_def = Some(card);
        }
    }

    for (id, &copies) in &copy_counts {
        let max = if registry.get(id).is_some_and(|c| c.is_champion()) {
            MAX_COPIES_PER_CHAMPION
        } else {
            MAX_COPIES_PER_CARD
        };

        if copies > max {
            errors.push(DeckValidationError::TooManyCopies {
                id: id.to_string(),
                copies,
                max,
            });
        }
    }

    if champion_count > MAX_CHAMPIONS_PER_DECK {
        errors.push(DeckValidationError::TooManyChampions {
            count: champion_count,
            max: MAX_CHAMPIONS_PER_DECK,
        });
    }

    // Enforce champion color identity when a champion is present.
    if let Some(champ) = champion_def {
        if let Some(allowed) = &champ.allowed_deck_colors {
            for id in &deck.card_ids {
                let Some(card) = registry.get(id) else {
                    continue;
                };
                if card.is_champion() {
                    continue;
                }
                if card.color == CardColor::Neutral {
                    continue;
                }
                if !allowed.contains(&card.color) {
                    errors.push(DeckValidationError::ColorIdentityViolation {
                        id: id.clone(),
                        card_color: format!("{:?}", card.color),
                    });
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{CardRarity, CardType};
    use crate::champion::ChampionPowerDefinition;
    use crate::effects::{EffectDefinition, TargetingRule};

    fn make_unit(id: &str, color: CardColor) -> CardDefinition {
        CardDefinition {
            id: id.to_string(),
            name: id.to_string(),
            color,
            card_type: CardType::Unit,
            cost: 2,
            attack: Some(2),
            health: Some(2),
            durability: None,
            keywords: vec![],
            rules_text: String::new(),
            tribes: vec![],
            art_id: id.to_string(),
            art_background_id: "default".to_string(),
            frame_id: "default".to_string(),
            rarity: CardRarity::Common,
            set_id: "test".to_string(),
            champion_power: None,
            spell_effect: None,
            allowed_deck_colors: None,
        }
    }

    fn make_champion(id: &str, color: CardColor) -> CardDefinition {
        CardDefinition {
            id: id.to_string(),
            name: id.to_string(),
            color: color.clone(),
            card_type: CardType::Champion,
            cost: 5,
            attack: Some(3),
            health: Some(4),
            durability: None,
            keywords: vec![],
            rules_text: String::new(),
            tribes: vec![],
            art_id: id.to_string(),
            art_background_id: "default".to_string(),
            frame_id: "default".to_string(),
            rarity: CardRarity::Champion,
            set_id: "test".to_string(),
            champion_power: Some(ChampionPowerDefinition {
                id: format!("{}_power", id),
                cost: 2,
                targeting: TargetingRule::EnemyUnit,
                effect: EffectDefinition::DealDamage { amount: 1 },
                description: "Deal 1 damage to an enemy unit.".to_string(),
            }),
            spell_effect: None,
            allowed_deck_colors: Some(vec![color]),
        }
    }

    fn sixty_units(color: CardColor) -> (CardRegistry, DeckDefinition) {
        // 20 distinct units × 3 copies = 60
        let cards: Vec<CardDefinition> = (0..20)
            .map(|i| make_unit(&format!("unit_{}", i), color.clone()))
            .collect();

        let card_ids: Vec<String> = cards
            .iter()
            .flat_map(|c| std::iter::repeat(c.id.clone()).take(3))
            .collect();

        let registry = CardRegistry::from_cards(cards);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };
        (registry, deck)
    }

    #[test]
    fn valid_deck_passes() {
        let (registry, deck) = sixty_units(CardColor::Red);
        let errors = validate_deck(&deck, &registry);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn deck_wrong_size_fails() {
        let cards: Vec<CardDefinition> = (0..5).map(|i| make_unit(&format!("u{}", i), CardColor::Red)).collect();
        let card_ids: Vec<String> = cards.iter().map(|c| c.id.clone()).collect(); // only 5
        let registry = CardRegistry::from_cards(cards);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };

        let errors = validate_deck(&deck, &registry);
        assert!(errors.iter().any(|e| matches!(e, DeckValidationError::WrongSize { .. })));
    }

    #[test]
    fn too_many_copies_fails() {
        // One card, 60 copies — way over the 3-copy limit
        let card = make_unit("only_card", CardColor::Red);
        let card_ids = vec!["only_card".to_string(); 60];
        let registry = CardRegistry::from_cards(vec![card]);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };

        let errors = validate_deck(&deck, &registry);
        assert!(errors.iter().any(|e| matches!(e, DeckValidationError::TooManyCopies { .. })));
    }

    #[test]
    fn two_champions_fails() {
        // 18 units × 3 = 54 + 2 champions = 56 (also wrong size, but TooManyChampions must fire)
        let mut cards: Vec<CardDefinition> = (0..18)
            .map(|i| make_unit(&format!("u{}", i), CardColor::Red))
            .collect();
        cards.push(make_champion("champ_a", CardColor::Red));
        cards.push(make_champion("champ_b", CardColor::Green));

        let mut card_ids: Vec<String> = (0..18)
            .flat_map(|i| std::iter::repeat(format!("u{}", i)).take(3))
            .collect();
        card_ids.push("champ_a".to_string());
        card_ids.push("champ_b".to_string());

        let registry = CardRegistry::from_cards(cards);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };

        let errors = validate_deck(&deck, &registry);
        assert!(errors.iter().any(|e| matches!(e, DeckValidationError::TooManyChampions { .. })));
    }

    #[test]
    fn color_identity_violation_fails() {
        // Red champion + 19 red units × 3 = 57 + 1 champ + 2 blue units = 60
        let mut cards: Vec<CardDefinition> = (0..19)
            .map(|i| make_unit(&format!("r{}", i), CardColor::Red))
            .collect();
        cards.push(make_champion("red_champ", CardColor::Red));
        cards.push(make_unit("blue_a", CardColor::Blue));
        cards.push(make_unit("blue_b", CardColor::Blue));

        let mut card_ids: Vec<String> = (0..19)
            .flat_map(|i| std::iter::repeat(format!("r{}", i)).take(3))
            .collect();
        card_ids.push("red_champ".to_string());
        card_ids.push("blue_a".to_string());
        card_ids.push("blue_b".to_string());

        let registry = CardRegistry::from_cards(cards);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };

        let errors = validate_deck(&deck, &registry);
        assert!(
            errors.iter().any(|e| matches!(e, DeckValidationError::ColorIdentityViolation { .. })),
            "expected color identity violation, got: {:?}", errors
        );
    }

    #[test]
    fn neutral_cards_bypass_color_identity() {
        // Red champion + 19 red units × 3 = 57 + 1 champ + 2 neutral units = 60
        let mut cards: Vec<CardDefinition> = (0..19)
            .map(|i| make_unit(&format!("r{}", i), CardColor::Red))
            .collect();
        cards.push(make_champion("red_champ", CardColor::Red));
        cards.push(make_unit("neutral_a", CardColor::Neutral));
        cards.push(make_unit("neutral_b", CardColor::Neutral));

        let mut card_ids: Vec<String> = (0..19)
            .flat_map(|i| std::iter::repeat(format!("r{}", i)).take(3))
            .collect();
        card_ids.push("red_champ".to_string());
        card_ids.push("neutral_a".to_string());
        card_ids.push("neutral_b".to_string());

        let registry = CardRegistry::from_cards(cards);
        let deck = DeckDefinition { id: "d".into(), name: "D".into(), card_ids };

        let errors = validate_deck(&deck, &registry);
        let identity_errors: Vec<_> = errors
            .iter()
            .filter(|e| matches!(e, DeckValidationError::ColorIdentityViolation { .. }))
            .collect();
        assert!(identity_errors.is_empty(), "neutral cards should not trigger color identity: {:?}", identity_errors);
    }

    #[test]
    fn unit_missing_attack_fails() {
        let mut card = make_unit("test", CardColor::Red);
        card.attack = None;
        let errors = validate_card(&card);
        assert!(errors.iter().any(|e| matches!(e, CardValidationError::UnitMissingAttack { .. })));
    }

    #[test]
    fn spell_with_stats_fails() {
        let mut card = make_unit("test_spell", CardColor::Blue);
        card.card_type = CardType::Spell;
        card.rules_text = "Deal 2 damage.".into();
        // attack/health are still Some from make_unit
        let errors = validate_card(&card);
        assert!(errors.iter().any(|e| matches!(e, CardValidationError::SpellHasStats { .. })));
    }

    #[test]
    fn relic_missing_durability_fails() {
        let mut card = make_unit("test_relic", CardColor::Black);
        card.card_type = CardType::Relic;
        card.attack = None;
        card.health = None;
        card.durability = None;
        let errors = validate_card(&card);
        assert!(errors.iter().any(|e| matches!(e, CardValidationError::RelicMissingDurability { .. })));
    }

    #[test]
    fn champion_missing_power_fails() {
        let mut card = make_champion("test_champ", CardColor::Green);
        card.champion_power = None;
        let errors = validate_card(&card);
        assert!(errors.iter().any(|e| matches!(e, CardValidationError::ChampionMissingPower { .. })));
    }
}

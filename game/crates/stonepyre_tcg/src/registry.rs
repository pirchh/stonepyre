use std::collections::HashMap;
use crate::card::CardDefinition;

#[derive(Debug, Default, Clone)]
pub struct CardRegistry {
    cards: HashMap<String, CardDefinition>,
}

impl CardRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_cards(cards: impl IntoIterator<Item = CardDefinition>) -> Self {
        let cards = cards.into_iter().map(|c| (c.id.clone(), c)).collect();
        Self { cards }
    }

    pub fn insert(&mut self, card: CardDefinition) {
        self.cards.insert(card.id.clone(), card);
    }

    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.cards.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &CardDefinition> {
        self.cards.values()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

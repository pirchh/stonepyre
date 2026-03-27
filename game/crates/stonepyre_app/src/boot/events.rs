#[derive(Event, Debug, Clone)]
pub struct StartWorldEvent {
    pub character_id: Option<Uuid>, // Some when playing an existing char
    pub seed: u64,                  // placeholder: later derive from save/character/world
}
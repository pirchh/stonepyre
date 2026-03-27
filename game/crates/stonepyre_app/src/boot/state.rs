use bevy::prelude::*;
use chrono::NaiveDateTime;
use uuid::Uuid;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum Screen {
    #[default]
    MainMenu,
    AccountLogin,
    CharacterSelect,
    InWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusField {
    Email,
    Password,
    DisplayName,
    NewCharacterName,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub account_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Character {
    pub character_id: Uuid,
    pub name: String,
    pub cash: f64,
    pub created_at: NaiveDateTime,
}

#[derive(Resource)]
pub struct BootState {
    pub busy: bool,
    pub server_base_url: String,

    // account/login fields
    pub login_mode_is_register: bool,
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub focus: FocusField,
    pub error_banner: Option<String>,
    pub session: Option<Session>,

    // character selection
    pub slots: [Option<Character>; 5],
    pub selected_slot: usize,
    pub new_character_name: String,
    pub new_character_skin: String,

    // Boot -> world handoff (no Bevy Events needed)
    pub pending_start_world: Option<Uuid>,
}

impl Default for BootState {
    fn default() -> Self {
        Self {
            busy: false,
            server_base_url: String::new(),

            login_mode_is_register: false,
            email: String::new(),
            password: String::new(),
            display_name: String::new(),
            focus: FocusField::Email,
            error_banner: None,
            session: None,

            slots: [(); 5].map(|_| None),
            selected_slot: 0,
            new_character_name: String::new(),
            new_character_skin: "base_greyscale".to_string(),

            pending_start_world: None,
        }
    }
}

impl BootState {
    pub fn clear_errors(&mut self) {
        self.error_banner = None;
    }

    pub fn set_error(&mut self, s: impl Into<String>) {
        self.error_banner = Some(s.into());
    }

    pub fn clamp_selected_slot(&mut self) {
        let max = self.slots.len().saturating_sub(1);
        if self.selected_slot > max {
            self.selected_slot = max;
        }
    }

    /// For now we always preview the baked south idle:
    /// assets/characters/humanoid/<skin>/idle/south/south_idle.png
    pub fn selected_skin_preview_path(&self) -> String {
        format!(
            "characters/humanoid/{}/idle/south/south_idle.png",
            self.new_character_skin
        )
    }
}
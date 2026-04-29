use bevy::prelude::*;

pub mod camera;
pub mod fonts;
pub mod game_net;
pub mod net;
pub mod ui;

// must be public so main.rs can use Screen + BootState
pub mod state;
pub use state::{BootState, Screen};


#[derive(Component)]
struct CharacterSelectErrorOverlayRoot;

#[derive(Component)]
struct CharacterSelectErrorOverlayText;

pub struct BootFlowPlugin;

impl Plugin for BootFlowPlugin {
    fn build(&self, app: &mut App) {
        app
            // State + resources
            .init_state::<Screen>()
            .init_resource::<BootState>()
            .init_resource::<net::NetRuntime>()
            .init_resource::<game_net::GameNetRuntime>()
            .init_resource::<game_net::GameNetStatus>()
            .init_resource::<fonts::UiFonts>()
            // Startup init
            .add_systems(
                Startup,
                (
                    camera::spawn_boot_ui_camera,
                    net::init_server_base_url,
                    fonts::load_ui_fonts,
                ),
            )
            // Net pump
            .add_systems(Update, net::pump_net_results)
            // Screen lifecycle
            .add_systems(OnExit(Screen::MainMenu), ui::despawn_screen)
            .add_systems(OnExit(Screen::AccountLogin), ui::despawn_screen)
            .add_systems(OnExit(Screen::CharacterSelect), ui::despawn_screen)
            // When entering the real game, remove the boot camera
            .add_systems(OnEnter(Screen::InWorld), camera::despawn_boot_ui_camera)
            // Enters
            .add_systems(OnEnter(Screen::MainMenu), ui::main_menu_enter)
            .add_systems(OnEnter(Screen::AccountLogin), ui::login_enter)
            .add_systems(OnEnter(Screen::CharacterSelect), ui::character_select_enter)
            // Updates (gated by state)
            .add_systems(Update, ui::main_menu_update.run_if(in_state(Screen::MainMenu)))
            .add_systems(Update, ui::login_update.run_if(in_state(Screen::AccountLogin)))
            .add_systems(
                Update,
                (
                    preflight_selected_character_world_join,
                    ui::character_select_update,
                    update_character_select_error_overlay,
                )
                    .chain()
                    .run_if(in_state(Screen::CharacterSelect)),
            );
    }
}

/// Intercepts the Enter World button on character select before the normal
/// character-select update can transition into the local world.
///
/// The server-side websocket guard remains authoritative. This preflight is only
/// for UX: if the selected character is already active, we stay on character
/// select and show a clear error instead of briefly spawning the local world.
fn preflight_selected_character_world_join(
    mut st: ResMut<BootState>,
    net_runtime: Res<net::NetRuntime>,
    mut q_btn: Query<
        (&mut Interaction, &ui::common::ButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (mut interaction, action) in &mut q_btn {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let ui::common::ButtonAction::PlaySelectedCharacter = action else {
            continue;
        };

        // Stop character_select_update from immediately entering Screen::InWorld.
        *interaction = Interaction::None;

        if st.busy {
            continue;
        }

        st.clear_errors();
        st.clamp_selected_slot();

        let Some(character_id) = st.slots[st.selected_slot]
            .as_ref()
            .map(|character| character.character_id)
        else {
            st.set_error("Select a character before entering the world.");
            continue;
        };

        let Some(token) = st.session.as_ref().map(|session| session.token.clone()) else {
            st.set_error("You need to log in before entering the world.");
            continue;
        };

        st.busy = true;
        st.pending_start_world = None;

        net::spawn_check_character_world_status(
            &st,
            net_runtime.as_ref(),
            token,
            character_id,
        );
    }
}

fn update_character_select_error_overlay(
    mut commands: Commands,
    st: Res<BootState>,
    fonts: Res<fonts::UiFonts>,
    roots: Query<Entity, With<ui::ScreenRoot>>,
    mut q_text: Query<&mut Text, With<CharacterSelectErrorOverlayText>>,
    q_overlay: Query<Entity, With<CharacterSelectErrorOverlayRoot>>,
) {
    let message = st.error_banner.clone().unwrap_or_default();

    if let Some(mut text) = q_text.iter_mut().next() {
        text.0 = message;
        return;
    }

    if message.is_empty() || !q_overlay.is_empty() {
        return;
    }

    let Some(root) = roots.iter().next() else {
        return;
    };

    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(112.0),
                width: Val::Percent(100.0),
                height: Val::Px(52.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            CharacterSelectErrorOverlayRoot,
            Name::new("character_select_error_overlay"),
        ))
        .id();

    commands.entity(root).add_child(overlay);

    let banner = commands
        .spawn((
            Node {
                width: Val::Px(720.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(18.0)),
                border_radius: BorderRadius {
                    top_left: Val::Px(10.0),
                    top_right: Val::Px(10.0),
                    bottom_left: Val::Px(10.0),
                    bottom_right: Val::Px(10.0),
                },
                ..default()
            },
            BackgroundColor(Color::srgba(0.32, 0.07, 0.055, 0.96)),
            Name::new("character_select_error_banner"),
        ))
        .id();

    commands.entity(overlay).add_child(banner);

    let text = commands
        .spawn((
            Text::new(message),
            TextFont {
                font: fonts.mono.clone(),
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            CharacterSelectErrorOverlayText,
            Name::new("character_select_error_text"),
        ))
        .id();

    commands.entity(banner).add_child(text);
}


use bevy::prelude::*;

#[derive(Resource, Default, Clone)]
pub struct UiFonts {
    pub regular: Handle<Font>,
    pub accent: Handle<Font>,
    pub mono: Handle<Font>,
}

pub fn load_ui_fonts(asset_server: Res<AssetServer>, mut fonts: ResMut<UiFonts>) {
    // Put whatever font files you actually have in assets/fonts/
    fonts.regular = asset_server.load("fonts/ui_regular.ttf");
    fonts.accent = asset_server.load("fonts/ui_accent.ttf");
    fonts.mono = asset_server.load("fonts/ui_mono.ttf");
}

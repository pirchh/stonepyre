use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use chrono::NaiveDateTime;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use uuid::Uuid;

use super::state::{BootState, Character, Screen, Session};

pub fn init_server_base_url(mut st: ResMut<BootState>) {
    // dev default; override with STONEPYRE_SERVER_URL if you want
    st.server_base_url = std::env::var("STONEPYRE_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
}

#[derive(Debug)]
pub enum NetResult {
    AuthOk(Session),
    LoggedOut,
    AccountDeleted,
    Characters([Option<Character>; 5]),
    CharacterCreated(Character),
    CharacterDeleted(Uuid),
    CharacterWorldJoinAllowed(Uuid),
    CharacterWorldJoinRejected(String),
    Err(String),
}

#[derive(Resource)]
pub struct NetRuntime {
    pub tx: Sender<NetResult>,
    pub rx: Mutex<Receiver<NetResult>>,
}

impl Default for NetRuntime {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }
}

pub fn pump_net_results(
    mut st: ResMut<BootState>,
    mut next: ResMut<NextState<Screen>>,
    net: Res<NetRuntime>,
) {
    loop {
        // avoid holding the lock across our whole match handler
        let msg = {
            let rx = net.rx.lock().unwrap();
            rx.try_recv()
        };

        let Ok(msg) = msg else { break };

        match msg {
            NetResult::AuthOk(sess) => {
                st.session = Some(sess);
                st.busy = false;
                st.error_banner = None;
            }
            NetResult::LoggedOut => {
                st.session = None;
                st.busy = false;
            }
            NetResult::AccountDeleted => {
                st.session = None;
                st.busy = false;
            }
            NetResult::Characters(slots) => {
                st.slots = slots;
                st.busy = false;
            }
            NetResult::CharacterCreated(c) => {
                if let Some(i) = st.slots.iter().position(|s| s.is_none()) {
                    st.slots[i] = Some(c);
                }
                st.new_character_name.clear();
                st.busy = false;
            }
            NetResult::CharacterDeleted(id) => {
                for s in st.slots.iter_mut() {
                    if let Some(c) = s {
                        if c.character_id == id {
                            *s = None;
                        }
                    }
                }
                st.busy = false;
            }
            NetResult::CharacterWorldJoinAllowed(id) => {
                st.pending_start_world = Some(id);
                st.busy = false;
                st.error_banner = None;
                next.set(Screen::InWorld);
            }
            NetResult::CharacterWorldJoinRejected(msg) => {
                st.pending_start_world = None;
                st.error_banner = Some(msg);
                st.busy = false;
            }
            NetResult::Err(msg) => {
                st.error_banner = Some(msg);
                st.busy = false;
            }
        }
    }
}

fn client() -> Client {
    Client::builder().build().expect("reqwest blocking client")
}

fn bearer(token: &str) -> String {
    format!("Bearer {}", token)
}

#[derive(Debug, Serialize)]
struct RegisterReq {
    email: String,
    password: String,
    display_name: String,
}

#[derive(Debug, Serialize)]
struct LoginReq {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct AuthResp {
    token: String,
    account_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct CharacterDto {
    character_id: Uuid,
    name: String,
    cash: f64,
    created_at: NaiveDateTime,

    // ✅ Optional: future-proof. When server adds it, we’ll start receiving it.
    #[serde(default)]
    skin_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CharacterSlotsDto {
    slots: [Option<CharacterDto>; 5],
}

#[derive(Debug, Serialize)]
struct CreateCharacterReq {
    name: String,

    // ✅ Send this now; backend can ignore for now, or store later.
    // If/when you add skin_id column + API, you’re already wired.
    skin_id: String,
}

#[derive(Debug, Deserialize)]
struct ActiveSessionStatusDto {
    active: bool,
    message: Option<String>,
}

pub fn spawn_register(
    st: &BootState,
    net: &NetRuntime,
    email: String,
    password: String,
    display_name: String,
) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .post(format!("{}/v1/auth/register", base))
                .json(&RegisterReq {
                    email,
                    password,
                    display_name,
                })
                .send();

            match res {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        let _ = tx.send(NetResult::Err(format!("register failed: {status}")));
                        return;
                    }
                    match r.json::<AuthResp>() {
                        Ok(a) => {
                            let _ = tx.send(NetResult::AuthOk(Session {
                                token: a.token,
                                account_id: a.account_id,
                            }));
                        }
                        Err(e) => {
                            let _ = tx.send(NetResult::Err(format!("register parse error: {e}")));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("register error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_login(st: &BootState, net: &NetRuntime, email: String, password: String) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .post(format!("{}/v1/auth/login", base))
                .json(&LoginReq { email, password })
                .send();

            match res {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        let _ = tx.send(NetResult::Err(format!("login failed: {status}")));
                        return;
                    }
                    match r.json::<AuthResp>() {
                        Ok(a) => {
                            let _ = tx.send(NetResult::AuthOk(Session {
                                token: a.token,
                                account_id: a.account_id,
                            }));
                        }
                        Err(e) => {
                            let _ = tx.send(NetResult::Err(format!("login parse error: {e}")));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("login error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_logout(st: &BootState, net: &NetRuntime, token: String) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .post(format!("{}/v1/auth/logout", base))
                .header("Authorization", bearer(&token))
                .send();

            match res {
                Ok(r) if r.status().is_success() => {
                    let _ = tx.send(NetResult::LoggedOut);
                }
                Ok(r) => {
                    let _ = tx.send(NetResult::Err(format!("logout failed: {}", r.status())));
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("logout error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_delete_account(st: &BootState, net: &NetRuntime, token: String) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .delete(format!("{}/v1/auth/account", base))
                .header("Authorization", bearer(&token))
                .send();

            match res {
                Ok(r) if r.status().is_success() => {
                    let _ = tx.send(NetResult::AccountDeleted);
                }
                Ok(r) => {
                    let _ = tx.send(NetResult::Err(format!(
                        "delete account failed: {}",
                        r.status()
                    )));
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("delete account error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_list_characters(st: &BootState, net: &NetRuntime, token: String) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .get(format!("{}/v1/characters", base))
                .header("Authorization", bearer(&token))
                .send();

            match res {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        let _ = tx.send(NetResult::Err(format!(
                            "list characters failed: {status}"
                        )));
                        return;
                    }
                    match r.json::<CharacterSlotsDto>() {
                        Ok(dto) => {
                            let mut slots: [Option<Character>; 5] = [None, None, None, None, None];
                            for i in 0..5 {
                                slots[i] = dto.slots[i].as_ref().map(|c| {
                                    // c.skin_id is available (optional) but not stored yet in state::Character.
                                    let _skin = c.skin_id.clone();

                                    Character {
                                        character_id: c.character_id,
                                        name: c.name.clone(),
                                        cash: c.cash,
                                        created_at: c.created_at,
                                    }
                                });
                            }
                            let _ = tx.send(NetResult::Characters(slots));
                        }
                        Err(e) => {
                            let _ = tx.send(NetResult::Err(format!(
                                "list characters parse error: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("list characters error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_create_character(
    st: &BootState,
    net: &NetRuntime,
    token: String,
    name: String,
    skin_id: String,
) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .post(format!("{}/v1/characters", base))
                .header("Authorization", bearer(&token))
                .json(&CreateCharacterReq { name, skin_id })
                .send();

            match res {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        let _ = tx.send(NetResult::Err(format!(
                            "create character failed: {status}"
                        )));
                        return;
                    }
                    match r.json::<CharacterDto>() {
                        Ok(c) => {
                            let _skin = c.skin_id;

                            let _ = tx.send(NetResult::CharacterCreated(Character {
                                character_id: c.character_id,
                                name: c.name,
                                cash: c.cash,
                                created_at: c.created_at,
                            }));
                        }
                        Err(e) => {
                            let _ = tx.send(NetResult::Err(format!(
                                "create character parse error: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("create character error: {e}")));
                }
            }
        })
        .detach();
}

pub fn spawn_check_character_world_status(
    st: &BootState,
    net: &NetRuntime,
    token: String,
    id: Uuid,
) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .get(format!("{}/v1/game/characters/{}/active-session", base, id))
                .header("Authorization", bearer(&token))
                .send();

            match res {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        let _ = tx.send(NetResult::CharacterWorldJoinRejected(format!(
                            "enter world failed: {status}"
                        )));
                        return;
                    }

                    match r.json::<ActiveSessionStatusDto>() {
                        Ok(dto) => {
                            if dto.active {
                                let message = dto.message.unwrap_or_else(|| {
                                    "This character is already in world.".to_string()
                                });
                                let _ = tx.send(NetResult::CharacterWorldJoinRejected(message));
                            } else {
                                let _ = tx.send(NetResult::CharacterWorldJoinAllowed(id));
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(NetResult::CharacterWorldJoinRejected(format!(
                                "enter world status parse error: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetResult::CharacterWorldJoinRejected(format!(
                        "enter world status error: {e}"
                    )));
                }
            }
        })
        .detach();
}

pub fn spawn_delete_character(st: &BootState, net: &NetRuntime, token: String, id: Uuid) {
    let base = st.server_base_url.clone();
    let tx = net.tx.clone();

    IoTaskPool::get()
        .spawn(async move {
            let c = client();
            let res = c
                .delete(format!("{}/v1/characters/{}", base, id))
                .header("Authorization", bearer(&token))
                .send();

            match res {
                Ok(r) if r.status().is_success() => {
                    let _ = tx.send(NetResult::CharacterDeleted(id));
                }
                Ok(r) => {
                    let _ = tx.send(NetResult::Err(format!(
                        "delete character failed: {}",
                        r.status()
                    )));
                }
                Err(e) => {
                    let _ = tx.send(NetResult::Err(format!("delete character error: {e}")));
                }
            }
        })
        .detach();
}

//! Quick join: the part of shroudkit meant for sharing with friends. Client
//! only. Switches live under `[quickjoin]` in config.ini.
//!
//! - `fast_join`: the Join screen no longer asks Steam for the worldwide
//!   server list, so joins no longer time out when the password is confirmed
//!   quickly (README: Fast join).
//! - `remember_password`: type a server's password once; after that, press
//!   Join with the box empty and the saved password is sent (`remember.rs`,
//!   README: Remember password). Saved in `shroudkit/quickjoin.ini`.

mod cave;
mod remember;
mod store;

use std::sync::Mutex;

use shroudkit_api::{Api, Effect, FeatureSet, Kit};

static FEATURES: Mutex<Option<FeatureSet>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudkit_plugin_init(api: *const Api) -> bool {
    let Some(kit) = (unsafe { Kit::new(api, "quickjoin") }) else { return false };
    if kit.is_server() {
        kit.info("client only, nothing to do on the server");
        return true;
    }

    let saved = store::load();
    kit.info(&format!("{saved} remembered server password(s) in quickjoin.ini"));

    let mut features = FeatureSet::new(kit);
    features.add("fast_join", "none", fast_join);
    features.add("remember_password", "none", remember::build);
    *FEATURES.lock().unwrap() = Some(features);
    update();
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudkit_plugin_tick() {
    update();
}

fn update() {
    if let Some(features) = FEATURES.lock().unwrap().as_mut() {
        features.update();
    }
}

/// README, Fast join: two in-place patches stop RequestInternetServerList, which
/// held every join reply for about 17 s.
fn fast_join(kit: &Kit) -> Result<Effect, String> {
    // mov rax, [rcx] / call [rax] (RequestInternetServerList) / mov edx, [rdi-4Ch] / xor r9d, r9d / mov [rdi-48h], rax
    let request = kit.find_unique("fast_join request", "48 8B 01 FF 10 8B 57 B4 45 33 C9 48 89 47 B8")?;
    // test rdx, rdx / je fail / cmp [rdi-40h], r12 / je fail / test rax, rax / je fail / mov al, 2
    let check = kit.find_unique("fast_join check", "48 85 D2 74 15 4C 39 67 C0 74 0F 48 85 C0 74 0A B0 02")? + 3;
    Ok(Effect::Patches(vec![
        kit.patch(request, &[0x31, 0xC0, 0x90, 0x90, 0x90])?, // xor eax, eax: no request, handle 0
        kit.patch(check, &[0x90, 0x90])?,                       // a 0 internet handle no longer fails the search
    ]))
}

//! `remember_password`: learns a server's password the first time it is typed
//! in the Join screen's password box, and fills it in when Join is pressed with
//! the box empty (README: Remember password).
//!
//! The Join screen (1.0 build 1269579: exe+0xc48370) gives the box a game
//! string `{ptr [rbp+80h], len [rbp+88h], cap [rbp+90h]}` whose buffer is 64
//! bytes inline at `rbp+0A0h`. When a join is confirmed (OK, Enter, or the
//! screen's Join button for optional passwords), the code at exe+0xc4a519
//! reads that string and posts it to the pregame logic, which
//! starts the join:
//!
//! ```text
//! mov rax, [rbp+80h]      <- site: detour
//! mov [rsp+70h], rax
//! mov rax, [rbp+88h]
//! mov [rsp+78h], rax
//! ```
//!
//! The selected server is the 0x22-byte block at `rdi+40h` (rdi = the
//! screen's state block); its hex is the key the password is saved under.

use std::sync::OnceLock;

use shroudkit_api::{Effect, Kit};

use crate::cave::{self, Regs};
use crate::store;

const BUTTON_OK: u32 = 2;
/// Bytes the screen checks besides the OK result (0xc4a503, 0xc4a50c).
const ENTER_FLAG: usize = 0x6c7;
const JOIN_BUTTON_FLAG: usize = 0x137;
const STRING_POINTER: usize = 0x80;
const STRING_LENGTH: usize = 0x88;
const STRING_CAPACITY: usize = 0x90;
const STRING_INLINE: usize = 0xa0;
const SERVER_BLOCK: usize = 0x40;
const SERVER_BLOCK_SIZE: usize = 0x22;

static KIT: OnceLock<Kit> = OnceLock::new();

pub fn build(kit: &Kit) -> Result<Effect, String> {
    if kit.is_server() {
        return Err("remember_password: client only".into());
    }
    // mov rax, [rbp+80h] / mov [rsp+70h], rax / mov rax, [rbp+88h] / mov [rsp+78h], rax
    let site = kit.find_unique("remember_password", "48 8B 85 80 00 00 00 48 89 44 24 70 48 8B 85 88 00 00 00 48 89 44 24 78")?;
    let _ = KIT.set(*kit);
    let (jump, flag) = cave::install(kit, "remember_password", site, &[0x48, 0x8B, 0x85, 0x80, 0x00, 0x00, 0x00], on_confirm)?;
    // The jump stays in; the flag switches the feature (off = the cave only runs the original instruction).
    if !jump.set_enabled(true) {
        return Err("remember_password: could not write the jump".into());
    }
    Ok(Effect::Flag(flag))
}

fn short(key: &str) -> &str {
    &key[..key.len().min(12)]
}

extern "C" fn on_confirm(regs: *mut Regs) {
    let regs = unsafe { &*regs };
    let Some(kit) = KIT.get() else { return };
    let (frame, state) = (regs.rbp as usize, regs.rdi as usize);
    // The site is only reached when a join is confirmed: the box's OK (ebx = 2), Enter in the text field
    // ([rbp+6C7h]) or the screen's extra Join button for optional passwords ([rbp+137h]). All of them count.
    let byte_at = |offset: usize| unsafe { std::ptr::read((frame + offset) as *const u8) };
    let route = match (regs.rbx as u32, byte_at(ENTER_FLAG), byte_at(JOIN_BUTTON_FLAG)) {
        (BUTTON_OK, _, _) => "OK button",
        (_, enter, _) if enter != 0 => "Enter",
        (_, _, join) if join != 0 => "Join button",
        _ => "other",
    };
    let block = unsafe { std::slice::from_raw_parts((state + SERVER_BLOCK) as *const u8, SERVER_BLOCK_SIZE) };
    let key: String = block.iter().map(|byte| format!("{byte:02x}")).collect();
    let read = |offset: usize| unsafe { std::ptr::read_unaligned((frame + offset) as *const usize) };
    let (pointer, length, capacity) = (read(STRING_POINTER), read(STRING_LENGTH), read(STRING_CAPACITY));

    kit.info(&format!("remember_password: join confirmed ({route}) for server {}, typed length {length}", short(&key)));
    if length > 0 {
        if length > 256 || pointer == 0 {
            kit.warn(&format!("remember_password: unexpected password string (pointer {pointer:#x}, length {length}), not saved"));
            return;
        }
        let typed = unsafe { std::slice::from_raw_parts(pointer as *const u8, length) };
        let Ok(typed) = std::str::from_utf8(typed) else {
            kit.warn("remember_password: the typed password is not UTF-8, not saved");
            return;
        };
        match store::remember(&key, typed) {
            Ok(true) => kit.info(&format!("remember_password: saved the typed password for server {}", short(&key))),
            Ok(false) => {}
            Err(reason) => kit.error(&format!("remember_password: {reason}")),
        }
        return;
    }

    let Some(saved) = store::get(&key) else {
        kit.info(&format!("remember_password: Join with an empty box, but no saved password for server {} (key {key})", short(&key)));
        return;
    };
    // Only the untouched inline buffer the screen set up, and only if it fits with a terminating 0.
    if pointer != frame + STRING_INLINE || saved.len() + 1 > capacity.min(0x40) {
        kit.warn(&format!("remember_password: saved password for server {} not filled (buffer {pointer:#x}, capacity {capacity})", short(&key)));
        return;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(saved.as_ptr(), pointer as *mut u8, saved.len());
        std::ptr::write((pointer + saved.len()) as *mut u8, 0);
        std::ptr::write_unaligned((frame + STRING_LENGTH) as *mut usize, saved.len());
    }
    kit.info(&format!("remember_password: filled the saved password for server {}", short(&key)));
}

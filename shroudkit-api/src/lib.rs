//! C ABI shared between the shroudkit loader (`dbghelp.dll`) and its plugins.
//!
//! A plugin is a DLL in `<game>/shroudkit/plugins/` exporting
//! `shroudkit_plugin_init(*const Api) -> bool` and, optionally,
//! `shroudkit_plugin_tick()`, which the loader calls a few times per second.

pub mod asm;

use std::marker::PhantomData;

/// Bumped whenever [`Api`] changes layout. Plugins refuse to load on mismatch.
pub const API_VERSION: u32 = 7;

pub const INIT_SYMBOL: &[u8] = b"shroudkit_plugin_init\0";
pub const TICK_SYMBOL: &[u8] = b"shroudkit_plugin_tick\0";

pub type InitFn = unsafe extern "C" fn(*const Api) -> bool;
pub type TickFn = unsafe extern "C" fn();

/// Borrowed UTF-8 string, valid only for the duration of the call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str<'a> {
    pub ptr: *const u8,
    pub len: usize,
    _marker: PhantomData<&'a str>,
}

impl<'a> From<&'a str> for Str<'a> {
    fn from(s: &'a str) -> Self {
        Str { ptr: s.as_ptr(), len: s.len(), _marker: PhantomData }
    }
}

impl<'a> Str<'a> {
    /// # Safety
    /// `ptr`/`len` must describe valid UTF-8 that outlives `'a`.
    pub unsafe fn as_str(&self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.len)) }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info = 0,
    Warn = 1,
    Error = 2,
}

#[repr(C)]
pub struct Api {
    pub version: u32,
    pub log: unsafe extern "C" fn(level: Level, message: Str),
    /// Base address and size of the game executable image.
    pub module_base: unsafe extern "C" fn() -> usize,
    pub module_size: unsafe extern "C" fn() -> usize,
    pub is_server: unsafe extern "C" fn() -> bool,
    /// Scans the executable sections for an `AA BB ?? CC` signature. Returns the
    /// first match (0 if none) and writes the total number of matches to `count`.
    pub find_pattern: unsafe extern "C" fn(signature: Str, count: *mut usize) -> usize,
    /// Registers a patch (disabled) and returns its handle, or 0 on failure.
    pub create_patch: unsafe extern "C" fn(address: usize, bytes: *const u8, len: usize) -> u32,
    pub set_patch_enabled: unsafe extern "C" fn(handle: u32, enabled: bool) -> bool,
    /// Allocates executable memory within rel32 range of `address`, or returns 0.
    pub alloc_near: unsafe extern "C" fn(address: usize, size: usize) -> usize,
    pub write_memory: unsafe extern "C" fn(address: usize, bytes: *const u8, len: usize) -> bool,
    /// Config lookups; a missing key is added to config.ini with the default.
    pub config_bool: unsafe extern "C" fn(section: Str, key: Str, default: bool) -> bool,
    pub config_int: unsafe extern "C" fn(section: Str, key: Str, default: i64) -> i64,
    pub config_set_bool: unsafe extern "C" fn(section: Str, key: Str, value: bool),
    /// Virtual-key code for a key name stored in config ("F6", "K", "Insert"); 0 = unbound.
    pub config_key: unsafe extern "C" fn(section: Str, key: Str, default: Str) -> u32,
    /// True while the key is held and the game window has focus.
    pub key_down: unsafe extern "C" fn(virtual_key: u32) -> bool,
    /// Short audible cue for a toggle; there is no in-game overlay yet.
    pub notify: unsafe extern "C" fn(enabled: bool),
    /// Lists a switch on the in-game HUD. `state`: 0 off, 1 on, 2 unavailable, 3 paused (on, but held back).
    pub hud_set: unsafe extern "C" fn(section: Str, key: Str, state: u32),
    /// Screen brightness while the game has focus, as a gamma percentage
    /// (150 = gamma 1.5). 100 or less restores the display's own setting.
    pub set_brightness: unsafe extern "C" fn(percent: u32),
    /// True while this process hosts the game world: single player, a game this
    /// PC hosts, or the dedicated server. False in the menus and while connected
    /// to someone else's server. Read from the game's own session log.
    pub hosting: unsafe extern "C" fn() -> bool,
    /// A loader slot for another plugin family; not used by quickjoin. Kept so
    /// the table keeps the loader's layout.
    pub reserved_overlay: usize,
    /// Lists `[section] key`, a whole number in config.ini, as a number box in
    /// the menu (see [`MenuNumber`]). Calling it again for the key updates the row.
    pub menu_number: unsafe extern "C" fn(section: Str, key: Str, number: *const MenuNumber),
    /// Shows the switch `[section] key` on a menu page ("" = the main page)
    /// under `label` ("" = the key in title case).
    pub menu_switch: unsafe extern "C" fn(section: Str, key: Str, page: Str, label: Str),
    /// A list page (see [`MenuList`]): the menu lists `items`, each with a
    /// Shown/Hidden switch. Call it whenever the items change (every tick is
    /// fine); it fills in each item's `shown`.
    pub menu_list: unsafe extern "C" fn(list: *const MenuList, items: *mut MenuItem, count: usize),
}

/// A number row in the menu. Left/Right change it by `step`; a click opens it
/// for typing. The value is kept within `min..=max`.
#[repr(C)]
pub struct MenuNumber<'a> {
    /// Row name; "" = the key in title case.
    pub label: Str<'a>,
    /// Shown after the value, e.g. "%" or "m"; may be "".
    pub unit: Str<'a>,
    /// The menu page the row is on; "" = the main page.
    pub page: Str<'a>,
    /// A switch of the same section the number belongs to, or "". The row is
    /// listed right after that switch, and only while it is on.
    pub switch: Str<'a>,
    /// The switch's own row is hidden and this row stands in for it: it reads
    /// "Off" while the switch is off, and changing the number switches it on.
    /// The switch's hotkey still works.
    pub replaces_switch: bool,
    pub min: i64,
    pub max: i64,
    pub step: i64,
}

/// A menu page of items the plugin supplies.
#[repr(C)]
pub struct MenuList<'a> {
    /// The page's title; it opens from a row at the end of `parent` ("" = the main page).
    pub page: Str<'a>,
    pub parent: Str<'a>,
    /// Hidden items are remembered here, as `<item key> = false`; the key is
    /// the name in lower case with `_` for anything but letters and digits.
    pub config_section: Str<'a>,
}

#[repr(C)]
pub struct MenuItem<'a> {
    /// The text shown, e.g. "Shroud Liquid".
    pub name: Str<'a>,
    /// Shown dimmed beside it, e.g. the category; may be "".
    pub note: Str<'a>,
    /// How many there are now; the page lists the commonest first.
    pub count: u32,
    /// Filled in by the loader: whether the item is shown.
    pub shown: bool,
}

/// Safe wrapper plugins use instead of calling [`Api`] directly.
#[derive(Clone, Copy)]
pub struct Kit {
    api: &'static Api,
    name: &'static str,
}

impl Kit {
    /// # Safety
    /// `api` must be the pointer the loader passed to `shroudkit_plugin_init`.
    pub unsafe fn new(api: *const Api, name: &'static str) -> Option<Kit> {
        let api = unsafe { api.as_ref()? };
        (api.version == API_VERSION).then_some(Kit { api, name })
    }

    fn log(&self, level: Level, message: &str) {
        let line = format!("[{}] {}", self.name, message);
        unsafe { (self.api.log)(level, line.as_str().into()) }
    }

    pub fn info(&self, message: &str) {
        self.log(Level::Info, message)
    }

    pub fn warn(&self, message: &str) {
        self.log(Level::Warn, message)
    }

    pub fn error(&self, message: &str) {
        self.log(Level::Error, message)
    }

    pub fn module_base(&self) -> usize {
        unsafe { (self.api.module_base)() }
    }

    pub fn module_size(&self) -> usize {
        unsafe { (self.api.module_size)() }
    }

    pub fn is_server(&self) -> bool {
        unsafe { (self.api.is_server)() }
    }

    /// Address of the only 8-byte-aligned slot in the game image holding
    /// `value`, e.g. the table entry that points at a function.
    pub fn find_pointer(&self, what: &str, value: usize) -> Result<usize, String> {
        let (base, size) = (self.module_base(), self.module_size());
        // The image is one contiguous mapping; plugins run inside the game process.
        let image = unsafe { std::slice::from_raw_parts(base as *const u8, size) };
        let needle = value.to_le_bytes();

        let mut matches = image.chunks_exact(8).enumerate().filter(|(_, slot)| *slot == needle);
        match (matches.next(), matches.next()) {
            (Some((index, _)), None) => Ok(base + index * 8),
            (None, _) => Err(format!("{what}: no pointer to {value:#x} in the image")),
            _ => Err(format!("{what}: several pointers to {value:#x} in the image")),
        }
    }

    /// Address of the only match for `signature`. Zero or several matches are
    /// an error, so a patch never lands on a guess.
    pub fn find_unique(&self, what: &str, signature: &str) -> Result<usize, String> {
        let mut count = 0;
        let address = unsafe { (self.api.find_pattern)(signature.into(), &mut count) };
        match count {
            1 => {
                self.info(&format!("{what}: found at exe+{:#x}", address - self.module_base()));
                Ok(address)
            }
            0 => Err(format!("{what}: signature not found")),
            n => Err(format!("{what}: signature matches {n} places")),
        }
    }

    /// Update function of the game system called `name` (e.g. `oxygen_update`).
    ///
    /// Every system has a descriptor `{ name pointer, name length, update
    /// function, ... }` in the image. Names survive game updates, so this is a
    /// sturdier anchor than a byte signature.
    pub fn find_system(&self, name: &str) -> Result<usize, String> {
        let (base, size) = (self.module_base(), self.module_size());
        let image = unsafe { std::slice::from_raw_parts(base as *const u8, size) };
        let slot = |at: usize| usize::from_le_bytes(image[at..at + 8].try_into().unwrap());

        let mut needle = vec![0u8];
        needle.extend(name.as_bytes());
        needle.push(0);

        let mut found = None;
        for string in image.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle).map(|(at, _)| base + at + 1) {
            for at in (0..size.saturating_sub(24)).step_by(8) {
                let update = slot(at + 16);
                if slot(at) == string && slot(at + 8) == name.len() && (base..base + size).contains(&update) {
                    if found.is_some_and(|other| other != update) {
                        return Err(format!("system {name}: several descriptors"));
                    }
                    found = Some(update);
                }
            }
        }
        found.ok_or_else(|| format!("system {name}: not found"))
    }

    /// Patch that turns a game system into a no-op by making its update
    /// function return immediately. Created disabled.
    pub fn disable_system(&self, name: &str) -> Result<Patch, String> {
        let update = self.find_system(name)?;
        self.info(&format!("system {name}: update at exe+{:#x}", update - self.module_base()));
        self.patch(update, &[0xC3]) // ret
    }

    /// Byte patch at `address`, created disabled.
    pub fn patch(&self, address: usize, bytes: &[u8]) -> Result<Patch, String> {
        let handle = unsafe { (self.api.create_patch)(address, bytes.as_ptr(), bytes.len()) };
        if handle == 0 {
            return Err(format!("could not register a patch at {address:#x}"));
        }
        Ok(Patch { kit: *self, handle })
    }

    /// Replaces `overwritten` bytes at `address` (at least 5) with a jump to a
    /// code cave holding `body`, which then jumps back. `body` must reproduce
    /// whatever the overwritten instructions did and must be position independent.
    pub fn detour(&self, address: usize, overwritten: usize, body: &[u8]) -> Result<Patch, String> {
        if overwritten < 5 {
            return Err("a detour needs at least 5 bytes to overwrite".into());
        }

        let cave = unsafe { (self.api.alloc_near)(address, body.len() + 5) };
        if cave == 0 {
            return Err("no executable memory available near the patch site".into());
        }

        let mut code = body.to_vec();
        code.extend(jmp_rel32(cave + body.len(), address + overwritten));
        if !unsafe { (self.api.write_memory)(cave, code.as_ptr(), code.len()) } {
            return Err("could not write the code cave".into());
        }

        let mut entry = jmp_rel32(address, cave).to_vec();
        entry.resize(overwritten, 0x90);
        self.patch(address, &entry)
    }

    pub fn config_bool(&self, key: &str, default: bool) -> bool {
        unsafe { (self.api.config_bool)(self.name.into(), key.into(), default) }
    }

    pub fn config_int(&self, key: &str, default: i64) -> i64 {
        unsafe { (self.api.config_int)(self.name.into(), key.into(), default) }
    }

    pub fn set_config_bool(&self, key: &str, value: bool) {
        unsafe { (self.api.config_set_bool)(self.name.into(), key.into(), value) }
    }

    /// Hotkey bound to `key` in the `[<plugin>.keys]` section.
    pub fn config_key(&self, key: &str, default: &str) -> u32 {
        let section = format!("{}.keys", self.name);
        unsafe { (self.api.config_key)(section.as_str().into(), key.into(), default.into()) }
    }

    pub fn key_down(&self, virtual_key: u32) -> bool {
        virtual_key != 0 && unsafe { (self.api.key_down)(virtual_key) }
    }

    pub fn notify(&self, enabled: bool) {
        unsafe { (self.api.notify)(enabled) }
    }

    fn hud_set(&self, key: &str, state: u32) {
        unsafe { (self.api.hud_set)(self.name.into(), key.into(), state) }
    }

    pub fn set_brightness(&self, percent: u32) {
        unsafe { (self.api.set_brightness)(percent) }
    }

    /// True while this process hosts the game world (see [`Api::hosting`]).
    pub fn hosting(&self) -> bool {
        unsafe { (self.api.hosting)() }
    }

    /// Lists `key`, a whole number in this plugin's config section, in the menu.
    pub fn menu_number(&self, key: &str, number: &Number) {
        let raw = MenuNumber {
            label: number.label.into(),
            unit: number.unit.into(),
            page: number.page.into(),
            switch: number.switch.into(),
            replaces_switch: number.replaces_switch,
            min: number.min,
            max: number.max,
            step: number.step.max(1),
        };
        unsafe { (self.api.menu_number)(self.name.into(), key.into(), &raw) }
    }

    /// Shows the switch `key` on a menu `page` ("" = main) under `label` ("" = the key in title case).
    pub fn menu_switch(&self, key: &str, page: &str, label: &str) {
        unsafe { (self.api.menu_switch)(self.name.into(), key.into(), page.into(), label.into()) }
    }

    /// Hands a list page its items as (name, note, count) and returns whether each is shown.
    pub fn menu_list(&self, page: &str, parent: &str, config_section: &str, items: &[(&str, &str, u32)]) -> Vec<bool> {
        let list = MenuList { page: page.into(), parent: parent.into(), config_section: config_section.into() };
        let mut raw: Vec<MenuItem> =
            items.iter().map(|(name, note, count)| MenuItem { name: (*name).into(), note: (*note).into(), count: *count, shown: true }).collect();
        unsafe { (self.api.menu_list)(&list, raw.as_mut_ptr(), raw.len()) };
        raw.iter().map(|item| item.shown).collect()
    }

    pub fn write_memory(&self, address: usize, bytes: &[u8]) -> bool {
        unsafe { (self.api.write_memory)(address, bytes.as_ptr(), bytes.len()) }
    }

    /// Executable memory within rel32 range of `address`.
    pub fn alloc_near(&self, address: usize, size: usize) -> Result<usize, String> {
        match unsafe { (self.api.alloc_near)(address, size) } {
            0 => Err("no executable memory available near the patch site".into()),
            cave => Ok(cave),
        }
    }
}

/// A menu number row, for [`Kit::menu_number`]; see [`MenuNumber`] for the fields.
#[derive(Clone, Copy, Default)]
pub struct Number<'a> {
    pub label: &'a str,
    pub unit: &'a str,
    pub page: &'a str,
    pub switch: &'a str,
    pub replaces_switch: bool,
    pub min: i64,
    pub max: i64,
    pub step: i64,
}

/// What switching a feature on or off does.
pub enum Effect {
    /// Apply or remove these byte patches.
    Patches(Vec<Patch>),
    /// Write 1 or 0 to this byte, which injected code reads.
    Flag(usize),
    /// Nothing here: the plugin acts on [`FeatureSet::is_on`] itself.
    Managed,
}

impl Effect {
    fn apply(&self, kit: &Kit, enabled: bool) -> bool {
        match self {
            Effect::Patches(patches) => patches.iter().all(|patch| patch.set_enabled(enabled)),
            Effect::Flag(address) => kit.write_memory(*address, &[enabled as u8]),
            Effect::Managed => true,
        }
    }
}

/// Sets a feature up, or says why its code could not be located.
pub type PatchBuilder = fn(&Kit) -> Result<Effect, String>;

/// How often and for how long a feature whose code was not found is retried.
/// The Steam client is wrapped by a DRM stub (`.bind` section) that may only
/// reveal the game code after the loader has already started.
const RETRY_EVERY_TICKS: u32 = 40;
const RETRY_ATTEMPTS: u32 = 60;

/// A switch the player can flip: a config.ini boolean, an optional hotkey, and
/// the patches that implement it.
struct Feature {
    key: &'static str,
    default_hotkey: &'static str,
    builder: PatchBuilder,
    /// `None` until the builder has found the code.
    effect: Option<Effect>,
    attempts: u32,
    enabled: bool,
    /// Only applied while this process hosts the world; held back (paused) otherwise.
    host_only: bool,
    hotkey_held: bool,
    /// State last sent to the HUD.
    shown: Option<u32>,
}

/// The plugin's list of switches. Call [`FeatureSet::update`] from every tick:
/// config.ini stays the single source of truth, a hotkey just flips the value there.
pub struct FeatureSet {
    kit: Kit,
    features: Vec<Feature>,
    ticks: u32,
}

impl FeatureSet {
    pub fn new(kit: Kit) -> FeatureSet {
        FeatureSet { kit, features: Vec::new(), ticks: 0 }
    }

    pub fn add(&mut self, key: &'static str, default_hotkey: &'static str, builder: PatchBuilder) {
        let feature =
            Feature { key, default_hotkey, builder, effect: None, attempts: 0, enabled: false, host_only: false, hotkey_held: false, shown: None };
        self.features.push(feature);
    }

    /// Like [`FeatureSet::add`], for a feature that may only be applied while
    /// this process hosts the world. Switched on in the menus or while joined
    /// to someone else's server, it stays paused until hosting starts. (Turning
    /// a game system off on a client keeps it from joining or gets it dropped.)
    pub fn add_host_only(&mut self, key: &'static str, default_hotkey: &'static str, builder: PatchBuilder) {
        self.add(key, default_hotkey, builder);
        self.features.last_mut().unwrap().host_only = true;
    }

    /// Whether the feature is currently switched on.
    pub fn is_on(&self, key: &str) -> bool {
        self.features.iter().any(|feature| feature.key == key && feature.enabled)
    }

    pub fn update(&mut self) {
        let kit = self.kit;
        let hosting = kit.hosting();
        let retry_due = self.ticks % RETRY_EVERY_TICKS == 0;
        self.ticks = self.ticks.wrapping_add(1);

        for feature in &mut self.features {
            if feature.effect.is_none() && retry_due && feature.attempts < RETRY_ATTEMPTS {
                feature.attempts += 1;
                match (feature.builder)(&kit) {
                    Ok(effect) => feature.effect = Some(effect),
                    Err(reason) if feature.attempts == 1 => kit.warn(&format!("{reason}; retrying for a while")),
                    Err(reason) if feature.attempts == RETRY_ATTEMPTS => {
                        kit.error(&format!("{reason}; giving up, {} is unavailable in this game build", feature.key))
                    }
                    Err(_) => {}
                }
            }

            // Read both values even while unavailable so config.ini lists every option.
            let held = kit.key_down(kit.config_key(feature.key, feature.default_hotkey));
            let mut wanted = kit.config_bool(feature.key, false);
            let pressed = held && !feature.hotkey_held && feature.effect.is_some();
            feature.hotkey_held = held;
            if pressed {
                wanted = !wanted;
                kit.set_config_bool(feature.key, wanted);
            }
            let active = wanted && (!feature.host_only || hosting);

            let state = match (&feature.effect, feature.enabled) {
                (None, _) => 2,
                // Paused only when hosting is what holds it back, not for the frame before it is applied.
                (Some(_), false) if wanted && !active => 3,
                (Some(_), _) => active as u32,
            };
            if feature.shown != Some(state) {
                // (A feature that was on and is being paused now is logged by the apply below.)
                if state == 3 && !feature.enabled {
                    kit.info(&format!("{} paused: this PC is not hosting the world (menu, or joined to a server)", feature.key));
                }
                feature.shown = Some(state);
                kit.hud_set(feature.key, state);
            }

            let Some(effect) = &feature.effect else { continue };
            if pressed {
                kit.notify(wanted);
            }
            if active == feature.enabled {
                continue;
            }

            if effect.apply(&kit, active) {
                feature.enabled = active;
                kit.info(&format!("{} {}", feature.key, if active { "on" } else if wanted { "paused (off until this PC hosts the world)" } else { "off" }));
            } else {
                kit.error(&format!("{}: could not write the patch", feature.key));
            }
        }
    }
}

/// `jmp rel32` placed at `from`, landing on `to`.
pub fn jmp_rel32(from: usize, to: usize) -> [u8; 5] {
    let rel = (to as i64 - (from as i64 + 5)) as i32;
    let mut out = [0xE9, 0, 0, 0, 0];
    out[1..].copy_from_slice(&rel.to_le_bytes());
    out
}

#[derive(Clone, Copy)]
pub struct Patch {
    kit: Kit,
    handle: u32,
}

impl Patch {
    pub fn set_enabled(&self, enabled: bool) -> bool {
        unsafe { (self.kit.api.set_patch_enabled)(self.handle, enabled) }
    }
}

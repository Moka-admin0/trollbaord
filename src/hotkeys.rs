use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey, VK_DELETE, VK_END, VK_F1,
    VK_HOME, VK_INSERT, VK_NEXT, VK_NUMPAD0, VK_PAUSE, VK_PRIOR, VK_SCROLL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, PostThreadMessageW, WM_HOTKEY, WM_QUIT};

pub const SLOT_COUNT: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    Clip,
    PlayRandom,
    Stop,
    PlaySlot(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub action: HotkeyAction,
    pub hotkey: Hotkey,
    pub label: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub clip: String,
    pub random: String,
    pub stop: String,
    pub slot_mods: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self { clip: "F9".into(), random: "F8".into(), stop: "F10".into(), slot_mods: "ctrl+alt".into() }
    }
}

impl KeyConfig {
    pub fn bindings(&self) -> Result<Vec<Binding>> {
        let mut bindings = vec![
            Binding::parse(HotkeyAction::Clip, &self.clip)?,
            Binding::parse(HotkeyAction::PlayRandom, &self.random)?,
            Binding::parse(HotkeyAction::Stop, &self.stop)?,
        ];

        let modifiers = parse_modifiers(&self.slot_mods)?;
        for slot in 0..SLOT_COUNT {
            bindings.push(Binding {
                action: HotkeyAction::PlaySlot(slot),
                hotkey: Hotkey { modifiers, key: u32::from(b'1') + slot as u32 },
                label: self.slot_label(slot),
            });
        }
        Ok(bindings)
    }

    pub fn slot_label(&self, slot: usize) -> String {
        format_hotkey(&format!("{}+{}", self.slot_mods, slot + 1))
    }
}

impl Binding {
    fn parse(action: HotkeyAction, spec: &str) -> Result<Self> {
        Ok(Self { action, hotkey: parse_hotkey(spec)?, label: format_hotkey(spec) })
    }
}

pub fn format_hotkey(spec: &str) -> String {
    spec.split('+').map(str::trim).filter(|part| !part.is_empty()).map(capitalize).collect::<Vec<_>>().join("+")
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

fn parse_hotkey(spec: &str) -> Result<Hotkey> {
    let (modifiers, key) = parse_combo(spec)?;
    let key = key.with_context(|| format!("\"{spec}\" needs a key, not just modifiers"))?;
    Ok(Hotkey { modifiers, key })
}

fn parse_modifiers(spec: &str) -> Result<u32> {
    match parse_combo(spec)? {
        (0, None) => bail!("pick at least one modifier for the 1-9 keys (ctrl, alt, shift, win)"),
        (modifiers, None) => Ok(modifiers),
        (_, Some(_)) => bail!("\"{spec}\" should only contain modifiers (ctrl, alt, shift, win)"),
    }
}

fn parse_combo(spec: &str) -> Result<(u32, Option<u32>)> {
    let mut modifiers = 0;
    let mut key = None;
    for part in spec.split('+').map(|part| part.trim().to_ascii_lowercase()).filter(|part| !part.is_empty()) {
        match part.as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" => modifiers |= MOD_WIN,
            name if key.is_none() => {
                key = Some(virtual_key(name).with_context(|| format!("unknown key \"{name}\" in \"{spec}\""))?);
            }
            _ => bail!("\"{spec}\" has more than one key"),
        }
    }
    Ok((modifiers, key))
}

fn virtual_key(name: &str) -> Option<u32> {
    if let Some(number) = name.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&number).then(|| u32::from(VK_F1) + number - 1);
    }
    if let Some(digit) = name.strip_prefix("num").and_then(|n| n.trim_start_matches("pad").parse::<u32>().ok()) {
        return (digit <= 9).then(|| u32::from(VK_NUMPAD0) + digit);
    }
    if let [character] = name.as_bytes() {
        return character.is_ascii_alphanumeric().then(|| u32::from(character.to_ascii_uppercase()));
    }
    let key = match name {
        "pause" => VK_PAUSE,
        "insert" | "ins" => VK_INSERT,
        "delete" | "del" => VK_DELETE,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" | "pgup" => VK_PRIOR,
        "pagedown" | "pgdn" => VK_NEXT,
        "scrolllock" => VK_SCROLL,
        _ => return None,
    };
    Some(u32::from(key))
}

pub struct HotkeyThread {
    thread_id: u32,
    handle: Option<JoinHandle<()>>,
}

impl HotkeyThread {
    pub fn spawn(
        bindings: Vec<Binding>,
        mut on_press: impl FnMut(HotkeyAction) + Send + 'static,
    ) -> (Self, Vec<Binding>) {
        let (started, wait_for_start) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut registered = Vec::new();
            let mut failed = Vec::new();
            for (index, binding) in bindings.into_iter().enumerate() {
                let id = index as i32 + 1;
                if register(id, binding.hotkey) {
                    registered.push((id, binding.action));
                } else {
                    failed.push(binding);
                }
            }
            let _ = started.send((unsafe { GetCurrentThreadId() }, failed));

            while let Some(pressed_id) = wait_for_hotkey() {
                if let Some(&(_, action)) = registered.iter().find(|(id, _)| *id == pressed_id) {
                    on_press(action);
                }
            }
            for (id, _) in registered {
                unsafe { UnregisterHotKey(null_mut(), id) };
            }
        });

        let (thread_id, failed) = wait_for_start.recv().expect("hotkey thread stopped while starting");
        (Self { thread_id, handle: Some(handle) }, failed)
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn register(id: i32, hotkey: Hotkey) -> bool {
    unsafe { RegisterHotKey(null_mut(), id, hotkey.modifiers | MOD_NOREPEAT, hotkey.key) != 0 }
}

fn wait_for_hotkey() -> Option<i32> {
    let mut message: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {
        if message.message == WM_HOTKEY {
            return Some(message.wParam as i32);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_F9, VK_NUMPAD3};

    #[test]
    fn parses_single_keys_and_combos() {
        assert_eq!(parse_hotkey("F9").unwrap(), Hotkey { modifiers: 0, key: u32::from(VK_F9) });
        assert_eq!(
            parse_hotkey("Ctrl + Shift + p").unwrap(),
            Hotkey { modifiers: MOD_CONTROL | MOD_SHIFT, key: u32::from(b'P') }
        );
        assert_eq!(parse_hotkey("alt+numpad3").unwrap().key, u32::from(VK_NUMPAD3));
    }

    #[test]
    fn rejects_bad_hotkeys() {
        assert!(parse_hotkey("ctrl+alt").is_err());
        assert!(parse_hotkey("a+b").is_err());
        assert!(parse_hotkey("f25").is_err());
        assert!(parse_modifiers("ctrl+x").is_err());
        assert!(parse_modifiers("").is_err());
    }

    #[test]
    fn default_config_binds_every_action() {
        let bindings = KeyConfig::default().bindings().unwrap();
        assert_eq!(bindings.len(), 3 + SLOT_COUNT);
        assert_eq!(bindings.last().unwrap().label, "Ctrl+Alt+9");
        assert_eq!(bindings.last().unwrap().hotkey.key, u32::from(b'9'));
    }

    #[test]
    fn formats_hotkeys_for_display() {
        assert_eq!(format_hotkey("ctrl+ALT + f9"), "Ctrl+Alt+F9");
    }
}

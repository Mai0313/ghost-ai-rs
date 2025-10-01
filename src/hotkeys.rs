use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use parking_lot::Mutex;
use rdev::{listen, EventType, Key};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::HotkeyConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HotkeyAction {
    ToggleAskPanel,
    ToggleRecording,
    ToggleHidden,
    ClearSession,
    CaptureScreenshot,
}

#[derive(Clone, Debug)]
pub struct KeyCombo {
    tokens: Vec<String>,
}

impl KeyCombo {
    pub fn matches(&self, pressed: &HashSet<String>) -> bool {
        self.tokens.iter().all(|token| pressed.contains(token))
    }
}

#[derive(Clone, Debug)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub combo: KeyCombo,
}

pub struct HotkeyHandle {
    _thread: thread::JoinHandle<()>,
}

impl HotkeyHandle {
    pub fn spawn(bindings: Vec<HotkeyBinding>, tx: UnboundedSender<HotkeyAction>) -> Result<Self> {
        let combos = Arc::new(bindings);
        let sender = Arc::new(tx);
        let state = Arc::new(Mutex::new(HotkeyState::default()));

        let thread = thread::Builder::new().name("ghost-hotkeys").spawn({
            let combos = Arc::clone(&combos);
            let sender = Arc::clone(&sender);
            let state = Arc::clone(&state);
            move || {
                if let Err(err) = listen(move |event| {
                    handle_event(event.event_type, &combos, &sender, &state);
                }) {
                    log::error!("global hotkey listener failed: {err:?}");
                }
            }
        })?;

        Ok(Self { _thread: thread })
    }
}

#[derive(Default)]
struct HotkeyState {
    pressed: HashSet<String>,
    last_fired: HashMap<HotkeyAction, Instant>,
}

fn handle_event(
    event: EventType,
    bindings: &Arc<Vec<HotkeyBinding>>,
    sender: &Arc<UnboundedSender<HotkeyAction>>,
    state: &Arc<Mutex<HotkeyState>>,
) {
    match event {
        EventType::KeyPress(key) => {
            if let Some(token) = key_to_token(key) {
                let mut guard = state.lock();
                guard.pressed.insert(token.clone());
                for binding in bindings.iter() {
                    if binding.combo.matches(&guard.pressed) {
                        let should_fire = match guard.last_fired.get(&binding.action) {
                            Some(last) => last.elapsed() > Duration::from_millis(200),
                            None => true,
                        };
                        if should_fire {
                            guard.last_fired.insert(binding.action, Instant::now());
                            if sender.send(binding.action).is_err() {
                                log::warn!(
                                    "failed to deliver hotkey action {:?}; receiver dropped",
                                    binding.action
                                );
                            }
                        }
                    }
                }
            }
        }
        EventType::KeyRelease(key) => {
            if let Some(token) = key_to_token(key) {
                let mut guard = state.lock();
                guard.pressed.remove(&token);
            }
        }
        _ => {}
    }
}

fn key_to_token(key: Key) -> Option<String> {
    use Key::*;
    let token = match key {
        ControlLeft | ControlRight => "CTRL",
        ShiftLeft | ShiftRight => "SHIFT",
        Alt => "ALT",
        AltGr => "ALT_GR",
        MetaLeft | MetaRight => "META",
        Command => "META",
        Return | Enter => "ENTER",
        Escape => "ESCAPE",
        Space => "SPACE",
        Tab => "TAB",
        Backspace => "BACKSPACE",
        Delete => "DELETE",
        UpArrow => "UP",
        DownArrow => "DOWN",
        LeftArrow => "LEFT",
        RightArrow => "RIGHT",
        Home => "HOME",
        End => "END",
        PageUp => "PAGE_UP",
        PageDown => "PAGE_DOWN",
        PrintScreen => "PRINT_SCREEN",
        ScrollLock => "SCROLL_LOCK",
        Pause => "PAUSE",
        NumLock => "NUM_LOCK",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        Key0 => "0",
        Key1 => "1",
        Key2 => "2",
        Key3 => "3",
        Key4 => "4",
        Key5 => "5",
        Key6 => "6",
        Key7 => "7",
        Key8 => "8",
        Key9 => "9",
        KeyA => "A",
        KeyB => "B",
        KeyC => "C",
        KeyD => "D",
        KeyE => "E",
        KeyF => "F",
        KeyG => "G",
        KeyH => "H",
        KeyI => "I",
        KeyJ => "J",
        KeyK => "K",
        KeyL => "L",
        KeyM => "M",
        KeyN => "N",
        KeyO => "O",
        KeyP => "P",
        KeyQ => "Q",
        KeyR => "R",
        KeyS => "S",
        KeyT => "T",
        KeyU => "U",
        KeyV => "V",
        KeyW => "W",
        KeyX => "X",
        KeyY => "Y",
        KeyZ => "Z",
        Minus => "-",
        Equal => "=",
        LeftBracket => "[",
        RightBracket => "]",
        BackSlash => "\\",
        SemiColon => ";",
        Quote => "'",
        Comma => ",",
        Dot => ".",
        Slash => "/",
        BackQuote => "",
        _ => return None,
    };
    Some(token.to_string())
}

fn parse_token(token: &str) -> Option<String> {
    let normalized = token.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let upper = match normalized.as_str() {
        "ctrl" | "control" => "CTRL".to_string(),
        "shift" => "SHIFT".to_string(),
        "alt" => "ALT".to_string(),
        "altgr" | "alt-gr" => "ALT_GR".to_string(),
        "cmd" | "command" | "meta" | "super" | "win" => "META".to_string(),
        "enter" | "return" => "ENTER".to_string(),
        "esc" | "escape" => "ESCAPE".to_string(),
        "space" => "SPACE".to_string(),
        "tab" => "TAB".to_string(),
        "backspace" => "BACKSPACE".to_string(),
        "delete" => "DELETE".to_string(),
        "printscreen" | "print_screen" => "PRINT_SCREEN".to_string(),
        "pageup" | "page_up" => "PAGE_UP".to_string(),
        "pagedown" | "page_down" => "PAGE_DOWN".to_string(),
        "home" => "HOME".to_string(),
        "end" => "END".to_string(),
        "up" | "arrowup" | "uparrow" => "UP".to_string(),
        "down" | "arrowdown" | "downarrow" => "DOWN".to_string(),
        "left" | "arrowleft" | "leftarrow" => "LEFT".to_string(),
        "right" | "arrowright" | "rightarrow" => "RIGHT".to_string(),
        token if token.starts_with('f') && token.len() <= 3 => token.to_ascii_uppercase(),
        token if token.len() == 1 => token.to_ascii_uppercase(),
        _ => return None,
    };
    Some(upper)
}

pub fn parse_combo(text: &str) -> Option<KeyCombo> {
    let tokens: Vec<String> = text
        .split(['+', '-'])
        .filter_map(|part| parse_token(part))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(KeyCombo { tokens })
    }
}

pub fn bindings_from_config(cfg: &HotkeyConfig) -> Vec<HotkeyBinding> {
    let mut bindings = Vec::new();

    if let Some(combo) = parse_combo(&cfg.toggle_ask_panel) {
        bindings.push(HotkeyBinding {
            action: HotkeyAction::ToggleAskPanel,
            combo,
        });
    }
    if let Some(combo) = parse_combo(&cfg.toggle_record) {
        bindings.push(HotkeyBinding {
            action: HotkeyAction::ToggleRecording,
            combo,
        });
    }
    if let Some(combo) = parse_combo(&cfg.toggle_hide) {
        bindings.push(HotkeyBinding {
            action: HotkeyAction::ToggleHidden,
            combo,
        });
    }
    if let Some(combo) = parse_combo(&cfg.clear_session) {
        bindings.push(HotkeyBinding {
            action: HotkeyAction::ClearSession,
            combo,
        });
    }
    if let Some(combo) = parse_combo(&cfg.capture_screenshot) {
        bindings.push(HotkeyBinding {
            action: HotkeyAction::CaptureScreenshot,
            combo,
        });
    }

    bindings
}

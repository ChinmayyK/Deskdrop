use anyhow::{Context, Result};
use crate::protocol::MediaAction;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub fn dispatch_media_action(action: MediaAction) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default()).context("failed to initialize enigo")?;

    let key = match action {
        MediaAction::PlayPause => Key::MediaPlayPause,
        MediaAction::Next => Key::MediaNextTrack,
        MediaAction::Previous => Key::MediaPrevTrack,
        MediaAction::VolumeUp => Key::VolumeUp,
        MediaAction::VolumeDown => Key::VolumeDown,
        MediaAction::Mute => Key::VolumeMute,
    };

    enigo.key(key, Direction::Click).context("failed to simulate media key")?;

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn dispatch_media_action(_action: MediaAction) -> Result<()> {
    Ok(())
}

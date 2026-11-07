use crate::model::{validate_settings, Settings};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

const MAX_SETTINGS_BYTES: u64 = 2 * 1024 * 1024;

/// Invalid files are left untouched so a later save is always an explicit action.
pub fn load_settings(path: &Path) -> (Settings, Option<String>) {
    match read_settings(path) {
        Ok(Some(settings)) => (settings, None),
        Ok(None) => (Settings::default(), None),
        Err(error) => (
            Settings::default(),
            Some(format!("Saved settings could not be loaded. Defaults are in use. {error}")),
        ),
    }
}

fn read_settings(path: &Path) -> Result<Option<Settings>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut content = String::new();
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|error| error.to_string())?;
    if content.len() as u64 > MAX_SETTINGS_BYTES {
        return Err("The settings file exceeds 2 MB.".into());
    }
    let settings: Settings = serde_json::from_str(&content).map_err(|error| error.to_string())?;
    validate_settings(&settings)?;
    Ok(Some(settings))
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    validate_settings(settings)?;
    let serialized = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    if serialized.len() as u64 + 1 > MAX_SETTINGS_BYTES {
        return Err("The settings file would exceed 2 MB. Reduce the number or size of macros.".into());
    }
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| format!("Could not create the settings folder: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not prepare the settings file: {error}"))?;
    temporary.write_all(&serialized).map_err(|error| error.to_string())?;
    temporary.write_all(b"\n").map_err(|error| error.to_string())?;
    temporary.as_file().sync_all().map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| format!("Could not replace the settings file: {}", error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MacroStep;

    #[test]
    fn missing_file_returns_defaults_without_warning() {
        let directory = tempfile::tempdir().unwrap();
        let (settings, warning) = load_settings(&directory.path().join("settings.json"));
        assert_eq!(settings, Settings::default());
        assert!(warning.is_none());
    }

    #[test]
    fn malformed_or_invalid_settings_recover_without_overwriting_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        for content in ["{broken json", "null", "{\"version\":999}"] {
            std::fs::write(&path, content).unwrap();
            let (settings, warning) = load_settings(&path);
            assert_eq!(settings, Settings::default());
            assert!(warning.is_some());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[test]
    fn roundtrips_and_atomically_replaces_existing_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join("settings.json");
        save_settings(&path, &Settings::default()).unwrap();
        let mut updated = Settings::default();
        updated.active_profile_id = "everyday".into();
        updated.profiles[0].mappings[0].steps = vec![
            MacroStep::Key { keys: "Ctrl+F5".into() },
            MacroStep::Delay { ms: 500 },
            MacroStep::Text { text: "Hello, नमस्ते".into() },
        ];
        save_settings(&path, &updated).unwrap();
        let (loaded, warning) = load_settings(&path);
        assert_eq!(loaded, updated);
        assert!(warning.is_none());
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_save_preserves_last_valid_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        save_settings(&path, &Settings::default()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut invalid = Settings::default();
        invalid.profiles.clear();
        assert!(save_settings(&path, &invalid).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn disabled_repeat_delay_survives_save_and_reload() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let mut settings = Settings::default();
        settings.cooldown_ms = 0;
        save_settings(&path, &settings).unwrap();
        let (loaded, warning) = load_settings(&path);
        assert!(warning.is_none());
        assert_eq!(loaded.cooldown_ms, 0);
        assert_eq!(loaded, settings);
    }
}

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
            Some(format!(
                "Saved settings could not be loaded. Defaults are in use. {error}"
            )),
        ),
    }
}

pub fn read_settings(path: &Path) -> Result<Option<Settings>, String> {
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
        return Err(
            "The settings file would exceed 2 MB. Reduce the number or size of macros.".into(),
        );
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create the settings folder: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not prepare the settings file: {error}"))?;
    temporary
        .write_all(&serialized)
        .map_err(|error| error.to_string())?;
    temporary
        .write_all(b"\n")
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| format!("Could not replace the settings file: {}", error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MacroStep;

    #[test]
    fn loads_the_2_1_2_installer_migration_fixture() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profiles.json");
        let original = include_bytes!("../tests/fixtures/v2.1.2-profiles.json");
        std::fs::write(&path, original).unwrap();

        let (settings, warning) = load_settings(&path);
        assert!(warning.is_none(), "{warning:?}");
        assert_eq!(settings.active_profile_id, "migration-check");
        assert_eq!(settings.cooldown_ms, 420);
        assert_eq!(settings.accent_color, crate::model::AccentColor::Neutral);
        assert_eq!(
            settings.profiles[0].mappings[1].steps,
            [MacroStep::Text {
                text: "नमस्ते".into()
            }]
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

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
        let mut updated = Settings {
            active_profile_id: "everyday".into(),
            ..Settings::default()
        };
        updated.profiles[0].mappings[0].steps = vec![
            MacroStep::Key {
                keys: "Ctrl+F5".into(),
            },
            MacroStep::Delay { ms: 500 },
            MacroStep::Text {
                text: "Hello, नमस्ते".into(),
            },
        ];
        save_settings(&path, &updated).unwrap();
        let (loaded, warning) = load_settings(&path);
        assert_eq!(loaded, updated);
        assert!(warning.is_none());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
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
        let settings = Settings {
            cooldown_ms: 0,
            ..Settings::default()
        };
        save_settings(&path, &settings).unwrap();
        let (loaded, warning) = load_settings(&path);
        assert!(warning.is_none());
        assert_eq!(loaded.cooldown_ms, 0);
        assert_eq!(loaded, settings);
    }

    #[test]
    fn settings_read_accepts_exact_limit_and_rejects_one_extra_byte() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let settings = Settings::default();
        let mut content = serde_json::to_vec(&settings).unwrap();
        content.resize(MAX_SETTINGS_BYTES as usize, b' ');
        std::fs::write(&path, &content).unwrap();
        assert_eq!(read_settings(&path).unwrap(), Some(settings));

        content.push(b' ');
        std::fs::write(&path, &content).unwrap();
        assert!(read_settings(&path).unwrap_err().contains("exceeds 2 MB"));
        assert_eq!(std::fs::read(&path).unwrap(), content);
    }

    #[test]
    fn oversized_serialized_save_preserves_the_previous_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        save_settings(&path, &Settings::default()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut settings = Settings::default();
        let template = settings.profiles[0].clone();
        settings.profiles = (0..32)
            .map(|index| {
                let mut profile = template.clone();
                profile.id = format!("large-{index}");
                for mapping in &mut profile.mappings {
                    // JSON escaping makes this valid 2,000-character text step
                    // larger on disk than its in-memory UTF-8 representation.
                    mapping.steps = vec![MacroStep::Text {
                        text: "\u{0001}".repeat(2_000),
                    }];
                }
                profile
            })
            .collect();
        settings.active_profile_id = settings.profiles[0].id.clone();
        validate_settings(&settings).unwrap();
        assert!(serde_json::to_vec_pretty(&settings).unwrap().len() as u64 > MAX_SETTINGS_BYTES);
        assert!(save_settings(&path, &settings)
            .unwrap_err()
            .contains("exceed 2 MB"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_utf8_uses_defaults_without_replacing_original_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = [b'{', b'"', 0xFF, 0xFE, b'"', b'}'];
        std::fs::write(&path, original).unwrap();
        assert!(read_settings(&path).is_err());
        let (loaded, warning) = load_settings(&path);
        assert_eq!(loaded, Settings::default());
        assert!(warning.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    #[cfg(windows)]
    fn sharing_denied_replacement_preserves_old_file_and_cleans_temporary_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        save_settings(&path, &Settings::default()).unwrap();
        let before = std::fs::read(&path).unwrap();
        // FILE_SHARE_READ permits inspection but denies replacement/deletion.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let changed = Settings {
            theme: "dark".into(),
            ..Settings::default()
        };
        assert!(save_settings(&path, &changed)
            .unwrap_err()
            .contains("Could not replace"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        drop(lock);
        assert_eq!(read_settings(&path).unwrap(), Some(Settings::default()));
    }
}

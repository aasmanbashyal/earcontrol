use crate::shortcuts::parse_shortcut;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const TRIGGERS: [&str; 7] = [
    "play_pause",
    "next",
    "previous",
    "stop",
    "volume_up",
    "volume_down",
    "mute",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MacroStep {
    Key { keys: String },
    Delay { ms: u64 },
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub id: String,
    pub trigger: String,
    pub name: String,
    pub enabled: bool,
    pub steps: Vec<MacroStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub accent: String,
    pub mappings: Vec<Mapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub active_profile_id: String,
    pub profiles: Vec<Profile>,
    pub keyboard_fallback: bool,
    pub cooldown_ms: u64,
    pub theme: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetWindow {
    pub handle: isize,
    pub process_id: u32,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub transport: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: u64,
    pub timestamp: u64,
    pub trigger: String,
    pub source: String,
    pub outcome: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub mode: String,
    pub target: Option<TargetWindow>,
    pub running_macro: bool,
    pub media_ready: bool,
    pub message: String,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            mode: "idle".into(),
            target: None,
            running_macro: false,
            media_ready: false,
            message: "Choose a profile and check the headphone buttons in Monitor mode.".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: Settings,
    pub status: RuntimeStatus,
    pub version: String,
    pub warning: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 2,
            active_profile_id: "slides".into(),
            profiles: vec![
                presentation_profile(
                    "slides",
                    "Google Slides",
                    "Present with your hands free.",
                    "#a64619",
                ),
                presentation_profile(
                    "powerpoint",
                    "PowerPoint",
                    "Slide controls for your next talk.",
                    "#C46F48",
                ),
                Profile {
                    id: "everyday".into(),
                    name: "Everyday".into(),
                    description: "Navigation shortcuts for the app you choose.".into(),
                    accent: "#6476B5".into(),
                    mappings: vec![
                        mapping("everyday", "play_pause", "Press Space", true, "Space"),
                        mapping("everyday", "next", "Next page", true, "PageDown"),
                        mapping("everyday", "previous", "Previous page", true, "PageUp"),
                        mapping("everyday", "stop", "Escape", true, "Escape"),
                        mapping("everyday", "volume_up", "Volume up", false, "None"),
                        mapping("everyday", "volume_down", "Volume down", false, "None"),
                        mapping("everyday", "mute", "Mute", false, "None"),
                    ],
                },
            ],
            keyboard_fallback: true,
            cooldown_ms: 180,
            theme: "light".into(),
        }
    }
}

fn presentation_profile(id: &str, name: &str, description: &str, accent: &str) -> Profile {
    Profile {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        accent: accent.into(),
        mappings: vec![
            mapping(id, "play_pause", "Next slide", true, "Right"),
            mapping(id, "next", "Next slide", true, "Right"),
            mapping(id, "previous", "Previous slide", true, "Left"),
            mapping(id, "stop", "End presentation", true, "Escape"),
            mapping(id, "volume_up", "Volume up", false, "None"),
            mapping(id, "volume_down", "Volume down", false, "None"),
            mapping(id, "mute", "Mute", false, "None"),
        ],
    }
}

fn mapping(profile: &str, trigger: &str, name: &str, enabled: bool, keys: &str) -> Mapping {
    Mapping {
        id: format!("{profile}-{trigger}"),
        trigger: trigger.into(),
        name: name.into(),
        enabled,
        steps: if keys == "None" {
            Vec::new()
        } else {
            vec![MacroStep::Key { keys: keys.into() }]
        },
    }
}

/// Validates both loaded and edited settings before they can drive native input.
pub fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings.version != 2 {
        return Err("This settings version is not supported. Expected version 2.".into());
    }
    if settings.profiles.is_empty() || settings.profiles.len() > 32 {
        return Err("Keep between 1 and 32 profiles.".into());
    }
    if settings.cooldown_ms != 0 && !(80..=2_000).contains(&settings.cooldown_ms) {
        return Err("Repeat delay must be off or between 80 and 2000 milliseconds.".into());
    }
    if !["light", "dark", "system"].contains(&settings.theme.as_str()) {
        return Err("Choose the light, dark, or system theme.".into());
    }

    let mut profile_ids = HashSet::new();
    for profile in &settings.profiles {
        validate_identifier(&profile.id, "Profile ID")?;
        if !profile_ids.insert(&profile.id) {
            return Err("Each profile must have a unique ID.".into());
        }
        validate_name(&profile.name, "Profile name")?;
        if profile.description.chars().count() > 500 {
            return Err(format!(
                "The description for {} must be 500 characters or fewer.",
                profile.name
            ));
        }
        if profile.accent.len() != 7
            || !profile.accent.starts_with('#')
            || !profile.accent.as_bytes()[1..]
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            return Err(format!("Choose a valid color for {}.", profile.name));
        }
        if profile.mappings.len() != TRIGGERS.len() {
            return Err(format!(
                "{} must include all seven headphone commands. Unused commands can be disabled.",
                profile.name
            ));
        }
        let mut mapping_ids = HashSet::new();
        let mut triggers = HashSet::new();
        for mapping in &profile.mappings {
            validate_identifier(&mapping.id, "Mapping ID")?;
            if !mapping_ids.insert(&mapping.id) || !triggers.insert(&mapping.trigger) {
                return Err(format!(
                    "Each button in {} must have one unique mapping.",
                    profile.name
                ));
            }
            if !TRIGGERS.contains(&mapping.trigger.as_str()) {
                return Err(format!("Unknown headphone command: {}.", mapping.trigger));
            }
            validate_name(&mapping.name, "Action name")?;
            validate_macro(mapping)
                .map_err(|error| format!("{} / {}: {error}", profile.name, mapping.name))?;
        }
    }
    if !profile_ids.contains(&settings.active_profile_id) {
        return Err("Choose an existing profile as the active profile.".into());
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} must contain 1 to 128 bytes without control characters."
        ));
    }
    Ok(())
}

fn validate_name(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} must contain 1 to 80 characters without line breaks."
        ));
    }
    Ok(())
}

fn validate_macro(mapping: &Mapping) -> Result<(), String> {
    if mapping.steps.len() > 32 {
        return Err("Keep macros to 32 steps or fewer.".into());
    }
    if mapping.enabled && mapping.steps.is_empty() {
        return Err("Add a step before enabling this mapping.".into());
    }
    let mut total_delay = 0u64;
    let mut total_text = 0usize;
    for step in &mapping.steps {
        match step {
            MacroStep::Key { keys } => {
                parse_shortcut(keys)?;
            }
            MacroStep::Delay { ms } => {
                total_delay = total_delay
                    .checked_add(*ms)
                    .ok_or("The total delay is too long.")?;
                if total_delay > 30_000 {
                    return Err("The total delay in one macro must not exceed 30 seconds.".into());
                }
            }
            MacroStep::Text { text } => {
                total_text += text.chars().count();
                if total_text > 2_000 {
                    return Err("The text in one macro must total 2000 characters or fewer.".into());
                }
                if text.contains('\0') {
                    return Err("Text steps cannot contain a null character.".into());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_leave_volume_controls_disabled() {
        let settings = Settings::default();
        validate_settings(&settings).unwrap();
        assert_eq!(settings.active_profile_id, "slides");
        for profile in &settings.profiles {
            assert_eq!(profile.mappings.len(), 7);
            assert!(profile.mappings[..4].iter().all(|mapping| mapping.enabled));
            assert!(profile.mappings[4..].iter().all(|mapping| !mapping.enabled));
        }
        assert_eq!(
            settings.profiles[0].mappings[0].steps,
            vec![MacroStep::Key {
                keys: "Right".into()
            }]
        );
        assert_eq!(
            settings.profiles[0].mappings[2].steps,
            vec![MacroStep::Key {
                keys: "Left".into()
            }]
        );
    }

    #[test]
    fn json_contract_uses_camel_case_and_tagged_steps() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(value["activeProfileId"], "slides");
        assert_eq!(
            value["profiles"][0]["mappings"][0]["steps"][0],
            serde_json::json!({"kind":"key", "keys":"Right"})
        );
        assert!(value.get("active_profile_id").is_none());
        let roundtrip: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(roundtrip, Settings::default());
    }

    #[test]
    fn rejects_duplicate_triggers_and_missing_active_profile() {
        let mut settings = Settings::default();
        settings.profiles[0].mappings[1].trigger = "play_pause".into();
        assert!(validate_settings(&settings).is_err());
        let settings = Settings {
            active_profile_id: "missing".into(),
            ..Settings::default()
        };
        assert!(validate_settings(&settings).is_err());
    }

    #[test]
    fn imported_profiles_require_every_command_but_can_disable_them() {
        let mut exported = serde_json::to_value(Settings::default()).unwrap();
        exported["profiles"][0]["mappings"] = serde_json::json!([]);
        let empty: Settings = serde_json::from_value(exported).unwrap();
        let error = validate_settings(&empty).unwrap_err();
        assert!(error.contains("all seven headphone commands"));
        assert!(error.contains("Unused commands can be disabled"));

        for (missing, trigger) in TRIGGERS.iter().enumerate() {
            let mut settings = Settings::default();
            settings.profiles[0].mappings.remove(missing);
            assert!(
                validate_settings(&settings).is_err(),
                "Missing {} must be rejected",
                trigger
            );
        }

        let mut settings = Settings::default();
        settings.profiles[0].mappings[6].trigger = "unknown".into();
        assert!(validate_settings(&settings)
            .unwrap_err()
            .contains("Unknown headphone command"));

        let mut settings = Settings::default();
        for mapping in &mut settings.profiles[0].mappings {
            mapping.enabled = false;
            mapping.steps.clear();
        }
        settings.profiles[0].mappings.reverse();
        validate_settings(&settings).unwrap();
    }

    #[test]
    fn macro_validation_enforces_execution_limits() {
        let mut settings = Settings::default();
        let mapping = &mut settings.profiles[0].mappings[0];
        mapping.steps = vec![
            MacroStep::Delay { ms: 20_000 },
            MacroStep::Delay { ms: 10_001 },
        ];
        assert!(validate_settings(&settings).is_err());
        settings.profiles[0].mappings[0].steps = vec![MacroStep::Text {
            text: "x".repeat(2_001),
        }];
        assert!(validate_settings(&settings).is_err());
        settings.profiles[0].mappings[0].steps = vec![MacroStep::Key {
            keys: "Ctrl".into(),
        }];
        assert!(validate_settings(&settings).is_err());
        settings.profiles[0].mappings[0].steps = Vec::new();
        assert!(validate_settings(&settings).is_err());
        settings.profiles[0].mappings[0].enabled = false;
        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validates_settings_boundaries() {
        let mut settings = Settings {
            cooldown_ms: 0,
            ..Settings::default()
        };
        validate_settings(&settings).unwrap();
        settings.cooldown_ms = 79;
        assert!(validate_settings(&settings).is_err());
        settings.cooldown_ms = 2_001;
        assert!(validate_settings(&settings).is_err());
        settings.cooldown_ms = 80;
        settings.profiles[0].accent = "red; url(foo)".into();
        assert!(validate_settings(&settings).is_err());
    }
}

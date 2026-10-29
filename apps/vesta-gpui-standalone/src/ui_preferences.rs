//! Versioned host preferences for the standalone UI acceptance app.

use crate::localization::UiLocale;
use std::{env, fs, path::PathBuf};

const FORMAT: &str = "vesta-gpui-standalone-preferences";
const LEGACY_FORMAT: &str = "vesta-ui-standalone-preferences";
const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPreferences {
    pub drawer_width: f32,
    pub locale: UiLocale,
}

fn preferences_path() -> Option<PathBuf> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(
        base.join("Vesta")
            .join("ui-standalone")
            .join("preferences-v1.txt"),
    )
}

fn encode(preferences: UiPreferences) -> Result<String, String> {
    if !preferences.drawer_width.is_finite() {
        return Err("drawer width must be finite".to_owned());
    }
    Ok(format!(
        "{FORMAT}\nschema={SCHEMA_VERSION}\ndrawer_width={}\nlocale={}\n",
        preferences.drawer_width,
        preferences.locale.tag(),
    ))
}

fn decode(text: &str) -> Result<UiPreferences, String> {
    let mut lines = text.lines();
    if !matches!(lines.next(), Some(FORMAT) | Some(LEGACY_FORMAT)) {
        return Err("unrecognized preference format".to_owned());
    }
    let schema = lines
        .next()
        .and_then(|line| line.strip_prefix("schema="))
        .ok_or("missing preference schema")?
        .parse::<u32>()
        .map_err(|_| "invalid preference schema")?;
    if schema != 1 && schema != SCHEMA_VERSION {
        return Err(format!("unsupported preference schema {schema}"));
    }
    let drawer_width = lines
        .next()
        .and_then(|line| line.strip_prefix("drawer_width="))
        .ok_or("missing drawer width")?
        .parse::<f32>()
        .map_err(|_| "invalid drawer width")?;
    if !drawer_width.is_finite() {
        return Err("drawer width must be finite".to_owned());
    }
    let locale = if schema == 1 {
        UiLocale::EnUs
    } else {
        match lines
            .next()
            .and_then(|line| line.strip_prefix("locale="))
            .ok_or("missing locale")?
        {
            "en-US" => UiLocale::EnUs,
            "zh-CN" => UiLocale::ZhCn,
            _ => return Err("unsupported locale".to_owned()),
        }
    };
    if lines.next().is_some() {
        return Err("unknown preference field".to_owned());
    }
    Ok(UiPreferences {
        drawer_width,
        locale,
    })
}

pub fn load() -> Result<Option<UiPreferences>, String> {
    let Some(path) = preferences_path() else {
        return Ok(None);
    };
    let backup = path.with_extension("txt.bak");
    let mut errors = Vec::new();
    for candidate in [&path, &backup] {
        if !candidate.exists() {
            continue;
        }
        match fs::read_to_string(candidate)
            .map_err(|error| error.to_string())
            .and_then(|text| decode(&text))
        {
            Ok(preferences) => return Ok(Some(preferences)),
            Err(error) => errors.push(format!("{}: {error}", candidate.display())),
        }
    }
    if errors.is_empty() {
        Ok(None)
    } else {
        Err(errors.join("; "))
    }
}

pub fn save(preferences: UiPreferences) -> Result<(), String> {
    let Some(path) = preferences_path() else {
        return Ok(());
    };
    let text = encode(preferences)?;
    let parent = path
        .parent()
        .ok_or("UI preference path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("txt.tmp");
    let backup = path.with_extension("txt.bak");
    fs::write(&temporary, text).map_err(|error| error.to_string())?;
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| error.to_string())?;
        }
        fs::rename(&path, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_preferences_round_trip_and_reject_unknown_schema() {
        let preferences = UiPreferences {
            drawer_width: 236.5,
            locale: UiLocale::ZhCn,
        };
        assert_eq!(
            decode(&encode(preferences).expect("encode")),
            Ok(preferences)
        );
        assert_eq!(
            decode("vesta-ui-standalone-preferences\nschema=1\ndrawer_width=236.5\n"),
            Ok(UiPreferences {
                locale: UiLocale::EnUs,
                ..preferences
            }),
            "the package rename must not discard existing preferences"
        );
        assert_eq!(
            decode("vesta-gpui-standalone-preferences\nschema=3\ndrawer_width=236.5\n"),
            Err("unsupported preference schema 3".to_owned())
        );
        assert_eq!(
            decode("vesta-gpui-standalone-preferences\nschema=1\ndrawer_width=NaN\n"),
            Err("drawer width must be finite".to_owned())
        );
        assert!(
            decode("vesta-gpui-standalone-preferences\nschema=1\ndrawer_width=236.5\nextra=true\n")
                .is_err()
        );
    }
}

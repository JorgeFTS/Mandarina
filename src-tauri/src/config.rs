// User settings: the formats the wheel shows, the language and the color. See docs/Settings.md.
// They are saved in ~/.config/mandarina/config.json (macOS: ~/Library/Application Support/mandarina); with no file, or with invalid data, the usual
// lists from converters.rs are used.
use crate::converters::{type_targets, Kind};
use crate::lang::{Lang, LANGUAGES};
use crate::tr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// With fewer than 3 formats, once the file's own format is removed almost no segments would be left
pub const MIN_FORMATS: usize = 3;
// The wheel has 8 segments at most. Audio, video and document have 8 by default; since the format
// of the file itself is not shown, normally 7 are left and TOOLS or ZIP fits.
pub const MAX_FORMATS: usize = 8;

// Accent colors (the tones are in src/ajustes.js). The first one is the usual one.
pub const ACCENTS: [&str; 6] = ["orange", "red", "pink", "purple", "blue", "green"];

// Types with selectable formats (those with only 3 targets leave nothing to choose)
const KINDS: [(&str, Kind); 6] = [
    ("image", Kind::Image),
    ("audio", Kind::Audio),
    ("video", Kind::Video),
    ("pdf", Kind::Pdf),
    ("document", Kind::Document),
    ("spreadsheet", Kind::Spreadsheet),
];

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Config {
    // key of the type ("image", "audio"...) -> visible formats, in the order of the wheel
    #[serde(default)]
    formats: BTreeMap<String, Vec<String>>,
    // "en" or "es"; with no value it is decided by the system language
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    // one of ACCENTS; with no value, orange
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accent: Option<String>,
}

fn key_of(kind: Kind) -> Option<&'static str> {
    KINDS.iter().find(|(_, k)| *k == kind).map(|(key, _)| *key)
}

fn kind_of(key: &str) -> Option<Kind> {
    KINDS.iter().find(|(k, _)| *k == key).map(|(_, kind)| *kind)
}

impl Config {
    // Target formats of a type: the chosen ones (only those that exist and without repeats) or, if
    // nothing is chosen or there are not enough, the usual list
    pub(crate) fn visible(&self, kind: Kind) -> Vec<&'static str> {
        let all = type_targets(kind);
        let Some(saved) = key_of(kind).and_then(|key| self.formats.get(key)) else {
            return all.to_vec();
        };
        let mut list: Vec<&'static str> = Vec::new();
        for format in saved {
            if let Some(known) = all.iter().find(|f| **f == format.as_str()) {
                if !list.contains(known) {
                    list.push(known);
                }
            }
        }
        if list.len() < MIN_FORMATS || list.len() > MAX_FORMATS { all.to_vec() } else { list }
    }
}

// Validates what the interface sends: a known type, formats that exist for that type, no repeats,
// between MIN_FORMATS and MAX_FORMATS. Returns the clean list.
fn check(key: &str, formats: &[String]) -> Result<Vec<String>, String> {
    let kind = kind_of(key).ok_or_else(|| tr!("Unknown file type: {key}", "Tipo de archivo desconocido: {key}", key = key))?;
    let all = type_targets(kind);
    let mut list: Vec<String> = Vec::new();
    for format in formats {
        let format = format.trim().to_lowercase();
        if !all.contains(&format.as_str()) {
            return Err(tr!("Unknown format: {format}", "Formato desconocido: {format}", format = format));
        }
        if !list.contains(&format) {
            list.push(format);
        }
    }
    if list.len() < MIN_FORMATS || list.len() > MAX_FORMATS {
        let (min, max) = (MIN_FORMATS, MAX_FORMATS);
        return Err(tr!("Choose between {min} and {max} formats", "Elige entre {min} y {max} formatos", min = min, max = max));
    }
    Ok(list)
}

// "auto" (None), "en" or "es"
fn check_language(value: &str) -> Result<Option<String>, String> {
    match value {
        "auto" => Ok(None),
        _ if Lang::from_code(value).is_some() => Ok(Some(value.to_string())),
        _ => Err(tr!("Unknown language: {value}", "Idioma desconocido: {value}", value = value)),
    }
}

fn check_accent(value: &str) -> Result<String, String> {
    if ACCENTS.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(tr!("Unknown color: {value}", "Color desconocido: {value}", value = value))
    }
}

// ---------- Archivo ----------

fn config_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    if std::env::var_os("MANDARINA_CONFIG_DIR").is_none() {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let path = home.join("Library/Application Support/mandarina/config.json");
            migrate_legacy(&path, &home.join(".config/mandarina/config.json"));
            return path;
        }
    }
    let base = std::env::var_os("MANDARINA_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(|v| PathBuf::from(v).join("mandarina")))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("mandarina")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("config.json")
}

// macOS keeps settings in ~/Library/Application Support. A config saved by an earlier version in
// ~/.config/mandarina is copied there once, so the user's formats, language and color are kept.
#[cfg(any(target_os = "macos", test))]
fn migrate_legacy(path: &Path, legacy: &Path) {
    if path.exists() || !legacy.is_file() {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::copy(legacy, path);
}

// A missing or broken file must not stop the app from opening: the default values are used
fn load_from(path: &Path) -> Config {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

// Writes to a temporary file and renames it, so it is never left half written
fn save_to(path: &Path, config: &Config) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|e| tr!("Could not save the settings: {e}", "No se pudo guardar la configuración: {e}", e = e))?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| tr!("Could not save the settings: {e}", "No se pudo guardar la configuración: {e}", e = e))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| tr!("Could not save the settings: {e}", "No se pudo guardar la configuración: {e}", e = e))
}

// Saved language, so that lang.rs can use it at startup. It reads the file directly, without going through
// CURRENT: lang() can be called while CURRENT is locked and must not wait for that lock.
#[cfg_attr(test, allow(dead_code))] // in tests the language does not come from the file
pub(crate) fn saved_language() -> Option<String> {
    load_from(&config_path()).language.filter(|l| Lang::from_code(l).is_some())
}

// Settings in use: read from disk the first time and then kept in memory.
// In tests they are always the default values, so they do not depend on the machine.
static CURRENT: Mutex<Option<Config>> = Mutex::new(None);

pub(crate) fn current() -> Config {
    if cfg!(test) {
        return Config::default();
    }
    let mut guard = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(|| load_from(&config_path())).clone()
}

// Changes the settings and saves them; in memory it only stays if it could be saved
fn update(change: impl FnOnce(&mut Config)) -> Result<(), String> {
    let mut guard = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    let path = config_path();
    let mut config = guard.get_or_insert_with(|| load_from(&path)).clone();
    change(&mut config);
    save_to(&path, &config)?;
    *guard = Some(config);
    Ok(())
}

// ---------- Comandos ----------

// A type for the settings screen
#[derive(Serialize, Debug, PartialEq)]
pub struct KindFormats {
    key: &'static str,
    // all the possible formats, in the usual order
    all: Vec<&'static str>,
    // the ones shown now, in the order of the wheel
    enabled: Vec<&'static str>,
    min: usize,
    max: usize,
}

fn describe(config: &Config) -> Vec<KindFormats> {
    KINDS
        .iter()
        .map(|(key, kind)| KindFormats { key, all: type_targets(*kind).to_vec(), enabled: config.visible(*kind), min: MIN_FORMATS, max: MAX_FORMATS })
        .collect()
}

// Language and color for the settings screen and for painting the app at startup
#[derive(Serialize, Debug, PartialEq)]
pub struct Prefs {
    // "auto" or the code of a language ("en", "es", "pt", "fr")
    language: String,
    // available languages: code and name in their own language
    languages: Vec<LanguageInfo>,
    accent: String,
    accents: Vec<&'static str>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct LanguageInfo {
    code: &'static str,
    name: &'static str,
}

fn prefs_of(config: &Config) -> Prefs {
    Prefs {
        language: config.language.clone().unwrap_or_else(|| "auto".to_string()),
        languages: LANGUAGES.iter().map(|(code, name, _)| LanguageInfo { code, name }).collect(),
        accent: config.accent.clone().unwrap_or_else(|| ACCENTS[0].to_string()),
        accents: ACCENTS.to_vec(),
    }
}

#[tauri::command]
pub fn get_prefs() -> Prefs {
    prefs_of(&current())
}

// Saves the language and applies it immediately: lang::reload decides it again (the backend
// messages and output names already come out in the new language; the interface asks for get_language)
#[tauri::command]
pub fn set_language(value: String) -> Result<(), String> {
    let language = check_language(&value)?;
    update(|config| config.language = language)?;
    crate::lang::reload();
    Ok(())
}

#[tauri::command]
pub fn set_accent(value: String) -> Result<(), String> {
    let accent = check_accent(&value)?;
    update(|config| config.accent = Some(accent))
}

#[tauri::command]
pub fn get_config() -> Vec<KindFormats> {
    describe(&current())
}

#[tauri::command]
pub fn set_formats(kind: String, formats: Vec<String>) -> Result<(), String> {
    let list = check(&kind, &formats)?;
    update(|config| {
        config.formats.insert(kind, list);
    })
}

// Goes back to the usual list of that type
#[tauri::command]
pub fn reset_formats(kind: String) -> Result<(), String> {
    kind_of(&kind).ok_or_else(|| tr!("Unknown file type: {kind}", "Tipo de archivo desconocido: {kind}", kind = kind))?;
    update(|config| {
        config.formats.remove(&kind);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_la_config_antigua_una_vez() {
        let dir = std::env::temp_dir().join(format!("mandarina_mig_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let legacy = dir.join("old").join("config.json");
        let path = dir.join("new").join("sub").join("config.json");
        // Nothing to copy: nothing is created
        migrate_legacy(&path, &legacy);
        assert!(!path.exists());
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, r#"{"language":"es"}"#).unwrap();
        migrate_legacy(&path, &legacy);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"language":"es"}"#);
        // Once the new file exists the old one is no longer looked at
        std::fs::write(&legacy, r#"{"language":"fr"}"#).unwrap();
        migrate_legacy(&path, &legacy);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"language":"es"}"#);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn with(key: &str, list: &[&str]) -> Config {
        let mut config = Config::default();
        config.formats.insert(key.to_string(), strings(list));
        config
    }

    #[test]
    fn sin_configuracion_es_la_lista_de_siempre() {
        let config = Config::default();
        assert_eq!(config.visible(Kind::Image), type_targets(Kind::Image));
        assert_eq!(config.visible(Kind::Audio), type_targets(Kind::Audio));
        // Types that cannot be configured
        assert_eq!(config.visible(Kind::Slides), type_targets(Kind::Slides));
        assert_eq!(config.visible(Kind::Folder), type_targets(Kind::Folder));
    }

    #[test]
    fn usa_los_elegidos_en_su_orden() {
        let config = with("image", &["webp", "jpg", "png"]);
        assert_eq!(config.visible(Kind::Image), ["webp", "jpg", "png"]);
        // Another type does not change
        assert_eq!(config.visible(Kind::Audio), type_targets(Kind::Audio));
    }

    #[test]
    fn ignora_lo_que_no_vale_en_el_archivo() {
        // A format that does not exist and a repeated one are removed; 3 good ones are left
        let config = with("image", &["jpg", "xyz", "jpg", "png", "bmp"]);
        assert_eq!(config.visible(Kind::Image), ["jpg", "png", "bmp"]);
        // If fewer than 3 are left, the usual list is used
        assert_eq!(with("image", &["jpg", "xyz", "png"]).visible(Kind::Image), type_targets(Kind::Image));
        // The 8 default audio formats are valid
        let eight = with("audio", &type_targets(Kind::Audio));
        assert_eq!(eight.visible(Kind::Audio), type_targets(Kind::Audio));
    }

    #[test]
    fn valida_lo_que_manda_la_interfaz() {
        assert_eq!(check("audio", &strings(&["MP3", " wav ", "ogg"])).unwrap(), ["mp3", "wav", "ogg"]);
        // repeated ones: they are removed and count once
        assert_eq!(check("audio", &strings(&["mp3", "wav", "mp3", "ogg"])).unwrap(), ["mp3", "wav", "ogg"]);
        assert!(check("audio", &strings(&["mp3", "wav"])).is_err(), "pocos");
        // The 8 default audio formats are valid: that way the one that was removed can be checked again
        assert_eq!(check("audio", &strings(&["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"])).unwrap().len(), 8);
        assert!(check("audio", &strings(&["mp3", "wav", "jpg"])).is_err(), "formato de otro tipo");
        assert!(check("audio", &strings(&["mp3", "mp3", "mp3"])).is_err(), "repetidos no suman");
        assert!(check("slides", &strings(&["pdf", "jpg", "png"])).is_err(), "tipo sin elección");
        assert!(check("nada", &strings(&["a", "b", "c"])).is_err());
    }

    #[test]
    fn describe_todos_los_tipos() {
        let list = describe(&with("video", &["mp4", "webm", "gif"]));
        let keys: Vec<&str> = list.iter().map(|k| k.key).collect();
        assert_eq!(keys, ["image", "audio", "video", "pdf", "document", "spreadsheet"]);
        let video = list.iter().find(|k| k.key == "video").unwrap();
        assert_eq!(video.enabled, ["mp4", "webm", "gif"]);
        assert_eq!(video.all, type_targets(Kind::Video));
        assert_eq!((video.min, video.max), (3, 8));
        // Everything configurable really has more than MIN_FORMATS targets
        assert!(list.iter().all(|k| k.all.len() > MIN_FORMATS));
    }

    #[test]
    fn valida_idioma_y_color() {
        assert_eq!(check_language("auto").unwrap(), None);
        assert_eq!(check_language("es").unwrap(), Some("es".to_string()));
        assert_eq!(check_language("pt").unwrap(), Some("pt".to_string()));
        assert_eq!(check_language("fr").unwrap(), Some("fr".to_string()));
        assert!(check_language("de").is_err());
        assert!(check_language("").is_err());
        assert_eq!(check_accent("blue").unwrap(), "blue");
        assert!(check_accent("#ff0000").is_err());
        // With nothing saved: automatic and orange
        let prefs = prefs_of(&Config::default());
        assert_eq!((prefs.language.as_str(), prefs.accent.as_str()), ("auto", "orange"));
        assert_eq!(prefs.accents.len(), 6);
        let codes: Vec<&str> = prefs.languages.iter().map(|l| l.code).collect();
        assert_eq!(codes, ["en", "es", "pt", "fr"]);
        let config = Config { language: Some("es".into()), accent: Some("green".into()), ..Config::default() };
        let prefs = prefs_of(&config);
        assert_eq!((prefs.language.as_str(), prefs.accent.as_str()), ("es", "green"));
    }

    #[test]
    fn guarda_y_lee_el_archivo() {
        let dir = std::env::temp_dir().join(format!("mandarina_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub").join("config.json");
        // No file: default values
        assert_eq!(load_from(&path), Config::default());
        let mut config = with("pdf", &["docx", "txt", "md"]);
        config.language = Some("es".to_string());
        config.accent = Some("purple".to_string());
        save_to(&path, &config).unwrap();
        assert_eq!(load_from(&path), config);
        assert!(!path.with_extension("json.tmp").exists());
        // Broken file or with another shape: default values, no error
        std::fs::write(&path, "{ esto no es json").unwrap();
        assert_eq!(load_from(&path), Config::default());
        std::fs::write(&path, r#"{"formats": 5}"#).unwrap();
        assert_eq!(load_from(&path), Config::default());
        // A file without the field is also valid
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(load_from(&path), Config::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

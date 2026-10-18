// Ajustes del usuario: formatos que muestra la rueda, idioma y color. Ver docs/Configuracion.md.
// Se guardan en ~/.config/mandarina/config.json; sin archivo, o con datos que no valen, se usan
// las listas de siempre de converters.rs.
use crate::converters::{type_targets, Kind};
use crate::lang::{Lang, LANGUAGES};
use crate::tr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Con menos de 3 formatos, al quitar el del propio archivo casi no quedarían segmentos
pub const MIN_FORMATS: usize = 3;
// La rueda tiene 8 segmentos como máximo. Audio, vídeo y documento traen 8 de serie; como el formato
// del propio archivo no se muestra, normalmente quedan 7 y cabe HERRAMIENTAS o ZIP.
pub const MAX_FORMATS: usize = 8;

// Colores de acento (los tonos están en src/ajustes.js). El primero es el de siempre.
pub const ACCENTS: [&str; 6] = ["orange", "red", "pink", "purple", "blue", "green"];

// Tipos con formatos elegibles (los de solo 3 destinos no dejan nada que elegir)
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
    // clave del tipo ("image", "audio"...) -> formatos visibles, en el orden de la rueda
    #[serde(default)]
    formats: BTreeMap<String, Vec<String>>,
    // "en" o "es"; sin valor se decide por el idioma del sistema
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    // uno de ACCENTS; sin valor, el naranja
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
    // Formatos de destino de un tipo: los elegidos (solo los que existen y sin repetir) o, si no hay
    // nada elegido o no son suficientes, la lista de siempre
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

// Valida lo que manda la interfaz: tipo conocido, formatos que existen para ese tipo, sin repetir,
// entre MIN_FORMATS y MAX_FORMATS. Devuelve la lista limpia.
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

// "auto" (None), "en" o "es"
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
    let base = std::env::var_os("MANDARINA_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(|v| PathBuf::from(v).join("mandarina")))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("mandarina")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("config.json")
}

// Un archivo que falta o está estropeado no debe impedir abrir la app: se usan los valores por defecto
fn load_from(path: &Path) -> Config {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

// Escribe en un temporal y lo renombra, así nunca queda a medias
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

// Idioma guardado, para que lang.rs lo use al arrancar. Lee el archivo directamente, sin pasar por
// CURRENT: lang() puede llamarse mientras CURRENT está bloqueado y no debe esperar a ese bloqueo.
#[cfg_attr(test, allow(dead_code))] // en las pruebas el idioma no sale del archivo
pub(crate) fn saved_language() -> Option<String> {
    load_from(&config_path()).language.filter(|l| Lang::from_code(l).is_some())
}

// Configuración en uso: se lee del disco la primera vez y luego se guarda en memoria.
// En las pruebas siempre son los valores por defecto, para no depender de la máquina.
static CURRENT: Mutex<Option<Config>> = Mutex::new(None);

pub(crate) fn current() -> Config {
    if cfg!(test) {
        return Config::default();
    }
    let mut guard = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(|| load_from(&config_path())).clone()
}

// Cambia la configuración y la guarda; en memoria solo queda si se pudo guardar
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

// Un tipo para la pantalla de configuración
#[derive(Serialize, Debug, PartialEq)]
pub struct KindFormats {
    key: &'static str,
    // todos los formatos posibles, en el orden de siempre
    all: Vec<&'static str>,
    // los que se muestran ahora, en el orden de la rueda
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

// Idioma y color para la pantalla de ajustes y para pintar la app al arrancar
#[derive(Serialize, Debug, PartialEq)]
pub struct Prefs {
    // "auto" o el código de un idioma ("en", "es", "pt", "fr")
    language: String,
    // idiomas disponibles: código y nombre en su propio idioma
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

// Guarda el idioma y lo aplica al momento: lang::reload lo vuelve a decidir (los mensajes y los
// nombres de salida del backend ya salen en el idioma nuevo; la interfaz pide get_language)
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

// Vuelve a la lista de siempre de ese tipo
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
        // Tipos que no se pueden configurar
        assert_eq!(config.visible(Kind::Slides), type_targets(Kind::Slides));
        assert_eq!(config.visible(Kind::Folder), type_targets(Kind::Folder));
    }

    #[test]
    fn usa_los_elegidos_en_su_orden() {
        let config = with("image", &["webp", "jpg", "png"]);
        assert_eq!(config.visible(Kind::Image), ["webp", "jpg", "png"]);
        // Otro tipo no cambia
        assert_eq!(config.visible(Kind::Audio), type_targets(Kind::Audio));
    }

    #[test]
    fn ignora_lo_que_no_vale_en_el_archivo() {
        // Un formato que no existe y un repetido se quitan; quedan 3 buenos
        let config = with("image", &["jpg", "xyz", "jpg", "png", "bmp"]);
        assert_eq!(config.visible(Kind::Image), ["jpg", "png", "bmp"]);
        // Si quedan menos de 3 se usa la lista de siempre
        assert_eq!(with("image", &["jpg", "xyz", "png"]).visible(Kind::Image), type_targets(Kind::Image));
        // Los 8 de audio de serie valen
        let eight = with("audio", &type_targets(Kind::Audio));
        assert_eq!(eight.visible(Kind::Audio), type_targets(Kind::Audio));
    }

    #[test]
    fn valida_lo_que_manda_la_interfaz() {
        assert_eq!(check("audio", &strings(&["MP3", " wav ", "ogg"])).unwrap(), ["mp3", "wav", "ogg"]);
        // repetidos: se quitan y cuentan una vez
        assert_eq!(check("audio", &strings(&["mp3", "wav", "mp3", "ogg"])).unwrap(), ["mp3", "wav", "ogg"]);
        assert!(check("audio", &strings(&["mp3", "wav"])).is_err(), "pocos");
        // Los 8 formatos de serie de audio son válidos: así se puede volver a marcar el que se quitó
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
        // Todo lo configurable tiene de verdad más de MIN_FORMATS destinos
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
        // Sin nada guardado: automático y naranja
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
        // Sin archivo: valores por defecto
        assert_eq!(load_from(&path), Config::default());
        let mut config = with("pdf", &["docx", "txt", "md"]);
        config.language = Some("es".to_string());
        config.accent = Some("purple".to_string());
        save_to(&path, &config).unwrap();
        assert_eq!(load_from(&path), config);
        assert!(!path.with_extension("json.tmp").exists());
        // Archivo estropeado o con otra forma: valores por defecto, sin error
        std::fs::write(&path, "{ esto no es json").unwrap();
        assert_eq!(load_from(&path), Config::default());
        std::fs::write(&path, r#"{"formats": 5}"#).unwrap();
        assert_eq!(load_from(&path), Config::default());
        // Un archivo sin el campo también vale
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(load_from(&path), Config::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

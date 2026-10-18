// App language: English (default), Spanish, Portuguese and French. See docs/Languages.md.
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::Mutex;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Lang {
    En,
    Es,
    Pt,
    Fr,
}

// Available languages: code and name in their own language. The code is what is saved in the
// settings, what get_language returns and the start of LANG ("pt_BR.UTF-8" -> "pt").
pub const LANGUAGES: [(&str, &str, Lang); 4] =
    [("en", "English", Lang::En), ("es", "Español", Lang::Es), ("pt", "Português", Lang::Pt), ("fr", "Français", Lang::Fr)];

impl Lang {
    pub fn code(self) -> &'static str {
        LANGUAGES.iter().find(|(_, _, lang)| *lang == self).map(|(code, _, _)| *code).unwrap_or("en")
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        LANGUAGES.iter().find(|(c, _, _)| *c == code).map(|(_, _, lang)| *lang)
    }
}

// ---------- Stable error codes (they are not translated) ----------
// Two errors have a special meaning for the interface and for the internal logic. Since the
// text changes with the language, the error string starts with a code in square brackets, a space
// and the translated text:
// "[cancelled] Cancelled"  / "[cancelled] Cancelado"        (the user cancelled)
// "[optimized] Already optimized" / "[optimized] Ya está optimizado"  (reducing the size saves nothing)
// To recognize them `has_code` is used, never the translated text.
pub const CANCELLED: &str = "[cancelled]";
pub const OPTIMIZED: &str = "[optimized]";

// Error with a code: "[code] text"
pub fn coded(code: &str, text: &str) -> String {
    format!("{code} {text}")
}

// true if the error message starts with that code
pub fn has_code(message: &str, code: &str) -> bool {
    message.starts_with(code)
}

pub fn cancelled_error() -> String {
    coded(CANCELLED, &tr("Cancelled", "Cancelado"))
}

pub fn optimized_error() -> String {
    coded(OPTIMIZED, &tr("Already optimized", "Ya está optimizado"))
}

// ---------- Detection ----------

// Environment variables that decide the language, in order of priority
const VARIABLES: [&str; 5] = ["MANDARINA_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"];

// Pure function: `read` is the "environment reader" (in the app it reads the real variables and in tests
// a table). The first variable that exists and is not empty decides: if its value starts with "es"
// (case-insensitive, e.g. "es_PE.UTF-8") it is Spanish; any other value ("C", "en_US",
// "fr"...) is English. In LANGUAGE only the first language of the list counts ("es:en" -> "es").
// If there is no variable, English.
pub fn detect(read: impl Fn(&str) -> Option<String>) -> Lang {
    for name in VARIABLES {
        let Some(value) = read(name) else {
            continue;
        };
        let value = if name == "LANGUAGE" { value.split(':').next().unwrap_or("").to_string() } else { value };
        let value = value.trim().to_lowercase();
        if value.is_empty() {
            continue;
        }
        // The first one whose code matches the start of the value ("pt_BR.UTF-8" -> pt)
        return LANGUAGES.iter().find(|(code, _, _)| value.starts_with(code)).map(|(_, _, lang)| *lang).unwrap_or(Lang::En);
    }
    Lang::En
}

// Active language. In the app it is detected on first use with the real variables and decided again
// with `reload` when the user changes it in the settings (so it changes without restarting).
#[cfg(not(test))]
static ACTIVE: Mutex<Option<Lang>> = Mutex::new(None);

// MANDARINA_LANG is the first variable detect looks at: if it is not in the environment, the language
// saved in the settings takes its place, so it wins over the system language
// (if MANDARINA_LANG is set, it wins and changing the language in the settings has no visible effect)
#[cfg(not(test))]
fn detect_now() -> Lang {
    detect(|name| std::env::var(name).ok().or_else(|| if name == "MANDARINA_LANG" { crate::config::saved_language() } else { None }))
}

#[cfg(not(test))]
pub fn lang() -> Lang {
    let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    *active.get_or_insert_with(detect_now)
}

// Decides the language again (after saving the one from the settings). It is computed before taking the lock.
#[cfg(not(test))]
pub fn reload() {
    let lang = detect_now();
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(lang);
}

// In tests the language is set by each thread with set_lang: there is nothing to reload
#[cfg(test)]
pub fn reload() {}

// In tests the language is per thread: each test runs in its own thread, so `set_lang`
// in one does not affect the others even if they run in parallel (a Mutex would force them to be serialized).
// Without `set_lang` the language is English, without looking at the environment of whoever runs the tests.
#[cfg(test)]
thread_local! {
    static TEST_LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

#[cfg(test)]
pub fn lang() -> Lang {
    TEST_LANG.with(|l| l.get())
}

// Tests only: sets the language of the current thread
#[cfg(test)]
pub fn set_lang(lang: Lang) {
    TEST_LANG.with(|l| l.set(lang));
}

// ---------- Catalogs ----------
// English and Spanish are written in each call to tr/tr!. The other languages are in files
// locales/<code>.json: { "English text": "translation" }. The key is the English text exactly as it
// is in the code, with its {placeholders}; the translation carries the same placeholders. A text that
// is missing from the catalog comes out in English. To add a language see docs/Languages.md.
const CATALOGS: [(Lang, &str); 2] = [
    (Lang::Pt, include_str!("../locales/pt.json")),
    (Lang::Fr, include_str!("../locales/fr.json")),
];

pub(crate) fn catalog(lang: Lang) -> &'static HashMap<String, String> {
    static LOADED: OnceLock<Vec<(Lang, HashMap<String, String>)>> = OnceLock::new();
    let loaded = LOADED.get_or_init(|| {
        CATALOGS.iter().map(|(lang, text)| (*lang, serde_json::from_str(text).unwrap_or_default())).collect()
    });
    static EMPTY: OnceLock<HashMap<String, String>> = OnceLock::new();
    match loaded.iter().find(|(l, _)| *l == lang) {
        Some((_, map)) => map,
        None => EMPTY.get_or_init(HashMap::new),
    }
}

// Translation of an English text into a language with a catalog (or the same text if it is not there)
pub fn translate(lang: Lang, en: &str) -> String {
    catalog(lang).get(en).cloned().unwrap_or_else(|| en.to_string())
}

// Replaces the {name} placeholders of a template with their values (languages with a catalog cannot use
// format!, because their text is not known at compile time)
pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut text = template.to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

// Picks the text according to the language: tr("Cancelled", "Cancelado")
pub fn tr(en: &str, es: &str) -> String {
    match lang() {
        Lang::En => en.to_string(),
        Lang::Es => es.to_string(),
        other => translate(other, en),
    }
}

// Same as tr but with formatting; the two texts use the same placeholders and each value goes with its
// name (languages with a catalog need them to fill in their translation):
// tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = n, format = f)
#[macro_export]
macro_rules! tr {
    ($en:literal, $es:literal $(, $name:ident = $value:expr)* $(,)?) => {
        match $crate::lang::lang() {
            $crate::lang::Lang::En => format!($en $(, $name = $value)*),
            $crate::lang::Lang::Es => format!($es $(, $name = $value)*),
            other => $crate::lang::fill(&$crate::lang::translate(other, $en), &[$((stringify!($name), $value.to_string())),*]),
        }
    };
}

#[tauri::command]
pub fn get_language() -> String {
    lang().code().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fake environment reader with the given variables
    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    #[test]
    fn sin_variables_es_ingles() {
        assert_eq!(detect(env(&[])), Lang::En);
    }

    #[test]
    fn cada_variable_se_lee() {
        for name in ["MANDARINA_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
            assert_eq!(detect(env(&[(name, "es_PE.UTF-8")])), Lang::Es, "{name}");
            assert_eq!(detect(env(&[(name, "en_US.UTF-8")])), Lang::En, "{name}");
        }
    }

    #[test]
    fn valores_de_ejemplo() {
        let lang = |v: &str| detect(env(&[("LANG", v)]));
        assert_eq!(lang("es_PE.UTF-8"), Lang::Es);
        assert_eq!(lang("es"), Lang::Es);
        assert_eq!(lang("en_US"), Lang::En);
        assert_eq!(lang("C"), Lang::En);
        assert_eq!(lang("POSIX"), Lang::En);
        assert_eq!(lang(""), Lang::En);
        assert_eq!(lang("fr_FR.UTF-8"), Lang::Fr);
        assert_eq!(lang("pt_BR.UTF-8"), Lang::Pt);
        assert_eq!(lang("pt"), Lang::Pt);
        assert_eq!(lang("de_DE.UTF-8"), Lang::En);
        assert_eq!(lang("zh_CN.UTF-8"), Lang::En);
        // Uppercase
        assert_eq!(lang("ES_PE"), Lang::Es);
        assert_eq!(lang("Es"), Lang::Es);
    }

    #[test]
    fn lista_de_language() {
        let lang = |v: &str| detect(env(&[("LANGUAGE", v)]));
        assert_eq!(lang("es:en"), Lang::Es);
        assert_eq!(lang("en:es"), Lang::En);
        assert_eq!(lang("es_PE:es:en"), Lang::Es);
    }

    #[test]
    fn el_orden_de_prioridad() {
        // MANDARINA_LANG wins over everything
        let all_es = [("LC_ALL", "es"), ("LC_MESSAGES", "es"), ("LANGUAGE", "es"), ("LANG", "es")];
        let mut vars = vec![("MANDARINA_LANG", "en")];
        vars.extend(all_es);
        assert_eq!(detect(env(&vars)), Lang::En);
        let all_en = [("LC_ALL", "en"), ("LC_MESSAGES", "en"), ("LANGUAGE", "en"), ("LANG", "en")];
        let mut vars = vec![("MANDARINA_LANG", "es")];
        vars.extend(all_en);
        assert_eq!(detect(env(&vars)), Lang::Es);
        // Then LC_ALL, LC_MESSAGES, LANGUAGE and LANG
        assert_eq!(detect(env(&[("LC_ALL", "en_US"), ("LC_MESSAGES", "es"), ("LANGUAGE", "es"), ("LANG", "es")])), Lang::En);
        assert_eq!(detect(env(&[("LC_MESSAGES", "en_US"), ("LANGUAGE", "es"), ("LANG", "es")])), Lang::En);
        assert_eq!(detect(env(&[("LANGUAGE", "en:es"), ("LANG", "es_PE")])), Lang::En);
        assert_eq!(detect(env(&[("LANGUAGE", "es:en"), ("LANG", "en_US")])), Lang::Es);
        // An empty variable does not decide: it moves on to the next one
        assert_eq!(detect(env(&[("LC_ALL", ""), ("LANG", "es_PE")])), Lang::Es);
        // "C" in LC_ALL forces English even if LANG is Spanish
        assert_eq!(detect(env(&[("LC_ALL", "C"), ("LANG", "es_PE")])), Lang::En);
    }

    #[test]
    fn tr_elige_el_idioma() {
        set_lang(Lang::En);
        assert_eq!(tr("Hello", "Hola"), "Hello");
        assert_eq!(get_language(), "en");
        set_lang(Lang::Es);
        assert_eq!(tr("Hello", "Hola"), "Hola");
        assert_eq!(get_language(), "es");
    }

    #[test]
    fn tr_con_formato() {
        let (name, format) = ("a.png", "docx");
        set_lang(Lang::En);
        assert_eq!(tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = name, format = format), "Cannot convert a.png to docx");
        set_lang(Lang::Es);
        assert_eq!(tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = name, format = format), "No se puede convertir a.png a docx");
        let n = 5;
        assert_eq!(tr!("Value {n}", "Valor {n}", n = n), "Valor 5");
    }

    // All the English texts of the calls to tr and tr! in the code (without the tests)
    fn english_texts() -> Vec<String> {
        let mut texts = Vec::new();
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let code = source.split("#[cfg(test)]\nmod tests").next().unwrap();
            let bytes: Vec<char> = code.chars().collect();
            let mut i = 0;
            while i + 3 < bytes.len() {
                // "tr(" or "tr!(" that is not the end of another name, followed by a quoted text
                let start = if bytes[i] == 't' && bytes[i + 1] == 'r' && (i == 0 || !(bytes[i - 1].is_alphanumeric() || bytes[i - 1] == '_')) {
                    match (bytes[i + 2], bytes[i + 3]) {
                        ('(', _) => Some(i + 3),
                        ('!', '(') => Some(i + 4),
                        _ => None,
                    }
                } else {
                    None
                };
                i += 1;
                let Some(mut j) = start else { continue };
                while j < bytes.len() && bytes[j].is_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != '"' {
                    continue;
                }
                let mut text = String::new();
                j += 1;
                while j < bytes.len() && bytes[j] != '"' {
                    if bytes[j] == '\\' {
                        j += 1;
                    }
                    text.push(bytes[j]);
                    j += 1;
                }
                texts.push(text);
            }
        }
        texts
    }

    fn placeholders(text: &str) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            names.push(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        names.sort();
        names
    }

    #[test]
    fn los_catalogos_tienen_todos_los_textos() {
        let texts = english_texts();
        assert!(texts.len() > 100, "no se encontraron los textos del código: {}", texts.len());
        for (code, _, lang) in LANGUAGES {
            if lang == Lang::En || lang == Lang::Es {
                continue;
            }
            let map = catalog(lang);
            for text in &texts {
                assert!(map.contains_key(text), "falta la traducción '{code}' de: {text}");
            }
            // And none is left over: a text that no longer exists in the code is a leftover
            for key in map.keys() {
                assert!(texts.contains(key), "sobra en '{code}' (ya no está en el código): {key}");
            }
        }
    }

    #[test]
    fn las_traducciones_llevan_los_mismos_marcadores() {
        for (code, _, lang) in LANGUAGES {
            for (key, translation) in catalog(lang) {
                assert_eq!(placeholders(key), placeholders(translation), "'{code}': {key} -> {translation}");
                assert!(!translation.trim().is_empty(), "'{code}' vacía: {key}");
            }
        }
    }

    #[test]
    fn tr_en_portugues_y_frances() {
        set_lang(Lang::Pt);
        assert_eq!(tr("Cancelled", "Cancelado"), "Cancelado");
        assert_eq!(tr("Already optimized", "Ya está optimizado"), "Já otimizado");
        assert_eq!(get_language(), "pt");
        let (name, format) = ("a.png", "docx");
        assert_eq!(
            tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = name, format = format),
            "Não é possível converter a.png para docx"
        );
        set_lang(Lang::Fr);
        assert_eq!(tr("Already optimized", "Ya está optimizado"), "Déjà optimisé");
        assert_eq!(get_language(), "fr");
        assert_eq!(
            tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = name, format = format),
            "Impossible de convertir a.png en docx"
        );
        // A text that is not in the catalog comes out in English, without breaking anything
        assert_eq!(tr("Not in the catalog", "No está en el catálogo"), "Not in the catalog");
        set_lang(Lang::En);
    }

    #[test]
    fn codigos_de_idioma() {
        for (code, _, lang) in LANGUAGES {
            assert_eq!(Lang::from_code(code), Some(lang));
            assert_eq!(lang.code(), code);
        }
        assert_eq!(Lang::from_code("de"), None);
        assert_eq!(Lang::from_code("auto"), None);
    }

    #[test]
    fn errores_con_codigo() {
        set_lang(Lang::En);
        assert_eq!(cancelled_error(), "[cancelled] Cancelled");
        assert_eq!(optimized_error(), "[optimized] Already optimized");
        set_lang(Lang::Es);
        assert_eq!(cancelled_error(), "[cancelled] Cancelado");
        assert_eq!(optimized_error(), "[optimized] Ya está optimizado");
        assert!(has_code(&cancelled_error(), CANCELLED));
        assert!(!has_code(&cancelled_error(), OPTIMIZED));
        assert!(has_code(&optimized_error(), OPTIMIZED));
        assert!(!has_code("Cancelado", CANCELLED));
    }
}

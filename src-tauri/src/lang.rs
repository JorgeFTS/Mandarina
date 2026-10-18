// Idioma de la app: inglés (por defecto), español, portugués y francés. Ver docs/Idiomas.md.
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

// Idiomas disponibles: código y nombre en su propio idioma. El código es lo que se guarda en los
// ajustes, lo que devuelve get_language y el principio de LANG ("pt_BR.UTF-8" -> "pt").
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

// ---------- Códigos estables de error (no se traducen) ----------
// Dos errores tienen un significado especial para la interfaz y para la lógica interna. Como el
// texto cambia con el idioma, la cadena de error empieza por un código entre corchetes, un espacio
// y el texto traducido:
//   "[cancelled] Cancelled"  / "[cancelled] Cancelado"        (el usuario canceló)
//   "[optimized] Already optimized" / "[optimized] Ya está optimizado"  (comprimir no ahorra nada)
// Para reconocerlos se usa `has_code`, nunca el texto traducido.
pub const CANCELLED: &str = "[cancelled]";
pub const OPTIMIZED: &str = "[optimized]";

// Error con código: "[código] texto"
pub fn coded(code: &str, text: &str) -> String {
    format!("{code} {text}")
}

// true si el mensaje de error empieza por ese código
pub fn has_code(message: &str, code: &str) -> bool {
    message.starts_with(code)
}

pub fn cancelled_error() -> String {
    coded(CANCELLED, &tr("Cancelled", "Cancelado"))
}

pub fn optimized_error() -> String {
    coded(OPTIMIZED, &tr("Already optimized", "Ya está optimizado"))
}

// ---------- Detección ----------

// Variables de entorno que deciden el idioma, en orden de prioridad
const VARIABLES: [&str; 5] = ["MANDARINA_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"];

// Función pura: `read` es el "lector de entorno" (en la app lee las variables reales y en las pruebas
// una tabla). Decide la primera variable que exista y no esté vacía: si su valor empieza por "es"
// (sin distinguir mayúsculas, p. ej. "es_PE.UTF-8") es español; cualquier otro valor ("C", "en_US",
// "fr"...) es inglés. En LANGUAGE solo cuenta el primer idioma de la lista ("es:en" -> "es").
// Si no hay ninguna variable, inglés.
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
        // El primero cuyo código coincide con el principio del valor ("pt_BR.UTF-8" -> pt)
        return LANGUAGES.iter().find(|(code, _, _)| value.starts_with(code)).map(|(_, _, lang)| *lang).unwrap_or(Lang::En);
    }
    Lang::En
}

// Idioma activo. En la app se detecta al primer uso con las variables reales y se vuelve a decidir
// con `reload` cuando el usuario lo cambia en los ajustes (así cambia sin reiniciar).
#[cfg(not(test))]
static ACTIVE: Mutex<Option<Lang>> = Mutex::new(None);

// MANDARINA_LANG es la primera variable que mira detect: si no está en el entorno, ocupa su
// lugar el idioma guardado en los ajustes, que así manda sobre el idioma del sistema
// (si MANDARINA_LANG sí está, manda ella y cambiar el idioma en los ajustes no se nota)
#[cfg(not(test))]
fn detect_now() -> Lang {
    detect(|name| std::env::var(name).ok().or_else(|| if name == "MANDARINA_LANG" { crate::config::saved_language() } else { None }))
}

#[cfg(not(test))]
pub fn lang() -> Lang {
    let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    *active.get_or_insert_with(detect_now)
}

// Vuelve a decidir el idioma (tras guardar el de los ajustes). Se calcula antes de tomar el bloqueo.
#[cfg(not(test))]
pub fn reload() {
    let lang = detect_now();
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(lang);
}

// En las pruebas el idioma lo fija cada hilo con set_lang: no hay nada que recargar
#[cfg(test)]
pub fn reload() {}

// En las pruebas el idioma es por hilo: cada prueba corre en su propio hilo, así que `set_lang`
// de una no afecta a las demás aunque corran en paralelo (un Mutex obligaría a serializarlas).
// Sin `set_lang` el idioma es el inglés, sin mirar el entorno de quien ejecuta las pruebas.
#[cfg(test)]
thread_local! {
    static TEST_LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

#[cfg(test)]
pub fn lang() -> Lang {
    TEST_LANG.with(|l| l.get())
}

// Solo para pruebas: fija el idioma del hilo actual
#[cfg(test)]
pub fn set_lang(lang: Lang) {
    TEST_LANG.with(|l| l.set(lang));
}

// ---------- Catálogos ----------
// Inglés y español van escritos en cada llamada a tr/tr!. Los demás idiomas están en archivos
// locales/<código>.json: { "texto en inglés": "traducción" }. La clave es el texto en inglés tal cual
// está en el código, con sus {marcadores}; la traducción lleva los mismos marcadores. Un texto que
// falta en el catálogo sale en inglés. Para añadir un idioma ver docs/Idiomas.md.
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

// Traducción de un texto en inglés a un idioma con catálogo (o el mismo texto si no está)
pub fn translate(lang: Lang, en: &str) -> String {
    catalog(lang).get(en).cloned().unwrap_or_else(|| en.to_string())
}

// Sustituye los {nombre} de una plantilla por sus valores (los idiomas con catálogo no pueden usar
// format!, porque su texto no se conoce al compilar)
pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut text = template.to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

// Elige el texto según el idioma: tr("Cancelled", "Cancelado")
pub fn tr(en: &str, es: &str) -> String {
    match lang() {
        Lang::En => en.to_string(),
        Lang::Es => es.to_string(),
        other => translate(other, en),
    }
}

// Igual que tr pero con formato; los dos textos usan los mismos marcadores y cada valor va con su
// nombre (los idiomas con catálogo los necesitan para rellenar su traducción):
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

    // Lector de entorno falso con las variables dadas
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
        // Mayúsculas
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
        // MANDARINA_LANG gana a todo
        let all_es = [("LC_ALL", "es"), ("LC_MESSAGES", "es"), ("LANGUAGE", "es"), ("LANG", "es")];
        let mut vars = vec![("MANDARINA_LANG", "en")];
        vars.extend(all_es);
        assert_eq!(detect(env(&vars)), Lang::En);
        let all_en = [("LC_ALL", "en"), ("LC_MESSAGES", "en"), ("LANGUAGE", "en"), ("LANG", "en")];
        let mut vars = vec![("MANDARINA_LANG", "es")];
        vars.extend(all_en);
        assert_eq!(detect(env(&vars)), Lang::Es);
        // Después LC_ALL, LC_MESSAGES, LANGUAGE y LANG
        assert_eq!(detect(env(&[("LC_ALL", "en_US"), ("LC_MESSAGES", "es"), ("LANGUAGE", "es"), ("LANG", "es")])), Lang::En);
        assert_eq!(detect(env(&[("LC_MESSAGES", "en_US"), ("LANGUAGE", "es"), ("LANG", "es")])), Lang::En);
        assert_eq!(detect(env(&[("LANGUAGE", "en:es"), ("LANG", "es_PE")])), Lang::En);
        assert_eq!(detect(env(&[("LANGUAGE", "es:en"), ("LANG", "en_US")])), Lang::Es);
        // Una variable vacía no decide: pasa a la siguiente
        assert_eq!(detect(env(&[("LC_ALL", ""), ("LANG", "es_PE")])), Lang::Es);
        // "C" en LC_ALL fuerza el inglés aunque LANG sea español
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

    // Todos los textos en inglés de las llamadas a tr y tr! del código (sin las pruebas)
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
                // "tr(" o "tr!(" que no sea el final de otro nombre, seguido de un texto entre comillas
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
            // Y no sobra ninguna: un texto que ya no existe en el código es un resto
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
        // Un texto que no está en el catálogo sale en inglés, sin romper nada
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

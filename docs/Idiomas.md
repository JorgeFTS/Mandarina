# Idiomas

Mandarina está en **inglés (por defecto), español, portugués y francés**. Las notas de esta bóveda (`docs/`) son apuntes del autor y siguen en español; todo lo que ve el usuario va en los idiomas de la app. Inglés y español van escritos en el código; los demás son **datos** (catálogos), de modo que añadir un idioma no toca la lógica.

## Cómo se elige el idioma

Se decide al arrancar y **se vuelve a decidir al cambiarlo en los ajustes**, sin reiniciar, en este orden: `MANDARINA_LANG` del entorno (para forzarlo), el idioma guardado en los ajustes (`language` de `config.json`, ver [[Configuracion]]), `LC_ALL`, `LC_MESSAGES`, `LANGUAGE` (el primer idioma de la lista) y `LANG`. Si el valor empieza por el código de un idioma (`es`, `pt`, `fr`; p. ej. `pt_BR.UTF-8`) se usa ese; **en cualquier otro caso, o si no hay ninguno → inglés** (un sistema en chino o alemán ve la app en inglés). En los ajustes hay **Automático** (sigue al sistema) y un botón por idioma, con su nombre en su propio idioma.

- Rust: `src-tauri/src/lang.rs`. `Lang { En, Es, Pt, Fr }`, `LANGUAGES` (código, nombre propio y variante: de aquí salen la detección, los ajustes y la validación), `lang()` (se calcula al primer uso y queda guardado en un `Mutex`), `reload()` (lo vuelve a decidir; lo llama `set_language`; en las pruebas no hace nada), `set_lang(Lang)` (solo pruebas), `tr("English", "Español")` y `tr!(...)` con formato. `get_language() -> String` devuelve el código (`"en"`, `"es"`, `"pt"`, `"fr"`).
- **Catálogos del backend** (`src-tauri/locales/<código>.json`, cargados con `include_str!`): `{ "texto en inglés": "traducción" }`. La clave es el texto inglés tal cual está en el código, con sus `{marcadores}`; la traducción lleva los mismos. Un texto que falta sale en inglés. `tr` busca ahí cuando el idioma no es inglés ni español. `tr!` lleva cada valor con nombre (`tr!("Cannot convert {name} to {format}", "...", name = n, format = f)`) porque los idiomas con catálogo no pueden usar `format!` (su texto no se conoce al compilar) y rellenan los marcadores con `lang::fill`.
- JS: `src/i18n.js` define `window.I18n` con `init(lang)` (un código que no existe cae en inglés), `t(clave, variables)` (marcadores `{nombre}`; si falta una clave se usa el inglés, y si falta del todo, la propia clave), `plural` y `lang`. Los idiomas extra están en `src/locales/<código>.js` y se registran en `window.IdiomasExtra` antes de cargar `i18n.js`. `main.js` llama a `get_language` antes de pintar nada y otra vez al cambiar el idioma en los ajustes (`cambiarIdioma`: repinta los textos fijos, el texto del centro y la rueda); en modo demo se usa `?lang=<código>` (por defecto inglés, no `navigator.language`).

## Qué se traduce

| Dónde | Qué |
|---|---|
| Interfaz (`index.html`, `main.js`, `editor.js`, `ajustes.js`) | todos los textos, `aria-label`, avisos, tarjetas, títulos de progreso, etiquetas de herramientas y opciones **por `id`** (el JS ignora los `label` que manda `get_tools`; el backend los sigue enviando por compatibilidad) |
| Mensajes de error del backend | cada error que llega a la interfaz sale en el idioma activo |
| Nombres de salida de las herramientas | inglés `(compressed)`, `(no metadata)`, `(trimmed)`, `(rotated)`, `(flipped)`, `(resized)`, `(no audio)`; español `(comprimido)`, `(sin metadatos)`, `(recortado)`, `(girado)`, `(volteado)`, `(redimensionado)`, `(sin audio)`; portugués `(comprimido)`, `(sem metadados)`, `(cortado)`, `(girado)`, `(espelhado)`, `(redimensionado)`, `(sem áudio)`; francés `(compressé)`, `(sans métadonnées)`, `(coupé)`, `(pivoté)`, `(retourné)`, `(redimensionné)`, `(sans audio)` |
| Nombre inicial del editor de recorte | `nombre (trimmed)` / `(recortado)` / `(cortado)` / `(coupé)` |
| Menú de Nautilus | `Convert with Mandarina` / `Convertir con Mandarina` / `Converter com Mandarina` / `Convertir avec Mandarina`. El instalador (`packaging/install-nautilus.sh`) elige según el idioma (con la misma regla, incluido el idioma guardado en los ajustes) y borra los de los otros idiomas. **No se actualiza solo** al cambiar el idioma en los ajustes: hay que volver a ejecutar el instalador. Sus propios mensajes salen en inglés o español |
| README | `README.md` en inglés (el que muestra GitHub), `README.es.md` en español (no hay en portugués ni francés) |
| Metadatos del paquete (`Cargo.toml`, `tauri.conf.json`) | en inglés |

No se traducen: los mensajes de `ffmpeg`, ImageMagick y demás programas (se pasan tal cual), ni `docs/`.

## Reglas

- Los textos en inglés van primero y son los de referencia; el español y los demás deben decir lo mismo.
- Ningún texto visible queda escrito directamente en el código fuera de las tablas de traducción.
- Las pruebas de Rust usan el **inglés** (idioma por defecto) y hay pruebas específicas de los demás.
- **Pruebas de los catálogos** (`lang.rs`): `los_catalogos_tienen_todos_los_textos` recorre el código fuente en busca de cada texto de `tr`/`tr!` y falla si a un catálogo le falta alguno, o le sobra uno que ya no existe; `las_traducciones_llevan_los_mismos_marcadores` comprueba que cada traducción conserva sus `{marcadores}`. En JS no hay pruebas automáticas: las claves de cada `locales/*.js` deben ser las de `en` (157 claves).
- Las traducciones al portugués (Brasil) y al francés las escribió el asistente, **sin revisión de un hablante nativo**: conviene revisarlas, sobre todo los términos técnicos (metadatos, recortar, voltear).
- Textos largos: `RÉDUIRE LA TAILLE` y `REDIMENSIONNER` caben en los segmentos de la rueda de herramientas, pero justos. Con un idioma nuevo hay que mirar la rueda de herramientas (`?demo=herramientas&tipo=video&lang=<código>`).

## Añadir un idioma

Pasos resumidos (los detallados están en `CONTRIBUTING.md`): `src/locales/<código>.js` (+ su `<script>` en `index.html`), `src-tauri/locales/<código>.json`, una variante en `Lang` más una línea en `LANGUAGES` y otra en `CATALOGS` de `lang.rs`, y el nombre del menú en `install-nautilus.sh`. Los ajustes lo muestran solos.

## Cambio de idioma en vivo

Al pulsar un idioma en los ajustes: `set_language` guarda `config.json` y llama a `lang::reload()` (los mensajes y los nombres de salida del backend ya salen en el idioma nuevo); `ajustes.js` llama a `alCambiarIdioma` (`cambiarIdioma` de `main.js`: pide `get_language`, `I18n.init`, repinta `document.title`, `lang`, los `data-i18n` y el engranaje) y se vuelve a dibujar; al cerrar los ajustes `abrirAjustes` recalcula el texto del centro (`textoDeCentro`, que depende del idioma por la coma decimal) y dibuja la rueda otra vez. Si `MANDARINA_LANG` está definida en el entorno, manda ella y cambiar el idioma en los ajustes no se nota. Las tarjetas ya mostradas (progreso, errores) no se repintan; son pasajeras. El menú de Nautilus tampoco: hay que volver a ejecutar el instalador.

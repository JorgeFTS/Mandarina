# Configuración

Ajustes del usuario: qué formatos de destino muestra la rueda de conversión, el idioma y el color de acento. Código en `src-tauri/src/config.rs`; comandos en [[Comandos-Tauri]]. Pantalla en `src/ajustes.js` y `src/ajustes.css`.

## Qué se guarda

`~/.config/mandarina/config.json` (o `$XDG_CONFIG_HOME/mandarina/`; `MANDARINA_CONFIG_DIR` lo fuerza, útil para pruebas):

```json
{ "formats": { "image": ["webp", "jpg", "png"], "audio": ["mp3", "wav", "ogg"] }, "language": "es", "accent": "blue" }
```

La lista de cada tipo está en el orden en que sale en la rueda. Sin archivo, o con un archivo estropeado o que no vale, se usan las listas de siempre de [[Conversores]] (nunca falla el arranque).

## Reglas

- Tipos configurables: `image`, `audio`, `video`, `pdf`, `document`, `spreadsheet`. Presentaciones y subtítulos tienen solo 3 destinos y no se tocan; comprimidos y carpetas tampoco.
- Entre **3 y 8** formatos por tipo: con menos, al quitar el del propio archivo casi no quedan segmentos; 8 es el máximo de segmentos de la rueda y lo que traen de serie audio, vídeo y documento. Como el formato del propio archivo no se muestra, normalmente quedan 7 y cabe el segmento de HERRAMIENTAS o ZIP (con 8 visibles y un archivo de un formato que no está en la lista, p. ej. aac, HERRAMIENTAS no cabe, igual que antes). `set_formats` rechaza lo demás con un error; al leer el archivo, una lista que no cumple se ignora y vale la de siempre. (Antes el máximo era 7 y, tras quitar uno, no se podía volver a marcar sin Restaurar.)
- Solo valen formatos que existen para ese tipo (se normalizan a minúsculas y se quitan los repetidos).
- Se guarda en un temporal y se renombra, y en memoria solo queda lo que se pudo guardar.

## Idioma y color

- `language`: `"auto"` (sin valor guardado) o el código de un idioma de `LANGUAGES` en `lang.rs` (`en`, `es`, `pt`, `fr`; los acepta `check_language`). Se aplica **al momento**: `set_language` guarda el archivo y llama a `lang::reload()`, y la interfaz repinta sus textos (ver [[Idiomas]]). Orden de decisión: `MANDARINA_LANG` del entorno, luego el idioma guardado, luego `LC_ALL`/`LANG`... (`lang::detect_now` pasa el idioma guardado como si fuera el valor de `MANDARINA_LANG` cuando esa variable no existe; si `MANDARINA_LANG` está definida, manda ella y el cambio no se nota). `saved_language()` lee el archivo directamente, sin el bloqueo de `current()`, para que un error al guardar (que llama a `tr!`) no pueda bloquearse a sí mismo. El nombre del menú de Nautilus lo elige el instalador (que también lee este ajuste al ejecutarse) y no se actualiza solo al cambiarlo.
- `accent`: uno de `ACCENTS` (`orange`, `red`, `pink`, `purple`, `blue`, `green`; el primero es el de siempre). Los tonos están en `src/ajustes.js` (`ACENTOS`) y `aplicarAcento` cambia `--naranja` y `--naranja-claro` en `:root`; todo el CSS usa esas variables (el tinte del editor es `color-mix` con `--ed-naranja`). Se aplica en vivo y al arrancar (`aplicarAcentoGuardado` en `main.js`). Los segmentos de herramientas siguen grises azulados y no cambian.
- El tema claro u oscuro sigue al sistema y no es configurable (forzarlo obligaría a rehacer las hojas de estilo).

## Dónde se aplica

- `get_targets` (un archivo): `shown_targets_with` = lista completa de `targets_of` filtrada y ordenada por lo elegido. `targets_of` sigue siendo la lista completa y es la que valida `convert`, así que un formato oculto sigue siendo convertible por línea de comandos o desde pruebas.
- `get_targets_many` (varios): la intersección usa las listas **visibles** de cada tipo, y ZIP se añade siempre (ver [[Conversores]]).
- En las pruebas `config::current()` devuelve siempre los valores por defecto, para no depender de la máquina.

## Pantalla

- Un **engranaje** pequeño en la parte baja del círculo central de la rueda de conversión (no sale en la de herramientas ni en la de opciones). Está invisible y solo se ve con el ratón sobre el círculo central (`alMoverRaton` le pone la clase `visible`) o al enfocarlo con el teclado. El contenido del centro sube 18 px (`padding-bottom` de `#centro`) para dejarle sitio bajo la píldora.
- `abrirAjustes()` (`main.js`) hace lo mismo que el editor: oculta la rueda, pasa la ventana a 600x660 con `resize_center`, abre `#vista-ajustes` con `window.Ajustes.abrir` y al cerrar devuelve la ventana a su sitio. Si hubo cambios vuelve a pedir `get_targets_many` y arma la rueda otra vez. Durante la pantalla `vistaActual === "ajustes"`.
- Arriba, **Idioma** (Automático y un botón por idioma de `get_prefs().languages`, con su nombre propio) y **Color de acento** (6 círculos); después el título **Formatos de la rueda** y una sección por tipo (`get_config`) con una ficha-casilla por formato, el contador "N shown / N visibles" y **Restaurar** (`reset_formats`, desactivado si ya es la lista de siempre). Cada casilla guarda al momento con `set_formats`. En el máximo (7) las no marcadas se bloquean y en el mínimo (3) las marcadas; con 8 de serie siempre se puede quitar una. Un error del backend sale en rojo junto a **Listo**.
- Al elegir un idioma, la propia pantalla se vuelve a dibujar en el idioma nuevo (`alCambiarIdioma`). Escape y **Listo** cierran. Las funciones puras (`conCambio`, `puedeCambiar`) se exportan para pruebas.
- Demo sin Tauri: `?demo=rueda`, clic en el centro y en el engranaje (datos falsos en `CONFIG_DEMO`).

## Pendiente

Orden por arrastre y llegar a los ajustes también desde el círculo de inicio (sin archivo).

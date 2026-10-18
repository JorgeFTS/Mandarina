# Publicar

Notas de empaquetado y de preparación del repositorio para GitHub. Relacionado: [[Nautilus]], [[Decisiones]], [[Inicio]].

## Repositorio

- `git init -b main` hecho; sin commit, sin remoto. `.gitignore` ignora `node_modules/`, `src-tauri/target/` y `src-tauri/gen/` (Tauri regenera `gen/schemas` al compilar; `Cargo.lock` sí se versiona) y `.claude/`. También se ignoran `test-files/` (archivos de prueba personales, 170 MB), `*.deb`/`*.rpm`/`*.AppImage`, `.env*` y, por decisión del autor, `CLAUDE.md` y `SKILLS.md` (notas para trabajar con Claude Code; siguen en el disco pero no se publican).
- Licencia **MIT** (`LICENSE`, con "Copyright (c) 2026 JorgeFTS", el nombre de usuario de git; se puede cambiar por el nombre completo; también en `Cargo.toml` y `package.json`).
- Pendientes del autor: URL del repositorio (en `Cargo.toml` hay un `repository` comentado), autor real en `authors` (ahora "Autores de Mandarina"), y si el nombre "Mandarina" (traducción de "Tangerine") puede confundirse con la marca de Leap Studio.
- El registro de depuración temporal (`/tmp/mandarina-debug.log`) ya se quitó.

- Los README son `README.md` (inglés, el que muestra GitHub) y `README.es.md` (español); `README.en.md` ya no existe. Ver [[Idiomas]].
- Metadatos del paquete (`Cargo.toml` `[package]` y `bundle` de `tauri.conf.json`) en inglés. El `.desktop` del `.deb` también queda solo en inglés (Tauri no localiza el nombre ni el comentario sin una plantilla `desktopTemplate` propia; no se ha hecho). Se añadió `ghostscript` a `Recommends` porque comprimir PDF lo necesita.

## Icono

`packaging/icon/mandarina.svg` (hecho a mano) → `mandarina.png` de 1024x1024 con `rsvg-convert` → `npx tauri icon packaging/icon/mandarina.png`. Se borraron las carpetas `icons/android` e `icons/ios` que genera el comando, porque no se usan. Para cambiar el icono hay que repetir esos pasos.

## Compilación de release y .deb

Probado con `CARGO_TARGET_DIR` propio, fuera del repo:

| Paso | Resultado |
|---|---|
| `npm run tauri build -- --no-bundle` | OK, 1 min 52 s desde cero (LTO). Binario de 6.2 MB (la compilación de depuración pesa ~250 MB). |
| `npm run tauri build -- --bundles deb` | OK, 1 min 07 s. `Mandarina_0.1.0_amd64.deb` de 2.4 MB. |
| Arranque del binario release (`medir.sh`) | 1ª ejecución en frío ~2.9 s hasta la rueda lista; siguientes ~0.9 a 1.1 s. Ventana a pantalla completa a los ~0.5 s. |

El `.deb` contiene `usr/bin/mandarina`, `usr/share/applications/Mandarina.desktop` (categoría Utility, `Exec=mandarina`) e iconos hicolor (32, 128 y 256@2). **No instala** el script de Nautilus ni el atajo: `packaging/install-nautilus.sh` sigue siendo manual, y se le puede pasar `/usr/bin/mandarina`. Se podría añadir un script de postinstalación, pero tocaría `~/.local` de cada usuario y no se hace.

### Dependencias

Configuradas en `tauri.conf.json` (`bundle.linux.deb`):

- `Depends: libwebkit2gtk-4.1-0, libgtk-3-0`. Tauri ya las añade solas; si se repiten en `depends` salen duplicadas en el `control`, así que ahí queda la lista vacía.
- `Recommends: ffmpeg, imagemagick, librsvg2-bin, pandoc, libreoffice, poppler-utils, zip, unzip, p7zip-full`. Son recomendadas y no obligatorias porque cada formato usa su herramienta y la app avisa de la que falta; `libreoffice` es muy pesada. `apt` instala las recomendadas por defecto.

`targets` está en `["deb"]` (con `"all"` se intentan también rpm y AppImage, no probados).

## Sin probar

- Instalar el `.deb` en un sistema limpio (no se instaló en este equipo) y ejecutar la app desde el menú de aplicaciones.
- AppImage, rpm, Flatpak y Snap.
- El `.desktop` lanza la app sin argumentos (modo arrastrar), sin `GDK_BACKEND=x11`; en Wayland puede no colocarse junto al cursor.
- Hay avisos `dead_code` al compilar, procedentes del código Rust en desarrollo.

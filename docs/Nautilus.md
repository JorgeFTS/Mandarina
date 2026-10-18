# Nautilus (clic derecho)

Archivos: `packaging/nautilus/mandarina-script` (script; el instalador lo copia como `Convert with Mandarina`, `Convertir con Mandarina`, `Converter com Mandarina` o `Convertir avec Mandarina` según el idioma, ver [[Idiomas]]) y `packaging/install-nautilus.sh` (instalador). Lanza la app con las rutas de los elementos seleccionados, que abre la rueda del flujo A de la [[Rueda]].

- El script usa `$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` (o `$1`), exporta `GDK_BACKEND=x11` (con un archivo la ventana empieza cubriendo el monitor, transparente, para detectar el cursor y luego se encoge junto a él; ver [[Rueda]]) y busca el binario en `MANDARINA_BIN`, luego `mandarina` en el PATH y por último la ruta que escribe el instalador.
- El instalador copia el script a `~/.local/share/nautilus/scripts/`, sustituye `__MANDARINA_BIN__` por la ruta absoluta del binario y es idempotente. Elige el nombre con la misma regla de idioma de la app (`MANDARINA_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`), **borra el script del otro idioma** para no dejar dos entradas y sus mensajes salen en el idioma activo. Para desinstalar: borrar ese archivo.
- **Atajo de teclado:** el instalador escribe `<Alt>m Convert with Mandarina` (o el nombre del idioma que toque) en `~/.config/nautilus/scripts-accels` (cambiar con `MANDARINA_ATAJO="<Control><Alt>m" packaging/install-nautilus.sh`). Se selecciona el archivo en Nautilus y se pulsa el atajo. Nautilus lee ese archivo al arrancar: tras instalar hay que ejecutar `nautilus -q`.
- También se ve en clic derecho > Scripts.
- Formato de `scripts-accels` deducido, sin confirmar en la documentación: una línea `acelerador nombre del script`. Al instalar se quita la línea de cualquiera de los dos nombres y se deja el resto.

## Binario independiente

El script lanza el binario directamente, sin `npm run tauri dev`. Ese binario debe llevar `src/` incluido: compilar con

```
npm run tauri build -- --debug --no-bundle
```

(un `cargo build` normal no incluye el frontend). El resultado queda en `src-tauri/target/debug/mandarina`. Hay que recompilar así cada vez que cambie `src/`.

## Limitaciones

- Se pasan **todos** los archivos seleccionados (`NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` separado por saltos de línea, cada uno como un argumento; conserva espacios y comillas). Las carpetas se pueden comprimir (zip, tar, tar.gz).
- El escritorio de Ubuntu usa una extensión propia para los iconos: hay que comprobar si muestra los scripts al hacer clic derecho sobre un archivo del escritorio.
- Si se lanza desde la terminal de un VS Code instalado con snap, hay que limpiar su entorno (ver [[Decisiones]]).

Relacionado: [[Arquitectura]].

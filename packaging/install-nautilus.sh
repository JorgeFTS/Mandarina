#!/usr/bin/env bash
# Installs the right-click entry of the file manager: a Nautilus script into ~/.local/share/nautilus/scripts/
# and/or a Nemo action (Linux Mint) into ~/.local/share/nemo/actions/
# Usage / Uso: packaging/install-nautilus.sh [path/to/mandarina]
# Which one: MANDARINA_FM=nautilus|nemo|both. Without it, each file manager that is installed (or already has
# its folder in ~/.local/share); if neither is found, Nautilus.
# Idempotent: it can be run many times.
# Language (same rule as the app, see docs/Languages.md): MANDARINA_LANG, the language saved in the
# app settings (~/.config/mandarina/config.json), LC_ALL, LC_MESSAGES, first item of LANGUAGE, LANG.
# Starts with "es", "pt" or "fr" -> that language; anything else -> English.
# To add a language: one more line in LANGS and in script_name, plus the app files (see CONTRIBUTING.md).
set -eu

LANGS="en es pt fr"

# Name of the Nautilus script (the menu entry) in each language
script_name() {
    case "$1" in
        es) echo "Convertir con Mandarina" ;;
        pt) echo "Converter com Mandarina" ;;
        fr) echo "Convertir avec Mandarina" ;;
        *) echo "Convert with Mandarina" ;;
    esac
}

# Language saved in the app settings ("en", "es"...), or nothing
saved_lang() {
    local file="${XDG_CONFIG_HOME:-$HOME/.config}/mandarina/config.json"
    if [ -f "$file" ]; then
        sed -n 's/.*"language"[[:space:]]*:[[:space:]]*"\([a-z][a-z]\)".*/\1/p' "$file" | head -n 1
    fi
}

# Prints the language code. The first source that has a value decides (like the app)
detect_lang() {
    local name value
    for name in MANDARINA_LANG SAVED LC_ALL LC_MESSAGES LANGUAGE LANG; do
        if [ "$name" = "SAVED" ]; then value="$(saved_lang)"; else value="${!name:-}"; fi
        if [ "$name" = "LANGUAGE" ]; then value="${value%%:*}"; fi
        value="${value,,}"
        if [ -z "$value" ]; then continue; fi
        local code
        for code in $LANGS; do
            case "$value" in
                "$code"*) echo "$code"; return ;;
            esac
        done
        echo en
        return
    done
    echo en
}

lang="$(detect_lang)"
name="$(script_name "$lang")"

# Message by language: msg "English" "Español" (the installer only speaks English or Spanish)
msg() {
    if [ "$lang" = "es" ]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi
}

# Name of the Nemo action file (always the same, so there are never two entries) and its comment
nemo_comment() {
    case "$1" in
        es) echo "Convierte los archivos seleccionados con Mandarina" ;;
        pt) echo "Converte os arquivos selecionados com a Mandarina" ;;
        fr) echo "Convertit les fichiers sélectionnés avec Mandarina" ;;
        *) echo "Convert the selected files with Mandarina" ;;
    esac
}

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(dirname "$here")"
# Without an argument: the debug build of a repository checkout if it exists; otherwise the installed
# program (when this script comes from the .deb it lives in /usr/share/mandarina and there is no repository)
default_bin="$repo/src-tauri/target/debug/mandarina"
if [ ! -x "$default_bin" ]; then default_bin="$(command -v mandarina || echo /usr/bin/mandarina)"; fi
bin="$(realpath -m -- "${1:-$default_bin}")"

install_nautilus() {
    source_script="$here/nautilus/mandarina-script"
    dest_dir="$HOME/.local/share/nautilus/scripts"
    dest="$dest_dir/$name"

    mkdir -p "$dest_dir"
    # Remove the scripts of the other languages, so there are never two entries
    removed_other=0
    for code in $LANGS; do
        other="$(script_name "$code")"
        if [ "$other" != "$name" ] && { [ -e "$dest_dir/$other" ] || [ -L "$dest_dir/$other" ]; }; then
            rm -f -- "$dest_dir/$other"
            removed_other=1
            removed_name="$other"
        fi
    done
    # The marker is replaced by the binary path ("|" avoids clashing with the "/" of the path)
    escaped_bin="$(printf '%s' "$bin" | sed 's/[&|\\]/\\&/g')"
    sed "s|__MANDARINA_BIN__|$escaped_bin|g" "$source_script" > "$dest"
    chmod +x "$dest"

    # Keyboard shortcut (Nautilus reads scripts-accels). Change with MANDARINA_ATAJO, e.g. "<Control><Alt>c"
    atajo="${MANDARINA_ATAJO:-<Alt>m}"
    accels="${XDG_CONFIG_HOME:-$HOME/.config}/nautilus/scripts-accels"
    mkdir -p "$(dirname "$accels")"
    touch "$accels"
    # Remove the previous line of any language's name (if any) and add the new one; other lines stay untouched
    cp "$accels" "$accels.tmp"
    for code in $LANGS; do
        grep -v -F -e " $(script_name "$code")" "$accels.tmp" > "$accels.tmp2" || true
        mv "$accels.tmp2" "$accels.tmp"
    done
    printf '%s %s\n' "$atajo" "$name" >> "$accels.tmp"
    mv "$accels.tmp" "$accels"

    msg "Script installed at: $dest" "Script instalado en: $dest"
    if [ "$removed_other" = 1 ]; then
        msg "Removed the script of another language: $dest_dir/$removed_name" "Se borró el script de otro idioma: $dest_dir/$removed_name"
    fi
    msg "Keyboard shortcut: $atajo (file: $accels)" "Atajo de teclado: $atajo (archivo: $accels)"
    msg "If Nautilus does not show the script, restart it with: nautilus -q" "Si Nautilus no muestra el script, reinícialo con: nautilus -q"
}

install_nemo() {
    local dir="${XDG_DATA_HOME:-$HOME/.local/share}/nemo/actions"
    local file="$dir/mandarina.nemo_action"
    mkdir -p "$dir"
    # Quote=double: each path is quoted, so spaces in the names are preserved.
    # GDK_BACKEND=x11: same reason as in the Nautilus script (the window is placed next to the cursor).
    {
        echo "[Nemo Action]"
        echo "Name=$name"
        echo "Comment=$(nemo_comment "$lang")"
        echo "Exec=env GDK_BACKEND=x11 \"$bin\" %F"
        echo "Quote=double"
        echo "Selection=notnone"
        echo "Extensions=any;dir;"
        echo "Icon-Name=mandarina"
    } > "$file"
    msg "Nemo action installed at: $file" "Acción de Nemo instalada en: $file"
    msg "If Nemo does not show it, restart it with: nemo -q" "Si Nemo no la muestra, reinícialo con: nemo -q"
}

# Which file managers to set up
fm="${MANDARINA_FM:-auto}"
want_nautilus=0
want_nemo=0
case "$fm" in
    nautilus) want_nautilus=1 ;;
    nemo) want_nemo=1 ;;
    both) want_nautilus=1; want_nemo=1 ;;
    *)
        data="${XDG_DATA_HOME:-$HOME/.local/share}"
        if command -v nautilus >/dev/null 2>&1 || [ -d "$data/nautilus" ]; then want_nautilus=1; fi
        if command -v nemo >/dev/null 2>&1 || [ -d "$data/nemo" ]; then want_nemo=1; fi
        if [ "$want_nautilus" = 0 ] && [ "$want_nemo" = 0 ]; then want_nautilus=1; fi
        ;;
esac

if [ "$want_nautilus" = 1 ]; then install_nautilus; fi
if [ "$want_nemo" = 1 ]; then install_nemo; fi
msg "Configured binary: $bin" "Binario configurado: $bin"
if [ ! -x "$bin" ]; then
    msg "Warning: that binary does not exist yet. Build it or pass its path as the first argument." \
        "Aviso: ese binario aún no existe. Compílalo o pasa su ruta como primer argumento."
fi

# Nautilus (right click)

Files: `packaging/nautilus/mandarina-script` (the script; the installer copies it as `Convert with Mandarina`, `Convertir con Mandarina`, `Converter com Mandarina` or `Convertir avec Mandarina` depending on the language, see [[Languages]]) and `packaging/install-nautilus.sh` (the installer). It launches the app with the paths of the selected items, which opens flow A of the [[Wheel]].

- The script uses `$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` (or `$1`), exports `GDK_BACKEND=x11` (with a file, the window starts covering the monitor, transparent, to detect the cursor and then shrinks next to it; see [[Wheel]]) and looks for the binary in `MANDARINA_BIN`, then `mandarina` in the PATH and finally the path written by the installer.
- The installer copies the script to `~/.local/share/nautilus/scripts/`, replaces `__MANDARINA_BIN__` with the absolute path of the binary and is idempotent. It picks the name with the same language rule as the app (`MANDARINA_LANG`, the language saved in the settings, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`), **deletes the scripts of the other languages** so there are never two entries, and its messages come out in the active language. To uninstall: delete that file.
- **Keyboard shortcut:** the installer writes `<Alt>m Convert with Mandarina` (or the name of the language in use) in `~/.config/nautilus/scripts-accels` (change it with `MANDARINA_ATAJO="<Control><Alt>m" packaging/install-nautilus.sh`). Select the file in Nautilus and press the shortcut. Nautilus reads that file at startup: after installing, run `nautilus -q`.
- It also shows up under right click > Scripts.
- Format of `scripts-accels` inferred, not confirmed in the documentation: one line `accelerator script name`. When installing, the line of any of the names is removed and the rest is left alone.

## Standalone binary

The script launches the binary directly, without `npm run tauri dev`. That binary must include `src/`: build with

```
npm run tauri build -- --debug --no-bundle
```

(a plain `cargo build` does not include the frontend). The result is `src-tauri/target/debug/mandarina`. Rebuild like this every time `src/` changes.

## Limitations

- **All** the selected files are passed (`NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` separated by newlines, each one as an argument; it keeps spaces and quotes). Folders can be compressed (zip, tar, tar.gz).
- The Ubuntu desktop uses its own extension for icons: it needs to be checked whether it shows the scripts when right-clicking a file on the desktop.
- If it is launched from the terminal of a VS Code installed with snap, its environment has to be cleaned (see [[Decisions]]).

Related: [[Architecture]].

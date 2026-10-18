# Decisions

- **Tauri instead of Python/PySide6 or C++/Qt** (2026-10-18): the wheel is easier to make look good in SVG/CSS. Staying with Python, which was the recommended option for simplicity, was discarded. See [[Architecture]].
- **Vanilla template (no JS framework):** keep the code simple for contributors.
- **Open source on GitHub, MIT license** (`LICENSE`).
- **GNOME 50 and WebKitGTK** (2026-10-18): the app aborted at startup with `Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not contain a key named 'antialiasing'`. GNOME 50 moved those keys to a `.deprecated` schema and GTK 3.24 still looks for them in the original one. Local fix without `sudo`: copy the `xsettings` schema with the keys `antialiasing`, `hinting` and `rgba-order` added, plus `org.gnome.settings-daemon.enums.xml`, to `~/.local/share/glib-2.0/schemas/` and run `glib-compile-schemas` in that folder. Delete it if GNOME fixes it. It will affect users of Ubuntu with GNOME 50; the README warns about it.

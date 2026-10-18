# Decisiones

- **Tauri en vez de Python/PySide6 o C++/Qt** (2026-10-18): la rueda es más fácil de hacer bonita en SVG/CSS. Se descartó seguir con Python, que era la opción recomendada por simplicidad. Ver [[Arquitectura]].
- **Plantilla vanilla (sin framework JS):** mantener el código simple para contribuidores.
- **Open source en GitHub:** pendiente elegir licencia. Tauri y las herramientas del sistema permiten MIT o GPL.
- **GNOME 50 y WebKitGTK** (2026-10-18): la app se abortaba al arrancar con `Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not contain a key named 'antialiasing'`. GNOME 50 movió esas claves a un esquema `.deprecated` y GTK 3.24 aún las busca en el original. Arreglo local sin `sudo`: copiar el esquema `xsettings` con las claves `antialiasing`, `hinting` y `rgba-order` añadidas, más `org.gnome.settings-daemon.enums.xml`, a `~/.local/share/glib-2.0/schemas/` y ejecutar `glib-compile-schemas` en esa carpeta. Borrarlo si GNOME lo corrige. Afectará a usuarios con Ubuntu y GNOME 50; avisar en el README.

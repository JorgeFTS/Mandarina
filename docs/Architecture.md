# Arquitectura

Estado: plantilla de Tauri, sin lógica propia todavía.

Flujo previsto: el usuario suelta un archivo en [[Rueda]] → el JS llama a un comando de [[Comandos-Tauri]] con la ruta y el formato → Rust ejecuta la conversión en [[Conversores]] → devuelve éxito o error a la interfaz.

Carpetas:
- `src/`: `index.html`, `styles.css`, `main.js`.
- `src-tauri/src/`: `lib.rs` (comandos y arranque), `main.rs` (solo llama a `lib.rs`).

Ver [[Decisiones]].

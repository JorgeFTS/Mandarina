# Referencia: Tangerine for Mac

Fuente: press kit y video de demo en `~/Downloads/` (Leap Studio, v1.4, $29.99, solo Mac Apple silicon, 100 % offline). Es solo inspiración de comportamiento: no copiar sus imágenes, icono, textos ni diseño exacto.

## Comportamiento

- Al arrastrar un archivo aparece una rueda junto al cursor. **Tres teclas, tres ruedas:** Shift = conversiones, Command = herramientas de archivo, Control = acciones de archivo. La rueda sigue abierta al soltar la tecla.
- Se suelta el archivo sobre un segmento. Después sale una tarjeta flotante de progreso ("Converting to MP3", barra, X para cancelar) y el resultado aparece junto al original con nombre `sample 2.mp3`.
- Centro de la rueda: miniatura del archivo y píldora con el tamaño (2.4 MB); al pasar sobre un segmento muestra el formato. El segmento resaltado se pone naranja.

## Contenido de cada rueda

- **Conversión (imagen):** JPG, WEBP, HEIC, TIFF, AVIF, BMP, PDF, DOCX (8 segmentos). Audio: MP3, M4A, FLAC, OGG, OPUS, AIFF (6).
- **Herramientas (imagen):** COMPRESS, METADATA, EDIT, ANNOTATE, ADD BG, CROP, REDACT, con icono y etiqueta.
- **Ventanas de herramienta:** Edit Photo (deslizadores de exposición, contraste, sombras, temperatura...), Add BG (fondos, padding, esquinas, sombra, proporción), Trim Audio (forma de onda con extremos arrastrables), Convert Audio Channels (mono/estéreo), Bleep.
- **Control (acciones de archivo):** no aparece en el kit.

## Qué implica para Mandarina en Linux

- Wayland no permite detectar arrastres globales ni Shift/Ctrl fuera de la ventana: las tres ruedas se abrirían desde el clic derecho (tres entradas o un submenú) o con la rueda visible en nuestra ventana. Ver [[Decisiones]].
- Orden previsto: 1) rueda de conversión ([[Rueda]], [[Conversores]]), 2) tarjeta de progreso con cancelar, 3) rueda de herramientas (hecha: ver [[Herramientas]]).
- Nombre: "Mandarina" es la traducción de "Tangerine". Conviene pensar si hay riesgo de confusión de marca antes de publicar.

# Herramientas

Segunda rueda de la app, inspirada en la de herramientas de [[Referencia-Tangerine]]. Se abre desde un segmento **HERRAMIENTAS** de la rueda de conversión y solo con **un archivo** seleccionado (con varios no se ofrece). Comandos y tipos en [[Comandos-Tauri]]; interfaz en [[Rueda]]; Rust en `src-tauri/src/tools.rs`.

## Flujo de uso

1. Rueda de conversión → segmento **HERRAMIENTAS** (último de la rueda; solo aparece si `get_tools` devuelve algo).
2. Rueda de herramientas (mismo componente de rueda, otros segmentos). El centro del disco (o Escape) vuelve a la rueda anterior.
3. Según la herramienta:
   - sin opciones (**Reducir tamaño**, **Metadatos**, **Sin audio**, **ZIP**): se ejecuta directamente;
   - con opciones (**Girar**, **Voltear**, **Redimensionar**): una tercera rueda con las opciones;
   - con editor (**Recortar**): la ventana crece y se abre el editor visual (recuadro sobre la imagen, o forma de onda / fotogramas con dos extremos arrastrables para audio y vídeo); botones Aplicar y Cancelar.
4. Tarjeta de progreso y "Listo ✓" en el centro, como en las conversiones.

## Herramientas por tipo (primera versión)

| Tipo | Herramientas |
|---|---|
| imagen | reducir tamaño, metadatos, recortar (recuadro), girar, voltear, redimensionar, zip |
| vídeo | reducir tamaño, metadatos, recortar (tiempo), girar, voltear, redimensionar, sin audio, zip |
| audio | reducir tamaño, metadatos, recortar (tiempo), zip |
| pdf | reducir tamaño (ghostscript), zip |
| documento, presentación, hoja, subtítulos, svg, heic | solo zip |
| comprimidos, carpetas y tipos desconocidos | ninguna (no sale el segmento) |

Si un tipo solo tiene una herramienta (pdf), la rueda de conversión muestra ese segmento directamente (p. ej. ZIP en un documento) en vez de HERRAMIENTAS.

## Reglas

- La salida va junto al original, **mismo formato**, con un sufijo: `nombre (comprimido).ext`, `(sin metadatos)`, `(recortado)`, `(girado)`, `(volteado)`, `(redimensionado)`, `(sin audio)`; si existe, `nombre (comprimido) 2.ext`. Nunca se sobrescribe.
- Un solo nivel de compresión, equilibrado.
- **Nombre del archivo nuevo (solo en Recortar):** los tres editores tienen un campo de texto, con valor inicial `nombre (recortado)` y la extensión del original en gris a su derecha. Se valida en vivo (vacío, `/`, `.`, `..`, más de 200 caracteres) y Aplicar se desactiva si no vale. El editor devuelve `nombre` junto a los demás parámetros y `run_tool` lo usa (`custom_name`, `output_path_named`): conserva la extensión del original, quita la que escriba el usuario al final y nunca sobrescribe (`nombre 2.ext`). Enter dentro del campo aplica.
- Las herramientas de ffmpeg emiten `convert-progress` y se pueden cancelar con `cancel`, como las conversiones.
- Programas: ImageMagick, ffmpeg y ghostscript (`gs`). No hace falta exiftool.

## Cómo se hace cada herramienta (medido en esta máquina)

| Herramienta | Comando elegido | Resultado medido |
|---|---|---|
| imagen: comprimir jpg | `magick -auto-orient -strip -quality 75 -sampling-factor 4:2:0 -interlace Plane` | 843 KB → 175 KB |
| imagen: comprimir webp / avif / png / tiff / gif | `-quality 75` / `-quality 60` / `png:compression-level=9`+filtro y estrategia / `-compress zip` / `-layers Optimize` | avif 1,4 MB → 70 KB; png de captura 50 → 21 KB, pero con fotos casi no ahorra; tiff y gif ahorran poco |
| imagen: metadatos, recortar, girar, voltear, redimensionar | `-auto-orient -strip`; `-crop WxH+X+Y +repage`; `-rotate`; `-flop`/`-flip`; `-resize N%` o `Nx>` (no amplía) | comprobado con medidas y píxeles |
| vídeo: comprimir | libx264 crf 28 + aac 128k (mp4, mov, mkv); libx264 + libmp3lame (avi); libvpx-vp9 crf 33 + opus (webm, lento); wmv2 | 7,9 MB → 2,0 MB |
| vídeo: metadatos / sin audio | `-map_metadata -1 -map_chapters -1 -c copy` / `-map 0:v -c copy` | sin recodificar |
| vídeo: recortar | `-ss S -i ... -t D` y **recodifica** | corte exacto al fotograma |
| vídeo: girar, voltear, redimensionar | `transpose`, `hflip`/`vflip`, `scale` con dimensiones pares (sin ampliar) | el audio se copia |
| audio: comprimir | mp3 libmp3lame 128k; m4a aac 128k; ogg libvorbis q4; opus 96k; wma 128k | mp3 803 → 321 KB |
| audio: metadatos / recortar | `-map_metadata -1 -c copy` / `-ss -i -t -c copy` | recorte con precisión de unos 25 ms (flac se recodifica) |
| pdf: comprimir | `gs -sDEVICE=pdfwrite -dPDFSETTINGS=/ebook` (salida a un temporal) | 5,1 MB → 131 KB |

## Decisiones

- Si el resultado no pesa menos del 98 % del original, se borra y se devuelve `Err("Ya está optimizado")` (la interfaz lo muestra como aviso suave).
- **COMPRIMIR no se ofrece** en wav, flac, aiff (sin pérdida) ni bmp: no hay forma de comprimirlos sin perder calidad o formato.
- `get_tools` devuelve vacío para svg, heic y heif (ImageMagick no los escribe con la misma extensión o son vectoriales).
- Comprimir audio descarta la carátula incrustada; comprimir y recortar vídeo dejan solo la primera pista de vídeo y las de audio, sin subtítulos.
- Quitar metadatos de una imagen también quita el perfil ICC.
- Vídeos con rotación por metadato: `media_info` intercambia ancho y alto, y ffmpeg aplica la rotación al recodificar.
- Los mensajes de ffmpeg e ImageMagick salen con el prefijo "No se pudo aplicar la herramienta:" pero el detalle queda en inglés.
- En la rueda de conversión, HERRAMIENTAS se descarta si ya hay 8 formatos de destino (pasa con formatos como aac o flv, que no están en su propia lista).
- La rueda espera `get_tools` hasta 400 ms; si no llega, se pinta sin el segmento.

## Pruebas

Parte de las 82 pruebas de `cargo test`: tabla de herramientas, nombres de salida, validación de parámetros, picos de la forma de onda, y integración con archivos reales (comprimir, metadatos, recortar, girar, voltear, redimensionar, sin audio, progreso y cancelar). **Sin probar:** la ventana real (`resize_center`, el editor con ratón), `convert` junto con herramientas, recortes de heic, svg y gif animado, vídeos con varias pistas de audio o subtítulos, y avif real de principio a fin.

## Pendiente / no incluido

Reproducir el audio o vídeo dentro del editor, recortar vídeo en el espacio (solo en el tiempo), herramientas para documentos y hojas, y ejecutar herramientas con varios archivos.

- **Tarjeta de progreso con nombre personalizado:** si el editor devuelve `nombre`, la tarjeta de progreso muestra ese nombre (más la extensión del original, `nombreConExtension` en `src/main.js`) en vez del nombre del archivo de origen.

- **Reducir tamaño no es zipear:** `compress` recodifica el archivo para que pese menos y lo deja en el mismo formato (su etiqueta pasó de COMPRIMIR a REDUCIR TAMAÑO / REDUCE SIZE; el id y el sufijo `(comprimido)` no cambiaron).
- **ZIP (`zip`):** herramienta de cualquier archivo que no sea comprimido ni carpeta. Llama a `converters::pack`, que deja `nombre.zip` junto al original (nunca sobrescribe). Con varios archivos el mismo `pack` se usa desde la rueda de conversión, ver [[Conversores]].

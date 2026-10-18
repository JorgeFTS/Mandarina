# Rueda

Archivos: `src/index.html`, `src/styles.css`, `src/geometria.js` (geometría pura), `src/main.js` (lógica). Sin framework ni bundler. Usa los comandos de [[Comandos-Tauri]] con `window.__TAURI__`. Diseño propio inspirado en el comportamiento de [[Referencia-Tangerine]].

La ventana (ver `tauri.conf.json`) es transparente, sin bordes, siempre encima y **arranca oculta**: la interfaz pinta y luego llama a `show_wheel`.

**Dónde aparece la rueda (flujo A):** en Wayland la app no puede saber dónde está el cursor y bajo XWayland la posición de Tauri queda desfasada, así que se hace en dos pasos: 1) `show_wheel` **agranda la ventana a mano hasta cubrir el monitor principal** (transparente, escenario con `opacity: 0`) solo para que la página reciba el primer evento del cursor (`esperarCursor`, máximo 600 ms; si no llega, usa el centro del monitor); 2) `place_wheel(x, y)` **encoge la ventana a 460x460 con su centro en ese punto** (limitada al monitor) y la interfaz muestra la rueda. En el flujo B la ventana es pequeña y centrada desde el principio.

**Por qué no `set_fullscreen`:** ese modo lo gestiona el compositor de GNOME, que oculta la barra superior y anima la transición; se veía como un parpadeo ("reseteo") cada vez que se abría la rueda. Con `set_size` + `set_position` el compositor no interviene. Ojo: al quitar `set_fullscreen` también se quitaron las pausas entre cambiar el tamaño y mover la ventana, y sin ellas el movimiento se ignoraba (la ventana se quedaba en +138+64 y no seguía al cursor ni se centraba); se restauraron en `resize_and_move` (60 ms y tres intentos de posición). Limitación: la ventana cubre el monitor **principal**, no el del cursor (con varios monitores la rueda solo se colocaría bien en el principal).

Como la ventana ya no cubre la pantalla, un clic fuera no la cierra: `vigilarAbandono` (que vuelve a comprobar la vista al dispararse, para no cerrar a mitad de una conversión) cierra la app al perder el foco (se activa 1 s después de colocar la ventana), tras 4 s sin mover el ratón ni pulsar teclas con la rueda abierta (`TIEMPO_ABANDONO`), o con Escape. Medido con la compilación de depuración (250 MB): la primera ejecución tras compilar tarda ~2.8 s hasta tener la rueda lista (el binario aún no está en memoria) y las siguientes ~0.8 s.

**Los mensajes van en el centro:** la rueda se abre junto al cursor, pero las tarjetas (progreso, "Listo", errores, avisos) se muestran en el **centro del monitor**. Al pasar de la rueda a la tarjeta, `moverYMostrar("centro", ...)` oculta el escenario un instante, mueve la ventana con `center_wheel` y muestra la tarjeta; si se cancela, `moverYMostrar("cursor", ...)` la devuelve a donde se abrió (`posRueda`). Un aviso inicial (p. ej. "Formato no soportado") va directamente al centro. Mientras la ventana se mueve (`enTransicion`) no se cierra ni se admiten más clics.

## Dos formas de uso

- **A. Con archivo** (clic derecho en Nautilus, ver [[Nautilus]]): `initial_paths` → `file_info` (cada uno) + `get_targets_many` → rueda. Clic en un segmento (o flechas + Enter) convierte. Escape o clic fuera de la rueda cierra la app.
- **B. Sin archivo:** círculo punteado "Suelta un archivo aquí". Al arrastrar un archivo encima (`onDragDropEvent`: `enter`, `over`, `drop`, `leave`) aparece la rueda y se resalta el segmento bajo el cursor (ángulo y distancia, con `position / devicePixelRatio`). Soltar sobre un segmento convierte. Escape o clic fuera vuelven al círculo, no cierran.

## Vistas

1. **Rueda:** de 2 a 8 segmentos (anillo con hueco, separación y esquinas redondeadas), icono según el tipo y píldora con el tamaño; al pasar sobre un segmento se pone naranja y la píldora muestra el formato.
2. **Tarjeta convirtiendo:** "Convirtiendo a MP3", nombre, barra y X que llama a `cancel`. Con ffmpeg la barra sigue el porcentaje real; en los demás casos no mide el avance real: sube rápido y se frena cerca del 92 % hasta que termina la conversión (`animarBarra`); dura al menos 2.2 s aunque el archivo sea pequeño y al terminar se llena y espera 0.45 s antes de mostrar "Listo".
3. **Tarjeta resultado:** marca ✓, "Listo" y el nombre de salida, sin botones (el resultado ya está en la misma carpeta que el original); se cierra sola a 1 s, con pausa si el cursor está encima. Error: tarjeta roja con el mensaje. "Cancelado" vuelve a la rueda sin mensaje.

## Detalles

- Modo claro y oscuro con `prefers-color-scheme`; sin `backdrop-filter`; animación de 150 ms y `prefers-reduced-motion`.
- Accesible: segmentos con `role="button"`, `aria-label`, foco visible y teclado.
- **Modo demo** sin Tauri (abrir `index.html` en un navegador, o `?demo`): datos falsos. `?demo=rueda|convirtiendo|listo|error|aviso` salta a esa vista.
- Para probar la geometría con Node hay que copiar `geometria.js` a `.cjs` (`package.json` tiene `"type": "module"`).
- **Varios archivos:** el estado es una lista (`rutas`, `infos`). La rueda ofrece los formatos que todos admiten; el centro muestra "N archivos · tamaño total", el icono del tipo común (o una pila de archivos si son tipos mezclados) y la miniatura del primero con una insignia numérica. Con 1 archivo se usa `convert`; con más, `convert_many`: la tarjeta dice "Convirtiendo a MP3 · 2 de 5" (evento `convert-file`) y la barra sigue el progreso global. Resultado: "N archivos convertidos" (y "M omitidos"), o con fallos una tarjeta con borde naranja "K de N convertidos" con hasta 3 errores y botón Cerrar sin cierre automático; si todos fallan, tarjeta de error. Sin formato común: "Estos archivos no tienen un formato en común". Demos: `?demo=varios|variosfallo|variosnocomun|variosrueda|variossaltados`.
- **Miniatura:** tras pintar la rueda se llama a `thumbnail(path)` sin esperar; si llega un data URL sustituye al icono (88 px, esquinas redondeadas, fundido de 250 ms) y la píldora queda debajo. Si falla se queda el icono y no se muestra error. Si el usuario cambia de archivo antes de que llegue, se ignora la respuesta vieja.
- **Progreso real:** `animarBarra` tiene dos modos. Con ffmpeg (audio, vídeo, subtítulos) y con lotes, la barra empieza en 0 y sigue **solo** los eventos `convert-progress` (el texto dice "Convirtiendo a MP3 · 42 %"); si en 3 s no llega ninguno, pasa a la simulada. Con imágenes, documentos y demás sube una barra simulada (rápido y frenándose cerca del 92 %) sin porcentaje en el texto. Nunca retrocede. Se escucha antes de llamar a `convert` y se hace `unlisten` al terminar. Un fallo corregido: antes la simulada arrancaba siempre y se adelantaba al porcentaje real (la barra iba por el 50 % mientras el texto decía 0 %); ahora la real no se simula.
- **Herramientas:** ver [[Herramientas]]. La rueda tiene tres niveles (`conversion`, `herramientas`, `opciones`) con la misma geometría; el segmento HERRAMIENTAS (llave inglesa, tono gris azulado) va al final y solo con un archivo. El centro del disco o Escape retroceden un nivel. Las herramientas con editor abren `#vista-editor` con `window.Editor` (`src/editor.js` y `src/editor.css`, prefijo `.ed-`): la ventana pasa a 780x640 (`resize_center`) y vuelve a 460x460 al terminar; durante el editor no se aplica el cierre por inactividad ni por perder el foco (`vistaActual === "editor"`). Demos: `?demo=herramientas|opciones|editorimagen|editoraudio|editorvideo|herramientaejecutando` con `&tipo=imagen|audio|video|pdf`.
- Pendiente: ruedas de acciones de archivo (ver [[Referencia-Tangerine]]).

## Sin verificar en la ventana real

Posición real junto al cursor, la transparencia en pantalla, el arrastre real y `cancel`. Las capturas con Chrome headless solo prueban el diseño, no Tauri.

Relacionado: [[Arquitectura]], [[Conversores]].

## Fallo corregido: Aplicar en el editor cerraba la app

El clic de **Aplicar** (o Cancelar) del editor resolvía la promesa y, en ese mismo clic, el evento seguía subiendo hasta `alHacerClic`; como `abrirEditor` ya había puesto la vista en "rueda", el clic caía "fuera de la rueda" y llamaba a `salir()`: la app se cerraba antes de recortar. Se arregló ignorando en `alHacerClic` los clics dentro de `#vista-editor` y esperando 80 ms antes de cambiar de vista. Se encontró con un registro de depuración temporal que ya se quitó.

## Idiomas

Ver [[Idiomas]]. `src/i18n.js` (`window.I18n`, 130 claves en `en` y `es`) se carga antes que `main.js` y `editor.js`; `iniciar()` pide `get_language` antes de pintar. En demo, `?lang=es|en` (por defecto inglés). Las etiquetas de herramientas y opciones se traducen por id. Los errores se reconocen por código (`[cancelled]`, `[optimized]`), no por texto. Demo nuevo: `?demo=optimizado`.

- **Ajustes:** engranaje oculto en la parte baja del círculo central (solo se ve con el ratón encima) que abre la pantalla de formatos de la rueda (`#vista-ajustes`, `src/ajustes.js`). Ver [[Configuracion]]. Un clic en el engranaje no lo trata `alHacerClic` (con el teclado llega con coordenadas 0,0 y se tomaría por "fuera de la rueda" y cerraría la app).

- **Cuándo se cierra la rueda (flujo A, el de Nautilus; `modoLibre` nunca cierra la app, vuelve al círculo):** clic fuera del círculo, Escape (en las ruedas de herramientas y opciones solo retrocede un nivel), pérdida de foco (activa 1 s después de abrir) y 4 s sin mover el ratón ni pulsar teclas (`vigilarAbandono`); los tres últimos solo mientras la vista es la rueda y no hay transición. **No se cierra** durante la conversión (se cancela con la ✕), ni con el editor o los ajustes abiertos, y las tarjetas de error y aviso esperan a que se pulse Cerrar o Escape (la ventana es `alwaysOnTop` y sin barra de tareas). "Listo" se cierra a 1 s salvo que el ratón esté encima. `mostrarRueda` vuelve a armar el cierre por inactividad (`rearmarAbandono`) al regresar de los ajustes, del editor o de cancelar.

// Interfaz de la rueda flotante. La geometría está en geometria.js
const NS = "http://www.w3.org/2000/svg";
const TIEMPO_CIERRE = 1000; // cuánto se ve el "Listo" antes de cerrarse
const TIEMPO_ABANDONO = 4000; // la rueda se cierra sola si no se toca nada en este tiempo
const MIN_PROGRESO = 2200; // la barra tarda al menos esto en llegar al final, aunque el archivo sea pequeño

const params = new URLSearchParams(window.location.search);
const tauri = window.__TAURI__;

const demo = !tauri || params.has("demo");
const t = (clave, variables) => window.I18n.t(clave, variables);
// Flujo B ("modo libre"): sin archivo inicial; nunca cierra la app, vuelve al círculo
let modoLibre = true;

// ---------- Iconos por tipo de archivo ----------

const ICONOS = {
  imagen: '<rect x="4" y="5" width="16" height="14" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M4 17l5-4 4 3 3-2 4 3"/>',
  audio: '<path d="M9 18V6l10-2v12"/><circle cx="7" cy="18" r="2"/><circle cx="17" cy="16" r="2"/>',
  video: '<rect x="3" y="6" width="13" height="12" rx="2"/><path d="M16 11l5-3v8l-5-3z"/>',
  documento: '<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4M10 12h5M10 16h5"/>',
  hoja: '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 10h16M4 15h16M10 4v16"/>',
  carpeta: '<path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>',
  generico: '<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/>',
  pila: '<path d="M9 8h7l4 4v8H9z"/><path d="M16 8v4h4"/><path d="M6 16V5a1 1 0 011-1h7"/>',
};

// Iconos de las herramientas (mismo viewBox 24x24 que los de tipo)
const ICONOS_HERR = {
  herramientas: '<path d="M14.7 6.3a1 1 0 000 1.4l1.6 1.6a1 1 0 001.4 0l3.77-3.77a6 6 0 01-7.94 7.94l-6.91 6.91a2.12 2.12 0 01-3-3l6.91-6.91a6 6 0 017.94-7.94l-3.76 3.76z"/>',
  compress: '<path d="M4 4l6 6M10 5v5H5M20 4l-6 6M14 5v5h5M4 20l6-6M10 19v-5H5M20 20l-6-6M14 19v-5h5"/>',
  strip_metadata: '<path d="M3 12V4h8l9 9-8 8z"/><circle cx="7.5" cy="8.5" r="1.2"/><path d="M4 20L20 4"/>',
  trim: '<path d="M6 2v16h16M2 6h16v16"/>',
  rotate: '<path d="M20 12a8 8 0 11-2.5-5.8M20 4v5h-5"/>',
  flip: '<path d="M12 3v18M8 7l-5 5 5 5zM16 7l5 5-5 5z"/>',
  resize: '<path d="M14 4h6v6M20 4l-7 7M10 20H4v-6M4 20l7-7"/>',
  mute: '<path d="M4 9v6h4l5 4V5L8 9zM17 9l4 6M21 9l-4 6"/>',
  zip: '<path d="M4 7l8-4 8 4v10l-8 4-8-4zM4 7l8 4 8-4M12 11v10"/>',
};

const TIPOS = {
  imagen: "png jpg jpeg gif webp heic heif tiff tif avif bmp svg ico",
  audio: "mp3 wav m4a flac ogg opus aiff aif aac wma",
  video: "mp4 mov mkv avi webm m4v wmv flv",
  documento: "pdf doc docx odt txt rtf md ppt pptx odp",
  hoja: "xls xlsx ods csv",
};

function tipoDe(nombre, esCarpeta) {
  if (esCarpeta) return "carpeta";
  const ext = nombre.includes(".") ? nombre.split(".").pop().toLowerCase() : "";
  for (const tipo in TIPOS) {
    if (TIPOS[tipo].split(" ").includes(ext)) return tipo;
  }
  return "generico";
}

function iconoSvg(tipo) {
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONOS[tipo]}</svg>`;
}

// ---------- Utilidades ----------

function formatearTamano(bytes) {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + " KB";
  if (bytes < 1024 ** 3) return window.I18n.num(bytes / 1024 ** 2, 1) + " MB";
  return window.I18n.num(bytes / 1024 ** 3, 1) + " GB";
}

// Los errores del backend traen un código delante: "[cancelled] texto" o "[optimized] texto"
function conCodigo(mensaje, codigo) {
  return String(mensaje).includes("[" + codigo + "]");
}

function sinCodigo(mensaje) {
  return String(mensaje).replace(/^Error: /, "").replace(/\[(cancelled|optimized)\]\s*/, "");
}

// Idioma de la interfaz: lo decide el backend; en demo, ?lang=es|en (por defecto inglés)
async function elegirIdioma() {
  window.I18n.demo = demo;
  await cambiarIdioma();
}

// Pide el idioma al backend y repinta los textos fijos de la página. También se usa al cambiar el
// idioma en los ajustes, sin reiniciar.
async function cambiarIdioma() {
  let lang = "en";
  try {
    lang = await api.get_language(); // I18n.init cae en inglés si el código no existe
  } catch (e) {}
  window.I18n.init(lang);
  document.documentElement.lang = window.I18n.lang;
  document.title = t("app.title");
  document.querySelectorAll("[data-i18n]").forEach((nodo) => (nodo.textContent = t(nodo.dataset.i18n)));
  $("ajustes").setAttribute("aria-label", t("wheel.settings"));
  $("ajustes").title = t("wheel.settings");
}

// Etiqueta de una herramienta o de una opción por su id; el label del backend solo es respaldo
function etiqueta(tipo, elemento) {
  const clave = tipo + "." + elemento.id;
  return window.I18n.has(clave) ? t(clave) : elemento.label;
}

function nombreDe(ruta) {
  return ruta.split(/[\\/]/).filter(Boolean).pop() || ruta;
}

function crear(etiqueta, clase, texto) {
  const e = document.createElement(etiqueta);
  if (clase) e.className = clase;
  if (texto !== undefined) e.textContent = texto;
  return e;
}

function esperar(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

// ---------- Acceso al backend (o datos falsos en modo demo) ----------

// Miniatura falsa para el modo demo: un degradado naranja
const MINIATURA_DEMO =
  "data:image/svg+xml," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">' +
      '<stop offset="0" stop-color="#ffb347"/><stop offset="1" stop-color="#f26a1b"/></linearGradient></defs>' +
      '<rect width="200" height="200" fill="url(#g)"/><circle cx="140" cy="60" r="22" fill="#fff" opacity="0.7"/></svg>'
  );

// Archivos falsos del modo demo con varios archivos
const FOTOS_DEMO = [1, 2, 3, 4, 5].map((n) => `/demo/foto${n}.png`);
const TAMANOS_DEMO = [3.1, 1.2, 4.8, 0.9, 2.2];
const demoVarios = params.get("demo") || "";
let progresoGlobalDemo = null;
let archivoDemo = null;

// Conversión falsa de varios archivos: 5 archivos en unos 5 s
async function convertirVariosDemo(rutas) {
  const total = rutas.length;
  for (let i = 1; i <= total; i++) {
    if (archivoDemo) archivoDemo({ index: i, total, name: nombreDe(rutas[i - 1]) });
    for (let paso = 1; paso <= 10; paso++) {
      await esperar(100);
      if (progresoGlobalDemo) progresoGlobalDemo(Math.min(99, ((i - 1 + paso / 10) / total) * 100));
    }
  }
  const resultado = { outputs: rutas.map((r) => r.replace(".png", ".jpg")), skipped: [], failed: [] };
  if (demoVarios === "variosfallo") {
    resultado.outputs.splice(2, 1);
    resultado.failed = [{ name: "foto3.png", error: t("demo.damaged") }];
  }
  if (demoVarios === "variossaltados") {
    resultado.outputs = resultado.outputs.slice(0, 3);
    resultado.skipped = ["foto4.png", "foto5.png"];
  }
  return resultado;
}

// Progreso falso para el modo demo: del 0 al 99 % en unos 3 s
function progresoDemo(alProgreso) {
  const inicio = Date.now();
  const reloj = setInterval(() => alProgreso(Math.min(99, ((Date.now() - inicio) / 3000) * 100)), 100);
  return () => clearInterval(reloj);
}

// Archivos y herramientas falsos del modo demo (?demo=...&tipo=imagen|audio|video|pdf)
const ARCHIVOS_DEMO = {
  imagen: "/demo/foto.jpg",
  audio: "/demo/ejemplo.wav",
  video: "/demo/video.mp4",
  pdf: "/demo/documento.pdf",
};
// Idioma y color falsos del modo demo (?accent=blue prueba un color al arrancar)
const PREFS_DEMO = { language: "auto", accent: params.get("accent") || "orange" };

// Configuración falsa del modo demo (misma forma que get_config)
const CONFIG_DEMO = [
  ["image", ["jpg", "png", "webp", "tiff", "avif", "bmp", "pdf"]],
  ["audio", ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"]],
  ["video", ["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]],
  ["pdf", ["docx", "jpg", "png", "epub", "txt", "md"]],
  ["document", ["docx", "md", "txt", "epub", "html", "pdf", "jpg", "png"]],
  ["spreadsheet", ["xlsx", "ods", "csv", "tsv", "pdf"]],
].map(([key, all]) => ({ key, all, enabled: all.slice(), min: 3, max: 8 }));

const DESTINOS_DEMO = {
  imagen: ["png", "webp", "tiff", "avif", "bmp", "pdf"],
  audio: ["mp3", "m4a", "flac", "ogg", "opus", "aiff"],
  video: ["mp4", "mov", "mkv", "webm", "avi", "gif", "mp3"],
  pdf: ["docx", "jpg", "png", "epub", "txt", "md"],
};
function rutaDemo(porDefecto) {
  return ARCHIVOS_DEMO[params.get("tipo")] || ARCHIVOS_DEMO[porDefecto];
}

function extDe(nombre) {
  return nombre.includes(".") ? nombre.split(".").pop().toLowerCase() : "";
}

// "imagen" | "audio" | "video" | "pdf" | otro, según la extensión
function claseDemo(path) {
  const nombre = nombreDe(path);
  return extDe(nombre) === "pdf" ? "pdf" : tipoDe(nombre, false);
}

// Los label son neutros (el id): la interfaz traduce por id y solo usa el label como respaldo
function herramientasDemo(path) {
  const clase = claseDemo(path);
  const h = (id, options, editor) => ({ id, label: id, options: options || [], editor: editor || null });
  const opciones = (ids) => ids.map((id) => ({ id, label: id }));
  const pct = ["pct:75", "pct:50", "pct:25"];
  const girar = opciones(["left", "right", "180"]);
  const voltear = opciones(["horizontal", "vertical"]);
  if (clase === "pdf") return [h("compress"), h("zip")];
  if (clase === "audio") return [h("compress"), h("strip_metadata"), h("trim", [], "trim"), h("zip")];
  if (clase === "imagen") {
    const px = ["w:1920", "w:1280", "w:800"];
    return [
      h("compress"), h("strip_metadata"), h("trim", [], "crop"),
      h("rotate", girar), h("flip", voltear), h("resize", opciones([...pct, ...px])), h("zip"),
    ];
  }
  if (clase === "video") {
    const p = ["h:1080", "h:720", "h:480"];
    return [
      h("compress"), h("strip_metadata"), h("trim", [], "trim"),
      h("rotate", girar), h("flip", voltear), h("resize", opciones([...pct, ...p])),
      h("mute"), h("zip"),
    ];
  }
  return [];
}

const SVG_DEMO = (ancho, alto, tono) =>
  "data:image/svg+xml," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${ancho}" height="${alto}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
      `<stop offset="0" stop-color="hsl(${tono},85%,60%)"/><stop offset="1" stop-color="hsl(${tono + 50},80%,40%)"/></linearGradient></defs>` +
      `<rect width="${ancho}" height="${alto}" fill="url(#g)"/><circle cx="${ancho * 0.7}" cy="${alto * 0.3}" r="${alto * 0.1}" fill="#fff" opacity="0.7"/></svg>`
  );

function mediaInfoDemo(path) {
  const clase = claseDemo(path);
  if (clase === "audio") return { kind: "audio", width: null, height: null, duration: 187.4 };
  if (clase === "video") return { kind: "video", width: 1920, height: 1080, duration: 92.6 };
  return { kind: "imagen", width: 1600, height: 1000, duration: null };
}

function ondaDemo(cubos) {
  const onda = [];
  for (let i = 0; i < cubos; i++) {
    const envolvente = 0.5 + 0.5 * Math.sin(i / 23);
    onda.push(Math.min(1, 0.08 + envolvente * Math.abs(Math.sin(i * 1.7)) * 0.9));
  }
  return onda;
}

const api = demo
  ? {
      initial_paths: async () => [],
      file_info: async (path) => {
        const n = FOTOS_DEMO.indexOf(path);
        if (n >= 0) return { name: nombreDe(path), size: TAMANOS_DEMO[n] * 1024 * 1024, is_dir: false };
        return { name: nombreDe(path), size: 2.4 * 1024 * 1024, is_dir: false };
      },
      get_targets_many: async (paths) => {
        if (demoVarios === "variosnocomun") return [];
        if (paths.length > 1) return ["jpg", "webp", "gif", "bmp", "tiff", "avif"];
        return DESTINOS_DEMO[claseDemo(paths[0])] || DESTINOS_DEMO.audio;
      },
      get_tools: async (path) => herramientasDemo(path),
      run_tool: async (path, tool) => {
        await esperar(2500);
        if (params.get("demo") === "optimizado") throw "[optimized] " + t("demo.optimized");
        if (tool === "zip") return path.replace(/\.[^.\/]+$/, "") + ".zip";
        return path.replace(/(\.[^.\/]+)$/, ` (${t("demo.suffix." + tool)})$1`);
      },
      media_info: async (path) => mediaInfoDemo(path),
      preview_image: async () => SVG_DEMO(1600, 1000, 25),
      video_frame: async (path, seconds) => SVG_DEMO(1600, 900, 200 + Math.round(seconds * 3)),
      waveform: async (path, buckets) => ondaDemo(buckets),
      get_language: async () => (PREFS_DEMO.language === "auto" ? params.get("lang") || "en" : PREFS_DEMO.language),
      get_prefs: async () => ({ ...PREFS_DEMO, languages: [{ code: "en", name: "English" }, { code: "es", name: "Español" }, { code: "pt", name: "Português" }, { code: "fr", name: "Français" }], accents: ["orange", "red", "pink", "purple", "blue", "green"] }),
      set_language: async (valor) => (PREFS_DEMO.language = valor),
      set_accent: async (valor) => (PREFS_DEMO.accent = valor),
      get_config: async () => CONFIG_DEMO.map((k) => ({ ...k, enabled: k.enabled.slice() })),
      set_formats: async (tipo, formatos) => {
        const k = CONFIG_DEMO.find((c) => c.key === tipo);
        if (formatos.length < k.min || formatos.length > k.max) throw "limits";
        k.enabled = formatos.slice();
      },
      reset_formats: async (tipo) => {
        const k = CONFIG_DEMO.find((c) => c.key === tipo);
        k.enabled = k.all.slice();
      },
      resize_center: async () => {},
      show_wheel: async () => {},
      place_wheel: async () => {},
      center_wheel: async () => {},
      thumbnail: async () => {
        await esperar(500);
        return MINIATURA_DEMO;
      },
      on_progress: async (alProgreso) => {
        if (!demoVarios.startsWith("varios")) return progresoDemo(alProgreso);
        progresoGlobalDemo = alProgreso;
        return () => (progresoGlobalDemo = null);
      },
      on_file: async (alArchivo) => {
        archivoDemo = alArchivo;
        return () => (archivoDemo = null);
      },
      convert: async () => {
        await esperar(3200);
        return "/demo/ejemplo.mp3";
      },
      convert_many: convertirVariosDemo,
      cancel: async () => {},
      quit: async () => {},
    }
  : {
      initial_paths: () => tauri.core.invoke("initial_paths"),
      file_info: (path) => tauri.core.invoke("file_info", { path }),
      get_targets_many: (paths) => tauri.core.invoke("get_targets_many", { paths }),
      show_wheel: () => tauri.core.invoke("show_wheel"),
      place_wheel: (x, y) => tauri.core.invoke("place_wheel", { x, y }),
      center_wheel: () => tauri.core.invoke("center_wheel"),
      get_tools: (path) => tauri.core.invoke("get_tools", { path }),
      run_tool: (path, tool, option, params) => tauri.core.invoke("run_tool", { path, tool, option, params }),
      media_info: (path) => tauri.core.invoke("media_info", { path }),
      preview_image: (path, maxSide) => tauri.core.invoke("preview_image", { path, maxSide }),
      video_frame: (path, seconds, maxSide) => tauri.core.invoke("video_frame", { path, seconds, maxSide }),
      waveform: (path, buckets) => tauri.core.invoke("waveform", { path, buckets }),
      get_language: () => tauri.core.invoke("get_language"),
      get_prefs: () => tauri.core.invoke("get_prefs"),
      set_language: (value) => tauri.core.invoke("set_language", { value }),
      set_accent: (value) => tauri.core.invoke("set_accent", { value }),
      get_config: () => tauri.core.invoke("get_config"),
      set_formats: (kind, formats) => tauri.core.invoke("set_formats", { kind, formats }),
      reset_formats: (kind) => tauri.core.invoke("reset_formats", { kind }),
      resize_center: (width, height) => tauri.core.invoke("resize_center", { width, height }),
      thumbnail: (path) => tauri.core.invoke("thumbnail", { path }),
      on_progress: (alProgreso) => tauri.event.listen("convert-progress", (e) => alProgreso(e.payload.percent)),
      on_file: (alArchivo) => tauri.event.listen("convert-file", (e) => alArchivo(e.payload)),
      convert: (path, format) => tauri.core.invoke("convert", { path, format }),
      convert_many: (paths, format) => tauri.core.invoke("convert_many", { paths, format }),
      cancel: () => tauri.core.invoke("cancel"),
      quit: () => tauri.core.invoke("quit"),
    };

// ---------- Estado ----------

let vistaActual = "inicio"; // inicio | rueda | convirtiendo | tarjeta | editor
let rutas = [];
let infos = [];
let textoCentro = ""; // lo que muestra la píldora cuando no hay un segmento activo
let formatos = [];
let herramientas = []; // lo que devuelve get_tools (vacío con varios archivos)
let segmentos = []; // lo que muestra la rueda ahora: formatos, herramientas u opciones
let nivel = "conversion"; // conversion | herramientas | opciones
let herramienta = null; // la herramienta elegida (para su rueda de opciones)
let activo = -1;
let cancelando = false;
let soltado = false;
let cargando = null;
let temporizador = null;
let posRueda = null; // dónde estaba el cursor al abrir (para volver ahí tras cancelar)
let enTransicion = false; // la ventana se está moviendo: no cerrar ni cambiar de vista
let rearmarAbandono = null; // la pone vigilarAbandono (solo en el flujo A, el de Nautilus)

const $ = (id) => document.getElementById(id);
const vistas = { inicio: $("inicio"), rueda: $("vista-rueda"), tarjeta: $("vista-tarjeta") };
const LIMITE_ESPERA_HERRAMIENTAS = 400; // ms máximos esperando get_tools antes de pintar la rueda

function mostrarVista(nombre, estado) {
  vistaActual = estado || nombre;
  clearTimeout(temporizador);
  for (const clave in vistas) vistas[clave].hidden = clave !== nombre;
}

// ---------- Salir / volver ----------

// Flujo A: cierra la app. Flujo B: vuelve al círculo de inicio.
function salir() {
  if (modoLibre) mostrarInicio();
  else api.quit();
}

function mostrarInicio() {
  rutas = [];
  infos = [];
  formatos = [];
  herramientas = [];
  segmentos = [];
  nivel = "conversion";
  activo = -1;
  soltado = false;
  cargando = null;
  mostrarVista("inicio");
}

// ---------- Rueda ----------

function segmentoFormato(formato) {
  return { accion: "formato", valor: formato, texto: formato.toUpperCase(), aria: t("wheel.convertTo", { format: formato.toUpperCase() }) };
}

// Una herramienta sin opciones ni editor (p. ej. COMPRIMIR de un pdf) se ejecuta directo
function esDirecta(h) {
  return h.options.length === 0 && !h.editor;
}

function segmentoHerramienta(h) {
  const texto = etiqueta("tool", h);
  return { accion: "herramienta", valor: h, texto, icono: h.id, herr: true, aria: t("wheel.toolAria", { name: texto.toLowerCase() }) };
}

// Segmentos de la rueda según el nivel. En la de conversión, HERRAMIENTAS va al final (máximo 8 en total)
function armarSegmentos() {
  if (nivel === "herramientas") {
    segmentos = herramientas.map((h) => ({ ...segmentoHerramienta(h), herr: false }));
  } else if (nivel === "opciones") {
    segmentos = herramienta.options.map((o) => {
      const texto = etiqueta("option", o);
      return { accion: "opcion", valor: o.id, texto, aria: t("wheel.optionAria", { name: texto }) };
    });
  } else {
    segmentos = formatos.map(segmentoFormato);
    if (herramientas.length === 1 && esDirecta(herramientas[0]) && segmentos.length < 8) {
      segmentos.push(segmentoHerramienta(herramientas[0]));
    } else if (herramientas.length > 0 && segmentos.length < 8) {
      segmentos.push({ accion: "herramientas", texto: t("wheel.tools"), icono: "herramientas", herr: true, aria: t("wheel.openTools") });
    }
  }
}

function dibujarRueda() {
  armarSegmentos();
  const rueda = $("rueda");
  rueda.classList.toggle("herramientas", nivel !== "conversion");
  rueda.setAttribute("aria-label", { conversion: t("wheel.conversion"), herramientas: t("wheel.toolsCenter"), opciones: t("wheel.options") }[nivel]);
  $("ajustes").hidden = nivel !== "conversion";
  const grupo = $("segmentos");
  grupo.innerHTML = "";
  segmentos.forEach((seg, i) => {
    const g = document.createElementNS(NS, "g");
    g.setAttribute("class", "segmento" + (seg.herr ? " herr" : ""));
    g.setAttribute("role", "button");
    g.setAttribute("tabindex", "0");
    g.setAttribute("aria-label", seg.aria);

    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", rutaSegmento(i, segmentos.length));

    const texto = document.createElementNS(NS, "text");
    const [x, y] = punto((i + 0.5) * (360 / segmentos.length), (R_EXT + R_INT) / 2);
    texto.setAttribute("x", x);
    texto.setAttribute("y", seg.icono ? y + 13 : y);
    if (seg.texto.length > 9) texto.setAttribute("class", "largo");
    texto.textContent = seg.texto;

    g.append(path, texto);
    if (seg.icono) {
      const icono = document.createElementNS(NS, "svg");
      icono.setAttribute("class", "ico");
      icono.setAttribute("viewBox", "0 0 24 24");
      icono.setAttribute("x", x - 11);
      icono.setAttribute("y", y - 24);
      icono.setAttribute("width", 22);
      icono.setAttribute("height", 22);
      icono.innerHTML = ICONOS_HERR[seg.icono] || ICONOS_HERR.herramientas;
      g.append(icono);
    }
    g.addEventListener("focus", () => resaltar(i));
    g.addEventListener("keydown", (e) => teclaSegmento(e, i));
    grupo.append(g);
  });

  activo = -1;
  actualizarPildora();
}

// Cambia de rueda (conversión, herramientas u opciones) en el mismo componente
function irANivel(nuevo, h) {
  nivel = nuevo;
  if (h) herramienta = h;
  dibujarRueda();
  const vista = vistas.rueda;
  vista.style.animation = "none";
  void vista.offsetWidth;
  vista.style.animation = "";
}

function volverNivel() {
  if (nivel === "opciones") irANivel("herramientas");
  else if (nivel === "herramientas") irANivel("conversion");
}

// Sin bloquear: si la miniatura llega a tiempo sustituye al icono; si falla, se queda el icono
async function cargarMiniatura(lista) {
  try {
    const url = await api.thumbnail(lista[0]);
    if (rutas !== lista || !url) return;
    const img = crear("img", "miniatura");
    img.alt = "";
    img.onload = () => {
      if (rutas !== lista) return;
      const hijos = [img];
      if (lista.length > 1) hijos.push(crear("span", "insignia", String(lista.length)));
      $("centro-icono").replaceChildren(...hijos);
    };
    img.src = url;
  } catch (e) {}
}

// Icono del tipo común, o una pila de archivos si no todos son del mismo tipo
function tipoCentro() {
  const tipos = infos.map((i) => tipoDe(i.name, i.is_dir));
  if (tipos.every((t) => t === tipos[0])) return tipos[0];
  return "pila";
}

function actualizarPildora() {
  const pildora = $("pildora");
  const base = nivel === "herramientas" ? t("wheel.toolsCenter") : nivel === "opciones" ? etiqueta("tool", herramienta) : textoCentro;
  const texto = activo >= 0 ? segmentos[activo].texto : base;
  pildora.textContent = texto;
  pildora.classList.toggle("largo", texto.length > 10);
  pildora.classList.toggle("formato", activo >= 0);
  pildora.classList.toggle("varios", rutas.length > 1 && activo < 0 && nivel === "conversion");
  $("rueda").classList.toggle("sub", nivel !== "conversion");
}

function resaltar(i) {
  if (i === activo) return;
  activo = i;
  document.querySelectorAll(".segmento").forEach((g, n) => g.classList.toggle("activo", n === i));
  actualizarPildora();
}

function mostrarRueda() {
  mostrarVista("rueda");
  // Al volver a la rueda (de los ajustes, del editor o de cancelar) el cierre por inactividad
  // vuelve a contar; si no, la rueda se quedaba abierta hasta que se moviera el ratón
  if (rearmarAbandono) rearmarAbandono();
}

// posición en px CSS de la ventana -> índice de segmento (o -1)
function segmentoBajo(x, y) {
  const caja = $("rueda").getBoundingClientRect();
  const escala = 460 / caja.width;
  const dx = (x - (caja.left + caja.width / 2)) * escala;
  const dy = (y - (caja.top + caja.height / 2)) * escala;
  return segmentoEn(dx, dy, segmentos.length);
}

// distancia al centro de la rueda, en unidades del viewBox
function distanciaAlCentro(x, y) {
  const caja = $("rueda").getBoundingClientRect();
  const escala = 460 / caja.width;
  const dx = (x - (caja.left + caja.width / 2)) * escala;
  const dy = (y - (caja.top + caja.height / 2)) * escala;
  return Math.hypot(dx, dy);
}

function fueraDeRueda(x, y) {
  return distanciaAlCentro(x, y) > R_EXT;
}

function teclaSegmento(e, i) {
  const total = segmentos.length;
  const elementos = document.querySelectorAll(".segmento");
  if (e.key === "ArrowRight" || e.key === "ArrowDown") {
    elementos[(i + 1) % total].focus();
  } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
    elementos[(i - 1 + total) % total].focus();
  } else if (e.key === "Enter" || e.key === " ") {
    elegir(i);
  } else {
    return;
  }
  e.preventDefault();
}

// ---------- Tarjetas ----------

function boton(texto, alHacerClic, clase) {
  const b = crear("button", clase || "boton", texto);
  b.type = "button";
  b.addEventListener("click", alHacerClic);
  return b;
}

function mostrarTarjeta(clase, hijos, estado) {
  const tarjeta = crear("div", "tarjeta " + clase);
  tarjeta.append(...hijos);
  vistas.tarjeta.replaceChildren(tarjeta);
  mostrarVista("tarjeta", estado);
  return tarjeta;
}

function mostrarAviso(titulo, detalle, clase) {
  const hijos = [crear("h2", "", titulo)];
  if (detalle) hijos.push(crear("p", "detalle", detalle));
  hijos.push(boton(t("result.close"), salir));
  const tarjeta = mostrarTarjeta("aviso " + (clase || ""), hijos);
  tarjeta.setAttribute("role", "alert");
}

function mostrarError(mensaje, titulo) {
  const tarjeta = mostrarTarjeta(
    "error",
    [crear("h2", "", titulo || t("error.convert")), crear("p", "detalle", sinCodigo(mensaje)), boton(t("result.close"), salir)],
    "tarjeta"
  );
  tarjeta.setAttribute("role", "alert");
}

// Nombre que tendrá la salida con un nombre personalizado: el del usuario + la extensión del original
function nombreConExtension(nombre, original) {
  const ext = extDe(original);
  nombre = nombre.trim();
  if (!ext || nombre.toLowerCase().endsWith("." + ext)) return nombre;
  return nombre + "." + ext;
}

function mostrarProgreso(textoTitulo) {
  const cancelar = boton("✕", cancelar_, "equis");
  cancelar.setAttribute("aria-label", t("progress.cancel"));
  const barra = crear("div", "barra");
  barra.append(crear("div", "relleno"));
  const titulo = crear("h2", "", textoTitulo);
  const detalle = crear("p", "detalle", infos[0].name);
  const tarjeta = mostrarTarjeta(rutas.length > 1 ? "progreso varios" : "progreso", [cancelar, titulo, detalle, barra], "convirtiendo");
  tarjeta.setAttribute("role", "status");
  return { relleno: barra.firstChild, titulo, detalle };
}

// La barra tiene dos modos:
// - Progreso real (ffmpeg y los lotes): empieza en 0 y sigue solo los eventos del backend. Si en
//   3 s no llega ninguno (por ejemplo, ffprobe no pudo leer la duración), pasa a la simulada.
// - Simulada (imágenes, documentos...): sube rápido y se frena cerca del 92 %; no mide nada real.
// La barra nunca retrocede. Al terminar la conversión se rellena hasta el 100 % poco a poco, en
// el tiempo que falte hasta MIN_PROGRESO, para que no salte.
function animarBarra(relleno, titulo, base, mostrarPorcentaje, progresoReal) {
  let ancho = 0;
  let real = 0;
  let reloj = null;
  let recibioEvento = false;
  const poner = (valor) => {
    ancho = Math.max(ancho, valor);
    relleno.style.width = ancho + "%";
  };
  const texto = (valor) => {
    if (mostrarPorcentaje && progresoReal) titulo.textContent = t("progress.percent", { base, percent: Math.round(valor) });
  };
  const simular = () => {
    const inicio = Date.now();
    const paso = () => poner(Math.max(ancho, 92 * (1 - Math.exp(-(Date.now() - inicio) / 1100))));
    paso();
    reloj = setInterval(paso, 80);
  };
  let espera = null;
  if (progresoReal) {
    texto(0);
    espera = setTimeout(() => !recibioEvento && simular(), 3000);
  } else {
    simular();
  }
  const parar = () => {
    clearInterval(reloj);
    clearTimeout(espera);
  };
  return {
    detener: parar,
    progreso: (porcentaje) => {
      recibioEvento = true;
      parar();
      real = Math.max(real, Math.min(100, porcentaje));
      poner(real);
      texto(real);
    },
    completar: async (milisegundos) => {
      parar();
      texto(100);
      relleno.style.transitionDuration = milisegundos + "ms";
      ancho = 100;
      relleno.style.width = "100%";
      await esperar(milisegundos + 200); // que se vea llena un instante antes de pasar a "Listo"
    },
  };
}

// ¿El backend informa del avance real? Sí con ffmpeg (audio, vídeo, subtítulos) y con lotes
function conProgresoReal() {
  if (rutas.length > 1) return true;
  const nombre = infos[0].name;
  const ext = nombre.includes(".") ? nombre.split(".").pop().toLowerCase() : "";
  const tipo = tipoDe(nombre, infos[0].is_dir);
  return tipo === "audio" || tipo === "video" || ext === "srt" || ext === "vtt";
}

function textoSalida(salida) {
  const nombre = nombreDe(salida);
  return salidaEsCarpeta(salida) ? t("result.folder", { name: nombre }) : nombre;
}

function mostrarListo(detalle) {
  const tarjeta = mostrarTarjeta("listo", [crear("div", "check", "✓"), crear("h2", "", t("result.done")), crear("p", "detalle", detalle)]);
  tarjeta.setAttribute("role", "status");

  let restante = TIEMPO_CIERRE;
  let desde = 0;
  const arrancar = () => {
    desde = Date.now();
    temporizador = setTimeout(salir, restante);
  };
  tarjeta.addEventListener("mouseenter", () => {
    clearTimeout(temporizador);
    restante -= Date.now() - desde;
  });
  tarjeta.addEventListener("mouseleave", arrancar);
  arrancar();
}

// Sin una llamada extra al backend: una salida sin extensión se considera carpeta
function salidaEsCarpeta(salida) {
  return !nombreDe(salida).includes(".");
}

// ---------- Conversión ----------

// Mueve la ventana (con el escenario oculto un instante) y luego muestra la vista pedida.
// destino: "centro" para los mensajes, "cursor" para volver a la rueda donde se abrió.
async function moverYMostrar(destino, mostrar) {
  const mover = !modoLibre && !demo && posRueda;
  if (!mover) return mostrar();
  const escenario = $("escenario");
  enTransicion = true;
  escenario.classList.add("oculto");
  await esperar(130);
  if (destino === "centro") await api.center_wheel();
  else await api.place_wheel(posRueda.x, posRueda.y);
  await esperar(100);
  const resultado = mostrar();
  escenario.classList.remove("oculto");
  enTransicion = false;
  return resultado;
}

function volverTrasCancelar() {
  if (modoLibre) mostrarInicio();
  else moverYMostrar("cursor", mostrarRueda);
}

function cancelar_() {
  cancelando = true;
  api.cancel();
}

// Resultado de convert_many: todo bien, fallo parcial o todo fallado
function mostrarResultadoVarios(resultado, formato) {
  const { outputs, skipped, failed } = resultado;
  if (failed.length === 0) {
    let texto = window.I18n.plural("result.converted", outputs.length);
    if (skipped.length > 0) texto += " · " + window.I18n.plural("result.skipped", skipped.length, { format: formato.toUpperCase() });
    mostrarListo(texto);
  } else if (outputs.length === 0 && skipped.length === 0) {
    mostrarError(failed[0].error);
  } else {
    const lineas = failed.slice(0, 3).map((f) => `${f.name}: ${f.error}`);
    if (failed.length > 3) lineas.push(t("result.moreErrors", { count: failed.length - 3 }));
    mostrarAviso(t("result.partial", { ok: outputs.length, total: rutas.length }), lineas.join("\n"), "parcial");
  }
}

async function convertir(i) {
  if (vistaActual !== "rueda" || enTransicion) return;
  const formato = segmentos[i].valor;
  const varios = rutas.length > 1;
  cancelando = false;
  // Los mensajes (progreso, resultado, errores) van en el centro de la pantalla
  const { relleno, titulo, detalle } = await moverYMostrar("centro", () => mostrarProgreso(t("progress.converting", { format: formato.toUpperCase() })));
  const barra = animarBarra(relleno, titulo, t("progress.converting", { format: formato.toUpperCase() }), !varios, conProgresoReal());
  const inicio = Date.now();
  let escuchas = [];
  const dejarDeEscuchar = () => {
    escuchas.forEach((quitar) => quitar());
    escuchas = [];
  };
  const alArchivo = (e) => {
    titulo.textContent = t("progress.convertingOf", { format: formato.toUpperCase(), index: e.index, total: e.total });
    detalle.textContent = e.name;
  };
  try {
    // Los listeners se registran antes de convertir para no perder el primer evento
    escuchas.push(await api.on_progress(barra.progreso).catch(() => null));
    if (varios) escuchas.push(await api.on_file(alArchivo).catch(() => null));
    escuchas = escuchas.filter(Boolean);
    const salida = varios ? await api.convert_many(rutas, formato) : await api.convert(rutas[0], formato);
    dejarDeEscuchar();
    // Si terminó enseguida, la barra se rellena poco a poco hasta cumplir MIN_PROGRESO
    await barra.completar(Math.max(450, MIN_PROGRESO - (Date.now() - inicio)));
    if (varios) mostrarResultadoVarios(salida, formato);
    else mostrarListo(textoSalida(salida));
  } catch (e) {
    dejarDeEscuchar();
    barra.detener();
    const mensaje = String(e);
    if (cancelando || conCodigo(mensaje, "cancelled")) volverTrasCancelar();
    else mostrarError(mensaje);
  }
}

// ---------- Herramientas ----------

// Qué hace al elegir un segmento, según la rueda en que esté
function elegir(i) {
  if (vistaActual !== "rueda" || enTransicion) return;
  const seg = segmentos[i];
  if (seg.accion === "formato") convertir(i);
  else if (seg.accion === "herramientas") irANivel("herramientas");
  else if (seg.accion === "opcion") ejecutarHerramienta(herramienta, seg.valor, null);
  else if (seg.valor.options.length > 0) irANivel("opciones", seg.valor);
  else if (seg.valor.editor) abrirEditor(seg.valor);
  else ejecutarHerramienta(seg.valor, null, null);
}

// Las herramientas de audio y vídeo usan ffmpeg y dan el progreso real
function herramientaConProgresoReal() {
  const tipo = tipoDe(infos[0].name, infos[0].is_dir);
  return tipo === "audio" || tipo === "video";
}

// Cancelar una herramienta vuelve a la rueda en la que se abrió, en el sitio del cursor
function volverTrasCancelarHerramienta() {
  moverYMostrar("cursor", mostrarRueda);
}

// yaCentrada: viene del editor, que ya dejó la ventana en el centro
async function ejecutarHerramienta(tool, opcion, parametros, yaCentrada) {
  cancelando = false;
  const base = t(window.I18n.has("progress." + tool.id) ? "progress." + tool.id : "progress.processing");
  const mostrar = () => mostrarProgreso(base + "...");
  const { relleno, titulo, detalle } = yaCentrada ? mostrar() : await moverYMostrar("centro", mostrar);
  if (parametros && parametros.nombre) detalle.textContent = nombreConExtension(parametros.nombre, infos[0].name);
  const barra = animarBarra(relleno, titulo, base, true, herramientaConProgresoReal());
  const inicio = Date.now();
  let quitar = null;
  try {
    quitar = await api.on_progress(barra.progreso).catch(() => null);
    const salida = await api.run_tool(rutas[0], tool.id, opcion, parametros);
    if (quitar) quitar();
    await barra.completar(Math.max(450, MIN_PROGRESO - (Date.now() - inicio)));
    mostrarListo(textoSalida(salida));
  } catch (e) {
    if (quitar) quitar();
    barra.detener();
    const mensaje = String(e);
    if (cancelando || conCodigo(mensaje, "cancelled")) volverTrasCancelarHerramienta();
    else if (conCodigo(mensaje, "optimized")) mostrarAviso(sinCodigo(mensaje), infos[0].name);
    else mostrarError(mensaje, t("error.tool"));
  }
}

// ---------- Editor visual (recortar) ----------

function editorNoDisponible(contenedor) {
  return new Promise((resolver) => {
    const cerrar = boton(t("result.close"), () => resolver(null));
    const caja = crear("div", "editor-falta");
    caja.append(crear("h2", "", t("editor.unavailable")), cerrar);
    contenedor.replaceChildren(caja);
    cerrar.focus();
  });
}

async function abrirEditor(tool) {
  const tipo = tipoDe(infos[0].name, infos[0].is_dir);
  const escenario = $("escenario");
  const contenedor = $("editor-contenedor");
  enTransicion = true;
  escenario.classList.add("oculto");
  await esperar(130);
  await api.resize_center(780, 640);
  await esperar(100);
  escenario.hidden = true;
  escenario.classList.remove("oculto");
  $("vista-editor").hidden = false;
  vistaActual = "editor";
  enTransicion = false;

  let resultado = null;
  try {
    if (window.Editor) {
      const f = { media_info: api.media_info, preview_image: api.preview_image, video_frame: api.video_frame, waveform: api.waveform };
      resultado = await window.Editor.abrir({ tipo, ruta: rutas[0], nombre: infos[0].name, contenedor, api: f });
    } else {
      resultado = await editorNoDisponible(contenedor);
    }
  } catch (e) {
    resultado = null;
  }
  await esperar(80); // que termine el clic de Aplicar/Cancelar antes de cambiar de vista

  // Se cierra el editor y se devuelve la ventana a su tamaño de rueda (460x460)
  enTransicion = true;
  $("vista-editor").hidden = true;
  contenedor.replaceChildren();
  escenario.classList.add("oculto");
  escenario.hidden = false;
  vistaActual = "rueda";
  const alCursor = resultado === null && !modoLibre && posRueda;
  if (alCursor) await api.place_wheel(posRueda.x, posRueda.y);
  else await api.center_wheel();
  await esperar(100);
  enTransicion = false;
  if (resultado === null) {
    mostrarRueda();
    escenario.classList.remove("oculto");
  } else {
    const ejecucion = ejecutarHerramienta(tool, null, resultado, true);
    escenario.classList.remove("oculto");
    await ejecucion;
  }
}

// ---------- Ajustes (formatos de la rueda) ----------

async function abrirAjustes() {
  if (vistaActual !== "rueda" || enTransicion || nivel !== "conversion") return;
  const escenario = $("escenario");
  const contenedor = $("ajustes-contenedor");
  enTransicion = true;
  escenario.classList.add("oculto");
  await esperar(130);
  await api.resize_center(600, 660);
  await esperar(100);
  escenario.hidden = true;
  escenario.classList.remove("oculto");
  $("vista-ajustes").hidden = false;
  vistaActual = "ajustes";
  enTransicion = false;

  let cambio = false;
  try {
    if (window.Ajustes) {
      const f = {
        get_config: api.get_config, set_formats: api.set_formats, reset_formats: api.reset_formats,
        get_prefs: api.get_prefs, set_language: api.set_language, set_accent: api.set_accent,
      };
      cambio = await window.Ajustes.abrir({ contenedor, api: f, alCambiarIdioma: cambiarIdioma });
    }
  } catch (e) {
    // Si la pantalla de ajustes falla, la rueda sigue como estaba
  }
  await esperar(80);

  // Se cierra la pantalla y la ventana vuelve a su tamaño de rueda (460x460)
  enTransicion = true;
  $("vista-ajustes").hidden = true;
  contenedor.replaceChildren();
  escenario.classList.add("oculto");
  escenario.hidden = false;
  vistaActual = "rueda";
  if (!modoLibre && posRueda) await api.place_wheel(posRueda.x, posRueda.y);
  else await api.center_wheel();
  if (cambio) {
    // La rueda se arma otra vez con los formatos nuevos
    try {
      const lista = await api.get_targets_many(rutas);
      if (lista.length > 0) {
        formatos = lista.slice(0, 8);
        activo = -1;
        dibujarRueda();
      }
    } catch (e) {
      // Se queda la lista de formatos que ya había
    }
  }
  if (cambio) {
    // Por si cambió el idioma: el texto del centro y las etiquetas de la rueda se vuelven a pintar
    textoCentro = textoDeCentro();
    dibujarRueda();
    actualizarPildora();
  }
  await esperar(100);
  enTransicion = false;
  mostrarRueda();
  escenario.classList.remove("oculto");
}

// ---------- Cargar un archivo ----------

// Lo que muestra la píldora del centro: el tamaño, o "N archivos · tamaño" (depende del idioma)
function textoDeCentro() {
  const total = infos.reduce((suma, i) => suma + i.size, 0);
  return rutas.length === 1 ? formatearTamano(total) : t("wheel.centerMany", { count: rutas.length, size: formatearTamano(total) });
}

async function cargarArchivos(nuevas) {
  rutas = nuevas;
  activo = -1;
  nivel = "conversion";
  const inicio = Date.now();
  try {
    // Con varios archivos, si uno falla solo se ignora su tamaño
    const pedirInfos =
      nuevas.length === 1
        ? Promise.all([api.file_info(nuevas[0])])
        : Promise.all(nuevas.map((r) => api.file_info(r).catch(() => ({ name: nombreDe(r), size: 0, is_dir: false }))));
    // Las herramientas solo con un archivo; se piden a la vez, pero sin esperar más de 400 ms
    const pedirHerramientas = nuevas.length === 1 ? api.get_tools(nuevas[0]).catch(() => []) : Promise.resolve([]);
    const [datos, lista] = await Promise.all([pedirInfos, api.get_targets_many(nuevas)]);
    const sinHerramientas = esperar(Math.max(0, LIMITE_ESPERA_HERRAMIENTAS - (Date.now() - inicio))).then(() => []);
    const tools = await Promise.race([pedirHerramientas, sinHerramientas]);
    if (rutas !== nuevas) return false;
    infos = datos;
    formatos = lista.slice(0, 8);
    herramientas = Array.isArray(tools) ? tools : [];
  } catch (e) {
    if (rutas !== nuevas) return false;
    mostrarError(String(e));
    return false;
  }
  if (formatos.length === 0) {
    if (nuevas.length === 1) mostrarAviso(t("error.notSupported"), infos[0].name);
    else mostrarAviso(t("error.noCommon"), t("error.noCommonDetail", { count: nuevas.length }));
    return false;
  }
  textoCentro = textoDeCentro();
  $("centro-icono").innerHTML = iconoSvg(tipoCentro());
  dibujarRueda();
  mostrarRueda();
  cargarMiniatura(nuevas);
  return true;
}

function mismasRutas(a, b) {
  return a.length === b.length && a.every((r, i) => r === b[i]);
}

// ---------- Eventos ----------

function alMoverRaton(e) {
  if (vistaActual !== "rueda") return;
  resaltar(segmentoBajo(e.clientX, e.clientY));
  // El engranaje de ajustes solo se ve con el ratón sobre el círculo central
  $("ajustes").classList.toggle("visible", distanciaAlCentro(e.clientX, e.clientY) < R_INT);
}

function alHacerClic(e) {
  // Los clics del editor (Aplicar, Cancelar...) no son de la rueda: sin esto, el clic de Aplicar
  // llegaba aquí con la vista ya cambiada, caía "fuera de la rueda" y cerraba la app
  // El engranaje tiene su propio manejador; con el teclado su clic llega con coordenadas 0,0 y pasaría por "fuera de la rueda"
  if (e.target.closest && e.target.closest("#vista-editor, #vista-ajustes, #ajustes")) return;
  if (vistaActual === "rueda") {
    const i = segmentoBajo(e.clientX, e.clientY);
    if (i >= 0) elegir(i);
    else if (nivel !== "conversion" && distanciaAlCentro(e.clientX, e.clientY) < R_INT) volverNivel();
    else if (fueraDeRueda(e.clientX, e.clientY)) salir();
  } else if (vistaActual === "inicio" && demo && e.target.closest("#circulo")) {
    cargarArchivos(["/demo/ejemplo.wav"]);
  }
}

function alPulsarTecla(e) {
  if (e.key !== "Escape" || vistaActual === "convirtiendo" || vistaActual === "inicio" || vistaActual === "editor" || vistaActual === "ajustes") return;
  if (vistaActual === "rueda" && nivel !== "conversion") volverNivel();
  else salir();
}

async function alArrastrar(evento) {
  const datos = evento.payload;
  if (!modoLibre || vistaActual === "convirtiendo") return;
  const dpr = window.devicePixelRatio || 1;

  if (datos.type === "enter") {
    soltado = false;
    cargando = cargarArchivos(datos.paths);
  } else if (datos.type === "over") {
    if (vistaActual === "rueda") resaltar(segmentoBajo(datos.position.x / dpr, datos.position.y / dpr));
  } else if (datos.type === "drop") {
    soltado = true;
    if (!mismasRutas(rutas, datos.paths) || !cargando) cargando = cargarArchivos(datos.paths);
    if (!(await cargando) || vistaActual !== "rueda") return;
    const i = segmentoBajo(datos.position.x / dpr, datos.position.y / dpr);
    if (i >= 0) {
      resaltar(i);
      elegir(i);
    }
  } else if (datos.type === "leave") {
    if (!soltado) mostrarInicio();
  }
}

// Pide mostrar la ventana. Con un archivo, la ventana empieza a pantalla completa y transparente
// solo para enterarse de dónde está el cursor (la app no puede saberlo en Wayland, pero la página
// recibe el cursor en cuanto la ventana aparece debajo de él); luego place_wheel la encoge a
// 460x460 en ese punto y la rueda queda centrada en ella.
// No se usa requestAnimationFrame: con la ventana oculta WebKit no lo dispara.
async function mostrarVentana() {
  const escenario = $("escenario");
  const sobreCursor = !modoLibre && !demo;
  if (sobreCursor) escenario.classList.add("oculto");
  await esperar(60);
  await api.show_wheel();
  if (sobreCursor && vistaActual === "tarjeta") {
    // Un mensaje desde el principio (p. ej. "Formato no soportado") va en el centro
    await api.center_wheel();
    await esperar(150);
  } else if (sobreCursor) {
    posRueda = await esperarCursor();
    await api.place_wheel(posRueda.x, posRueda.y);
    await esperar(150); // que la página se adapte al nuevo tamaño antes de mostrar la rueda
  }
  escenario.classList.remove("oculto");
  if (sobreCursor) vigilarAbandono();
}

// Espera el primer evento del cursor (máximo 600 ms). Si no llega ninguno, usa el centro de la pantalla.
function esperarCursor() {
  const eventos = ["mousemove", "mouseover", "pointermove"];
  return new Promise((resolver) => {
    const terminar = (punto) => {
      for (const nombre of eventos) document.removeEventListener(nombre, alCursor);
      clearTimeout(limite);
      resolver(punto);
    };
    const alCursor = (e) => terminar({ x: e.clientX, y: e.clientY });
    const limite = setTimeout(() => terminar({ x: window.innerWidth / 2, y: window.innerHeight / 2 }), 600);
    for (const nombre of eventos) document.addEventListener(nombre, alCursor);
  });
}

// La ventana ya no cubre la pantalla, así que un clic fuera no la cierra: se cierra al perder el
// foco o tras 4 s sin tocar nada (solo mientras está la rueda abierta; no mientras convierte).
function vigilarAbandono() {
  let reloj;
  const reiniciar = () => {
    clearTimeout(reloj);
    // Se vuelve a comprobar al disparar: si mientras tanto empezó una conversión, no se cierra
    if (vistaActual === "rueda") reloj = setTimeout(() => vistaActual === "rueda" && !enTransicion && salir(), TIEMPO_ABANDONO);
  };
  document.addEventListener("mousemove", reiniciar);
  document.addEventListener("keydown", reiniciar);
  rearmarAbandono = reiniciar;
  reiniciar();
  // Se activa un poco después: al cambiar de tamaño la ventana puede perder el foco un instante
  setTimeout(() => window.addEventListener("blur", () => vistaActual === "rueda" && !enTransicion && salir()), 1000);
}

// Pinta la app con el color elegido en los ajustes; si falla se queda el naranja de siempre
async function aplicarAcentoGuardado() {
  try {
    const prefs = await api.get_prefs();
    if (window.Ajustes) window.Ajustes.aplicarAcento(prefs.accent);
  } catch (e) {}
}

async function iniciar() {
  await elegirIdioma();
  await aplicarAcentoGuardado();
  document.addEventListener("mousemove", alMoverRaton);
  $("ajustes").addEventListener("click", abrirAjustes);
  document.addEventListener("click", alHacerClic);
  document.addEventListener("keydown", alPulsarTecla);
  $("rueda").addEventListener("mouseleave", () => {
    if (vistaActual === "rueda") resaltar(-1);
    $("ajustes").classList.remove("visible"); // sin mousemove al salir, el engranaje se quedaba visible
  });

  if (!demo) tauri.webview.getCurrentWebview().onDragDropEvent(alArrastrar);

  try {
    const iniciales = demo ? [] : await api.initial_paths().catch(() => []);
    if (iniciales.length > 0) {
      modoLibre = false;
      await cargarArchivos(iniciales);
    } else {
      mostrarInicio();
      if (demo) await estadoDemo(params.get("demo"));
    }
  } finally {
    // Siempre se muestra la ventana: si algo falla antes, quedaría invisible
    mostrarVentana();
  }
}

// En demo, "?demo=rueda|convirtiendo|listo|error|aviso" salta a esa vista.
// Con varios archivos: variosrueda | varios | variossaltados | variosfallo | variosnocomun
// Herramientas: herramientas | opciones | editorimagen | editoraudio | editorvideo | herramientaejecutando
// (con &tipo=imagen|audio|video|pdf para elegir el archivo de la demo)
async function estadoDemo(estado) {
  if (!estado) return;
  const varios = estado.startsWith("varios");
  const deHerramientas = ["herramientas", "opciones", "herramientaejecutando", "optimizado"].includes(estado) || estado.startsWith("editor");
  let ruta = rutaDemo("audio");
  if (deHerramientas) ruta = rutaDemo({ opciones: "imagen", editorimagen: "imagen", editoraudio: "audio" }[estado] || "video");
  if (estado.startsWith("editor")) ruta = ARCHIVOS_DEMO[estado.slice(6)] || ruta;
  await cargarArchivos(varios ? FOTOS_DEMO : [ruta]);
  if (varios && estado !== "variosrueda" && estado !== "variosnocomun") convertir(0);
  if (estado === "convirtiendo") convertir(0);
  if (estado === "listo") mostrarListo("ejemplo.mp3");
  if (estado === "error") mostrarError(t("demo.ffmpegMissing"));
  if (estado === "aviso") mostrarAviso(t("error.notSupported"), "ejemplo.xyz");
  if (deHerramientas) await demoHerramientas(estado);
}

async function demoHerramientas(estado) {
  if (herramientas.length === 0) return;
  if (herramientas.length > 1) irANivel("herramientas");
  const buscar = (id) => herramientas.find((h) => h.id === id);
  if (estado === "opciones" && buscar("resize")) irANivel("opciones", buscar("resize"));
  if (estado.startsWith("editor") && buscar("trim")) abrirEditor(buscar("trim"));
  if (estado === "herramientaejecutando" || estado === "optimizado") ejecutarHerramienta(buscar("compress") || herramientas[0], null, null);
}

if (typeof document !== "undefined") iniciar();

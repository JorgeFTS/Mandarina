// Visual trim editor: a box over an image, or start and end in time for audio and video.
// Uso: window.Editor.abrir({ tipo, ruta, nombre, contenedor, api }) -> Promise<params | null>
// It lives inside a function so it does not clash with the names in main.js.
(function () {
  const MIN_HUECO = 0.1; // minimum seconds between start and end
  const MIN_LADO = 4; // minimum pixels of the box
  const PROPORCIONES = [[null, 0], ["1:1", 1], ["4:3", 4 / 3], ["16:9", 16 / 9], ["3:2", 3 / 2]];
  const TIRADORES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
  const FOTOGRAMAS = 10;
  const PARALELO = 3;
  const MAX_NOMBRE = 200;

  // In the browser index.html loads I18n; in Node (tests) the file is loaded
  const I18n = typeof window !== "undefined" && window.I18n ? window.I18n : require("./i18n.js");
  const tr = (clave, variables) => I18n.t(clave, variables);

  // ---------- Pure part (tested with Node) ----------

  function limitar(v, min, max) {
    return Math.min(Math.max(v, min), max);
  }

  function dosDigitos(n) {
    return String(n).padStart(2, "0");
  }

  // 83.4 -> "1:23.40"
  function formatearTiempo(segundos) {
    const cs = Math.round(Math.max(0, segundos) * 100);
    const m = Math.floor(cs / 6000);
    const s = Math.floor((cs % 6000) / 100);
    return `${m}:${dosDigitos(s)}.${dosDigitos(cs % 100)}`;
  }

  // Accepts "1:23.45", "83.4", "83,4" or "1:02:03". Returns seconds or null
  function parsearTiempo(texto) {
    const partes = String(texto).trim().replace(",", ".").split(":");
    if (partes.length > 3) return null;
    let total = 0;
    for (const parte of partes) {
      if (!/^\d+(\.\d+)?$/.test(parte)) return null;
      total = total * 60 + Number(parte);
    }
    return total;
  }

  function redondear2(n) {
    return Math.round(n * 100) / 100;
  }

  // Moves one end ("start" or "end") without crossing the other
  function moverExtremo(sel, cual, t, dur) {
    const hueco = Math.min(MIN_HUECO, dur);
    const valor = redondear2(t);
    if (cual === "start") return { start: limitar(valor, 0, sel.end - hueco), end: sel.end };
    return { start: sel.start, end: limitar(valor, sel.start + hueco, dur) };
  }

  function tiempoDesdeX(x, ancho, dur) {
    if (ancho <= 0) return 0;
    return limitar(x / ancho, 0, 1) * dur;
  }

  function porcentaje(t, dur) {
    return dur > 0 ? limitar(t / dur, 0, 1) * 100 : 0;
  }

  function seleccionValida(sel, dur) {
    if (!Number.isFinite(sel.start) || !Number.isFinite(sel.end)) return false;
    return sel.start >= 0 && sel.end <= dur + 1e-6 && sel.end - sel.start >= Math.min(MIN_HUECO, dur) - 1e-6;
  }

  function esTodoElTiempo(sel, dur) {
    return sel.start <= 0.004 && sel.end >= dur - 0.004;
  }

  function recuadroCompleto(img) {
    return { x: 0, y: 0, w: img.w, h: img.h };
  }

  function esImagenCompleta(r, img) {
    return r.x === 0 && r.y === 0 && r.w === img.w && r.h === img.h;
  }

  function recuadroValido(r, img) {
    const enteros = [r.x, r.y, r.w, r.h].every(Number.isInteger);
    return enteros && r.x >= 0 && r.y >= 0 && r.w >= 1 && r.h >= 1 && r.x + r.w <= img.w && r.y + r.h <= img.h;
  }

  // Screen coordinates -> real pixels of the image (they may fall outside the image)
  function pantallaAPixeles(clientX, clientY, caja, img) {
    return {
      x: ((clientX - caja.left) / caja.width) * img.w,
      y: ((clientY - caja.top) / caja.height) * img.h,
    };
  }

  // Edges (decimals) -> whole-number box inside the image
  function desdeBordes(izq, arr, der, aba, img) {
    const x = limitar(Math.round(izq), 0, img.w - 1);
    const y = limitar(Math.round(arr), 0, img.h - 1);
    const w = limitar(Math.round(der) - x, 1, img.w - x);
    const h = limitar(Math.round(aba) - y, 1, img.h - y);
    return { x, y, w, h };
  }

  function moverRecuadro(r, dx, dy, img) {
    return {
      x: limitar(Math.round(r.x + dx), 0, img.w - r.w),
      y: limitar(Math.round(r.y + dy), 0, img.h - r.h),
      w: r.w,
      h: r.h,
    };
  }

  // Forces the aspect ratio on the edges that were already moved, anchoring the opposite side or corner
  function forzarProporcion(b, r, t, img, ratio) {
    const oeste = t.includes("w");
    const norte = t.includes("n");
    const lateral = t.length === 1 && (t === "e" || t === "w");
    const ax = oeste ? r.x + r.w : r.x;
    const ay = norte ? r.y + r.h : r.y;
    const libreX = oeste ? ax : img.w - ax;
    const libreY = norte ? ay : img.h - ay;
    const w = b.der - b.izq;
    const h = b.aba - b.arr;
    let w2;
    let h2;
    if (lateral) {
      const cy = r.y + r.h / 2;
      w2 = Math.min(w, libreX, 2 * Math.min(cy, img.h - cy) * ratio);
      h2 = w2 / ratio;
      return { izq: oeste ? ax - w2 : ax, der: oeste ? ax : ax + w2, arr: cy - h2 / 2, aba: cy + h2 / 2 };
    }
    if (t === "n" || t === "s") {
      const cx = r.x + r.w / 2;
      h2 = Math.min(h, libreY, (2 * Math.min(cx, img.w - cx)) / ratio);
      w2 = h2 * ratio;
      return { izq: cx - w2 / 2, der: cx + w2 / 2, arr: norte ? ay - h2 : ay, aba: norte ? ay : ay + h2 };
    }
    const desdeAlto = h * ratio;
    w2 = Math.abs(w - r.w) >= Math.abs(desdeAlto - r.w) ? w : desdeAlto;
    w2 = Math.min(w2, libreX, libreY * ratio);
    h2 = w2 / ratio;
    return { izq: oeste ? ax - w2 : ax, der: oeste ? ax : ax + w2, arr: norte ? ay - h2 : ay, aba: norte ? ay : ay + h2 };
  }

  // t: "n", "s", "e", "w", "ne", "nw", "se" or "sw". dx and dy in real pixels from the initial box r
  function redimensionarRecuadro(r, t, dx, dy, img, ratio) {
    const min = Math.min(MIN_LADO, img.w, img.h);
    let b = { izq: r.x, arr: r.y, der: r.x + r.w, aba: r.y + r.h };
    if (t.includes("w")) b.izq = limitar(r.x + dx, 0, b.der - min);
    if (t.includes("e")) b.der = limitar(b.der + dx, b.izq + min, img.w);
    if (t.includes("n")) b.arr = limitar(r.y + dy, 0, b.aba - min);
    if (t.includes("s")) b.aba = limitar(b.aba + dy, b.arr + min, img.h);
    if (ratio) b = forzarProporcion(b, r, t, img, ratio);
    return desdeBordes(b.izq, b.arr, b.der, b.aba, img);
  }

  // Box with the requested aspect ratio, centered on the current one and not going outside it
  function ajustarProporcion(r, ratio, img) {
    if (!ratio) return r;
    const w = r.w / r.h > ratio ? r.h * ratio : r.w;
    const h = w / ratio;
    const cx = r.x + r.w / 2;
    const cy = r.y + r.h / 2;
    return desdeBordes(cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2, img);
  }

  // Applies what was typed in a field ("x", "y", "w" or "h"). Returns the new box or null
  function aplicarCampo(r, campo, texto, img) {
    if (!/^\s*\d+([.,]\d+)?\s*$/.test(String(texto))) return null;
    const n = Math.round(Number(String(texto).replace(",", ".")));
    const nuevo = { ...r };
    if (campo === "x") {
      nuevo.x = limitar(n, 0, img.w - 1);
      nuevo.w = Math.min(r.w, img.w - nuevo.x);
    } else if (campo === "y") {
      nuevo.y = limitar(n, 0, img.h - 1);
      nuevo.h = Math.min(r.h, img.h - nuevo.y);
    } else if (campo === "w") {
      nuevo.w = limitar(n, 1, img.w - r.x);
    } else {
      nuevo.h = limitar(n, 1, img.h - r.y);
    }
    return nuevo;
  }

  // "photo.v2.png" -> ".png"; with no extension (or a hidden name like ".bashrc") -> ""
  function extensionDe(archivo) {
    const texto = String(archivo || "");
    const punto = texto.lastIndexOf(".");
    if (punto <= 0 || punto === texto.length - 1) return "";
    return texto.slice(punto);
  }

  function sinExtension(archivo) {
    const texto = String(archivo || "");
    return texto.slice(0, texto.length - extensionDe(texto).length);
  }

  // "cuadro.jpg" -> "cuadro (trimmed)" / "cuadro (recortado)"
  function nombreInicial(archivo) {
    return tr("editor.name.initial", { name: sinExtension(archivo) || tr("editor.name.fallback") });
  }

  // Returns the error message for the typed name, or "" if it is valid. Typing the extension of the original is not an error
  function validarNombre(texto) {
    const nombre = String(texto).trim();
    if (nombre === "") return tr("editor.name.empty");
    if (nombre.includes("/")) return tr("editor.name.slash");
    if (nombre === "." || nombre === "..") return tr("editor.name.invalid");
    if (Array.from(nombre).length > MAX_NOMBRE) return tr("editor.name.tooLong");
    return "";
  }

  // ---------- DOM helpers ----------

  function el(etiqueta, clase, texto) {
    const nodo = document.createElement(etiqueta);
    if (clase) nodo.className = clase;
    if (texto !== undefined) nodo.textContent = texto;
    return nodo;
  }

  function mensajeDe(error) {
    if (!error) return "";
    if (typeof error === "string") return error;
    return error.message || String(error);
  }

  function capturar(nodo, evento) {
    try {
      nodo.setPointerCapture(evento.pointerId);
    } catch (e) {
      // without capture the drag keeps working while the pointer is over it
    }
  }

  function mostrarCarga(ctx, texto) {
    const centro = el("div", "ed-centro");
    centro.append(el("div", "ed-giro"), el("div", "", texto));
    ctx.cuerpo.replaceChildren(centro);
  }

  function mostrarError(ctx, titulo, error) {
    const centro = el("div", "ed-centro");
    const caja = el("div", "ed-error");
    caja.setAttribute("role", "alert");
    const detalle = mensajeDe(error);
    caja.textContent = detalle ? tr("error.withDetail", { title: titulo, detail: detalle }) : titulo;
    centro.append(caja);
    ctx.cuerpo.replaceChildren(centro);
    ctx.estadoAplicar(false, "");
  }

  // ---------- Frame: title, buttons and keyboard ----------

  let contadorNombres = 0;

  function crearMarco(op, resolver) {
    const ctx = { cerrado: false, limpiezas: [], obtenerParams: null, api: op.api, ruta: op.ruta };
    const subtitulos = { imagen: "editor.subtitle.image", audio: "editor.subtitle.audio", video: "editor.subtitle.video" };

    const raiz = el("div", "ed-raiz");
    raiz.setAttribute("role", "dialog");
    raiz.setAttribute("aria-modal", "true");
    raiz.setAttribute("aria-label", tr("editor.aria", { name: op.nombre }));
    raiz.tabIndex = -1;

    const cabecera = el("header", "ed-cabecera");
    cabecera.append(el("h2", "ed-titulo", op.nombre), el("span", "ed-subtitulo", subtitulos[op.tipo] ? tr(subtitulos[op.tipo]) : ""));
    const cuerpo = el("div", "ed-cuerpo");
    const pie = el("footer", "ed-pie");
    const aviso = el("p", "ed-aviso");
    aviso.setAttribute("role", "status");
    const botones = el("div", "ed-botones");
    const cancelar = el("button", "ed-boton", tr("editor.cancel"));
    cancelar.type = "button";
    const aplicar = el("button", "ed-boton ed-primario", tr("editor.apply"));
    aplicar.type = "button";
    aplicar.disabled = true;
    botones.append(aviso, cancelar, aplicar);

    // Name of the new file: an editable text and, on the right, the extension of the original
    const idNombre = `ed-nombre-${++contadorNombres}`;
    const grupoNombre = el("div", "ed-grupo ed-nombre");
    const etiquetaNombre = el("label", "ed-etiqueta", tr("editor.name.label"));
    etiquetaNombre.htmlFor = idNombre;
    const cajaNombre = el("div", "ed-nombrecaja");
    const entradaNombre = el("input", "ed-nombreentrada");
    entradaNombre.type = "text";
    entradaNombre.id = idNombre;
    entradaNombre.autocomplete = "off";
    entradaNombre.spellcheck = false;
    entradaNombre.value = nombreInicial(op.nombre);
    const extension = extensionDe(op.nombre);
    cajaNombre.append(entradaNombre);
    if (extension) cajaNombre.append(el("span", "ed-ext", extension));
    const errorNombre = el("p", "ed-nombre-error");
    errorNombre.id = `${idNombre}-error`;
    errorNombre.setAttribute("role", "alert");
    errorNombre.hidden = true;
    grupoNombre.append(etiquetaNombre, cajaNombre, errorNombre);
    pie.append(grupoNombre, botones);
    raiz.append(cabecera, cuerpo, pie);

    ctx.raiz = raiz;
    ctx.cuerpo = cuerpo;

    // Apply is only enabled if the selection and the name are valid; each one shows its own message
    ctx.seleccionOk = false;
    ctx.nombreOk = true;
    const refrescarAplicar = () => {
      aplicar.disabled = !(ctx.seleccionOk && ctx.nombreOk);
    };

    ctx.estadoAplicar = (ok, texto, esError) => {
      ctx.seleccionOk = Boolean(ok);
      aviso.textContent = texto || "";
      aviso.classList.toggle("ed-aviso-error", Boolean(esError));
      refrescarAplicar();
    };

    const revisarNombre = () => {
      const mensaje = validarNombre(entradaNombre.value);
      ctx.nombreOk = mensaje === "";
      errorNombre.textContent = mensaje;
      errorNombre.hidden = ctx.nombreOk;
      cajaNombre.classList.toggle("ed-invalido", !ctx.nombreOk);
      if (ctx.nombreOk) entradaNombre.removeAttribute("aria-invalid");
      else entradaNombre.setAttribute("aria-invalid", "true");
      if (ctx.nombreOk) entradaNombre.removeAttribute("aria-describedby");
      else entradaNombre.setAttribute("aria-describedby", errorNombre.id);
      refrescarAplicar();
    };
    revisarNombre();
    entradaNombre.addEventListener("input", revisarNombre);
    // When entering with a click or Tab the whole text is selected (the click must not undo the selection)
    let recienEnfocado = false;
    entradaNombre.addEventListener("focus", () => {
      recienEnfocado = true;
      entradaNombre.select();
    });
    entradaNombre.addEventListener("mouseup", (e) => {
      if (recienEnfocado) e.preventDefault();
      recienEnfocado = false;
    });

    ctx.terminar = (valor) => {
      if (ctx.cerrado) return;
      ctx.cerrado = true;
      document.removeEventListener("keydown", alTeclear, true);
      for (const limpiar of ctx.limpiezas) limpiar();
      ctx.limpiezas = [];
      op.contenedor.replaceChildren();
      resolver(valor);
    };

    const aplicarAhora = () => {
      if (aplicar.disabled || !ctx.obtenerParams) return;
      const params = ctx.obtenerParams();
      if (params) ctx.terminar({ ...params, nombre: entradaNombre.value.trim() });
    };

    function alTeclear(e) {
      const destino = e.target;
      const enCampo = destino instanceof HTMLInputElement;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        ctx.terminar(null);
      } else if (e.key === "Enter" && (destino === entradaNombre || (!enCampo && !(destino instanceof HTMLButtonElement)))) {
        e.preventDefault();
        e.stopPropagation();
        aplicarAhora();
      }
    }

    cancelar.addEventListener("click", () => ctx.terminar(null));
    aplicar.addEventListener("click", aplicarAhora);
    document.addEventListener("keydown", alTeclear, true);

    op.contenedor.replaceChildren(raiz);
    raiz.focus();
    return ctx;
  }

  // ---------- Modo imagen ----------

  function crearCampoNumerico(clave, texto) {
    const grupo = el("label", "ed-grupo");
    const entrada = el("input", "ed-campo");
    entrada.type = "text";
    entrada.inputMode = "numeric";
    entrada.autocomplete = "off";
    entrada.setAttribute("aria-label", tr("editor.field.pixels", { name: texto }));
    entrada.dataset.campo = clave;
    grupo.append(el("span", "", texto), entrada);
    return { grupo, entrada };
  }

  async function iniciarImagen(ctx) {
    mostrarCarga(ctx, tr("editor.image.loading"));
    let info;
    try {
      info = await ctx.api.media_info(ctx.ruta);
    } catch (e) {
      if (!ctx.cerrado) mostrarError(ctx, tr("editor.image.readError"), e);
      return;
    }
    if (ctx.cerrado) return;
    if (!info || !info.width || !info.height) {
      mostrarError(ctx, tr("editor.image.readError"), tr("editor.image.noSize"));
      return;
    }
    const img = { w: info.width, h: info.height };

    let url = null;
    let errorVista = null;
    try {
      url = await ctx.api.preview_image(ctx.ruta, 900);
    } catch (e) {
      errorVista = e;
    }
    if (ctx.cerrado) return;

    const estado = { r: recuadroCompleto(img), ratio: 0 };

    // Scene with the image and the box
    const escena = el("div", "ed-escena");
    const lienzo = el("div", "ed-lienzo");
    if (url) {
      const foto = el("img");
      foto.alt = tr("editor.image.previewAlt");
      foto.draggable = false;
      foto.src = url;
      lienzo.append(foto);
    } else {
      const sin = el(
        "div",
        "ed-sinvista",
        errorVista ? tr("editor.image.previewFailedDetail", { detail: mensajeDe(errorVista) }) : tr("editor.image.previewFailed")
      );
      sin.setAttribute("role", "alert");
      lienzo.append(sin);
    }
    const caja = el("div", "ed-caja");
    caja.tabIndex = 0;
    caja.setAttribute("role", "group");
    caja.setAttribute("aria-label", tr("editor.image.boxAria"));
    const tiradores = {};
    for (const lado of TIRADORES) {
      const tirador = el("div", "ed-tirador");
      tirador.dataset.t = lado;
      tirador.tabIndex = 0;
      tirador.setAttribute("role", "slider");
      tirador.setAttribute("aria-label", tr("editor.handle.aria", { name: tr("editor.handle." + lado) }));
      tirador.setAttribute("aria-valuemin", "0");
      tirador.setAttribute("aria-valuemax", String(lado.includes("e") || lado.includes("w") ? img.w : img.h));
      tiradores[lado] = tirador;
      caja.append(tirador);
    }
    lienzo.append(caja);
    escena.append(lienzo);

    // Panel: aspect ratios, fields and measurements
    const panel = el("div", "ed-panel");
    const grupoProp = el("div", "ed-grupo");
    const segmentos = el("div", "ed-segmentos");
    segmentos.setAttribute("role", "group");
    segmentos.setAttribute("aria-label", tr("editor.ratio.label"));
    const botonesProp = PROPORCIONES.map(([texto, valor]) => {
      const boton = el("button", "", texto || tr("editor.ratio.free"));
      boton.type = "button";
      boton.addEventListener("click", () => {
        estado.ratio = valor;
        estado.r = ajustarProporcion(estado.r, valor, img);
        dibujar();
      });
      segmentos.append(boton);
      return boton;
    });
    grupoProp.append(el("span", "", tr("editor.ratio.label")), segmentos);

    const campos = {
      x: crearCampoNumerico("x", "X"),
      y: crearCampoNumerico("y", "Y"),
      w: crearCampoNumerico("w", tr("editor.field.w")),
      h: crearCampoNumerico("h", tr("editor.field.h")),
    };
    const medida = el("div", "ed-grupo ed-medida");
    const medidaTexto = el("strong");
    medida.append(el("span", "", tr("editor.result")), medidaTexto);
    const restablecer = el("button", "ed-boton ed-chico", tr("editor.reset"));
    restablecer.type = "button";
    restablecer.addEventListener("click", () => {
      estado.r = recuadroCompleto(img);
      estado.ratio = 0;
      dibujar();
    });
    panel.append(grupoProp);
    for (const clave in campos) panel.append(campos[clave].grupo);
    panel.append(restablecer, medida);

    ctx.cuerpo.replaceChildren(escena, panel);

    function dibujar() {
      const r = estado.r;
      caja.style.left = `${(r.x / img.w) * 100}%`;
      caja.style.top = `${(r.y / img.h) * 100}%`;
      caja.style.width = `${(r.w / img.w) * 100}%`;
      caja.style.height = `${(r.h / img.h) * 100}%`;
      for (const clave in campos) campos[clave].entrada.value = String(r[clave]);
      medidaTexto.textContent = `${r.w} × ${r.h} px`;
      botonesProp.forEach((boton, i) => boton.setAttribute("aria-pressed", String(PROPORCIONES[i][1] === estado.ratio)));
      for (const t in tiradores) {
        const valor = t.includes("e") ? r.x + r.w : t.includes("w") ? r.x : t.includes("s") ? r.y + r.h : r.y;
        tiradores[t].setAttribute("aria-valuenow", String(valor));
      }
      if (esImagenCompleta(r, img)) ctx.estadoAplicar(false, tr("editor.nothingToTrim"));
      else ctx.estadoAplicar(true, "");
    }

    function cambiar(t, dx, dy, origen) {
      estado.r = t === "mover" ? moverRecuadro(origen, dx, dy, img) : redimensionarRecuadro(origen, t, dx, dy, img, estado.ratio);
      dibujar();
    }

    // Mouse and touch
    function empezarArrastre(e, t) {
      if (e.button !== undefined && e.button > 0) return;
      e.preventDefault();
      e.stopPropagation();
      const nodo = e.currentTarget;
      const origen = { ...estado.r };
      const area = lienzo.getBoundingClientRect();
      const inicio = pantallaAPixeles(e.clientX, e.clientY, area, img);
      capturar(nodo, e);
      nodo.focus();
      const mover = (ev) => {
        if (ev.pointerId !== e.pointerId) return;
        const p = pantallaAPixeles(ev.clientX, ev.clientY, area, img);
        cambiar(t, p.x - inicio.x, p.y - inicio.y, origen);
      };
      const soltar = (ev) => {
        if (ev.pointerId !== e.pointerId) return;
        nodo.removeEventListener("pointermove", mover);
        nodo.removeEventListener("pointerup", soltar);
        nodo.removeEventListener("pointercancel", soltar);
      };
      nodo.addEventListener("pointermove", mover);
      nodo.addEventListener("pointerup", soltar);
      nodo.addEventListener("pointercancel", soltar);
    }

    // Keyboard: arrows 1 px, with Shift 10 px
    function alTeclear(e, t) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const paso = e.shiftKey ? 10 : 1;
      const flechas = { ArrowLeft: [-paso, 0], ArrowRight: [paso, 0], ArrowUp: [0, -paso], ArrowDown: [0, paso] };
      const delta = flechas[e.key];
      if (!delta) return;
      e.preventDefault();
      e.stopPropagation();
      cambiar(t, delta[0], delta[1], { ...estado.r });
    }

    caja.addEventListener("pointerdown", (e) => empezarArrastre(e, "mover"));
    caja.addEventListener("keydown", (e) => alTeclear(e, "mover"));
    for (const t in tiradores) {
      tiradores[t].addEventListener("pointerdown", (e) => empezarArrastre(e, t));
      tiradores[t].addEventListener("keydown", (e) => alTeclear(e, t));
    }

    // Numeric fields
    function confirmarCampo(clave) {
      const nuevo = aplicarCampo(estado.r, clave, campos[clave].entrada.value, img);
      if (!nuevo) {
        campos[clave].entrada.value = String(estado.r[clave]);
        ctx.estadoAplicar(!esImagenCompleta(estado.r, img), tr("editor.image.integer"), true);
        return;
      }
      estado.r = nuevo;
      estado.ratio = 0;
      dibujar();
    }
    for (const clave in campos) {
      campos[clave].entrada.addEventListener("change", () => confirmarCampo(clave));
      campos[clave].entrada.addEventListener("keydown", (e) => {
        if (e.key === "Enter") confirmarCampo(clave);
      });
      campos[clave].entrada.addEventListener("focus", (e) => e.target.select());
    }

    // The canvas fits the available area, without distorting the image
    function ajustarLienzo() {
      const estilo = getComputedStyle(escena);
      const ancho = escena.clientWidth - parseFloat(estilo.paddingLeft) - parseFloat(estilo.paddingRight) - 24;
      const alto = escena.clientHeight - parseFloat(estilo.paddingTop) - parseFloat(estilo.paddingBottom) - 24;
      if (ancho <= 0 || alto <= 0) return;
      const escala = Math.min(ancho / img.w, alto / img.h, 3);
      lienzo.style.width = `${Math.max(1, Math.round(img.w * escala))}px`;
      lienzo.style.height = `${Math.max(1, Math.round(img.h * escala))}px`;
    }
    ajustarLienzo();
    if (typeof ResizeObserver !== "undefined") {
      const observador = new ResizeObserver(ajustarLienzo);
      observador.observe(escena);
      ctx.limpiezas.push(() => observador.disconnect());
    }

    ctx.obtenerParams = () => (recuadroValido(estado.r, img) && !esImagenCompleta(estado.r, img) ? { ...estado.r } : null);
    ctx.depurar = { estado, img, cambiar, dibujar };
    dibujar();
    caja.focus();
  }

  // ---------- Time mode (audio and video) ----------

  // Track with the two draggable ends. Returns the node and the function that places them
  function crearPista(sel, dur, alMover) {
    const zona = el("div", "ed-pistazona");
    const pista = el("div", "ed-pista");
    const base = el("div", "ed-base");
    base.style.cssText = "position:absolute;inset:0";
    const atenuadoIni = el("div", "ed-atenuado ed-ini");
    const atenuadoFin = el("div", "ed-atenuado ed-fin");
    const tramo = el("div", "ed-tramo");
    const extremos = {};
    const nombres = { start: "editor.time.startAria", end: "editor.time.endAria" };
    for (const cual of ["start", "end"]) {
      const extremo = el("div", "ed-extremo");
      extremo.dataset.cual = cual;
      extremo.tabIndex = 0;
      extremo.setAttribute("role", "slider");
      extremo.setAttribute("aria-label", tr(nombres[cual]));
      extremos[cual] = extremo;
    }
    pista.append(base, atenuadoIni, atenuadoFin, tramo, extremos.start, extremos.end);
    zona.append(pista);

    function colocar() {
      const ini = porcentaje(sel.start, dur);
      const fin = porcentaje(sel.end, dur);
      extremos.start.style.left = `${ini}%`;
      extremos.end.style.left = `${fin}%`;
      atenuadoIni.style.width = `${ini}%`;
      atenuadoFin.style.width = `${100 - fin}%`;
      tramo.style.left = `${ini}%`;
      tramo.style.width = `${fin - ini}%`;
      const hueco = Math.min(MIN_HUECO, dur);
      const datos = {
        start: [0, sel.end - hueco, sel.start],
        end: [sel.start + hueco, dur, sel.end],
      };
      for (const cual in datos) {
        const [min, max, valor] = datos[cual];
        extremos[cual].setAttribute("aria-valuemin", String(redondear2(min)));
        extremos[cual].setAttribute("aria-valuemax", String(redondear2(max)));
        extremos[cual].setAttribute("aria-valuenow", String(redondear2(valor)));
        extremos[cual].setAttribute("aria-valuetext", formatearTiempo(valor));
      }
    }

    // Mouse and touch: over an end it drags it; on any other point, it moves the nearest one
    pista.addEventListener("pointerdown", (e) => {
      if (e.button !== undefined && e.button > 0) return;
      e.preventDefault();
      const area = pista.getBoundingClientRect();
      const tiempoEn = (clientX) => tiempoDesdeX(clientX - area.left, area.width, dur);
      const tocado = e.target.closest ? e.target.closest(".ed-extremo") : null;
      let cual;
      if (tocado) cual = tocado.dataset.cual;
      else {
        const t = tiempoEn(e.clientX);
        cual = Math.abs(t - sel.start) <= Math.abs(t - sel.end) && t <= sel.end ? "start" : "end";
      }
      capturar(pista, e);
      extremos[cual].focus();
      alMover(cual, tiempoEn(e.clientX));
      const mover = (ev) => {
        if (ev.pointerId === e.pointerId) alMover(cual, tiempoEn(ev.clientX));
      };
      const soltar = (ev) => {
        if (ev.pointerId !== e.pointerId) return;
        pista.removeEventListener("pointermove", mover);
        pista.removeEventListener("pointerup", soltar);
        pista.removeEventListener("pointercancel", soltar);
      };
      pista.addEventListener("pointermove", mover);
      pista.addEventListener("pointerup", soltar);
      pista.addEventListener("pointercancel", soltar);
    });

    // Keyboard: arrows 0.1 s (1 s with Shift), Home and End jump to the limit
    for (const cual in extremos) {
      extremos[cual].addEventListener("keydown", (e) => {
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
        const paso = e.shiftKey ? 1 : 0.1;
        const actual = sel[cual];
        let nuevo = null;
        if (e.key === "ArrowLeft" || e.key === "ArrowDown") nuevo = actual - paso;
        else if (e.key === "ArrowRight" || e.key === "ArrowUp") nuevo = actual + paso;
        else if (e.key === "Home") nuevo = 0;
        else if (e.key === "End") nuevo = dur;
        if (nuevo === null) return;
        e.preventDefault();
        e.stopPropagation();
        alMover(cual, nuevo);
      });
    }

    return { zona, pista, base, colocar };
  }

  function crearCampoTiempo(texto) {
    const grupo = el("label", "ed-grupo");
    const entrada = el("input", "ed-campo ed-tiempo");
    entrada.type = "text";
    entrada.autocomplete = "off";
    entrada.setAttribute("aria-label", tr("editor.time.fieldAria", { name: texto }));
    grupo.append(el("span", "", texto), entrada);
    return { grupo, entrada };
  }

  function dibujarOnda(canvas, picos, sel, dur, colores) {
    const dpr = window.devicePixelRatio || 1;
    const ancho = canvas.clientWidth;
    const alto = canvas.clientHeight;
    if (ancho === 0 || alto === 0) return;
    if (canvas.width !== Math.round(ancho * dpr) || canvas.height !== Math.round(alto * dpr)) {
      canvas.width = Math.round(ancho * dpr);
      canvas.height = Math.round(alto * dpr);
    }
    const g = canvas.getContext("2d");
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, ancho, alto);
    const n = picos.length;
    const ancho1 = ancho / n;
    for (let i = 0; i < n; i++) {
      const t = ((i + 0.5) / n) * dur;
      g.fillStyle = t >= sel.start && t <= sel.end ? colores.dentro : colores.fuera;
      const barra = Math.max(3, limitar(picos[i], 0, 1) * alto * 0.88);
      const x = i * ancho1 + ancho1 * 0.15;
      g.beginPath();
      g.roundRect(x, (alto - barra) / 2, Math.max(1, ancho1 * 0.7), barra, 1.5);
      g.fill();
    }
  }

  // Loads the frames of the strip, with at most PARALELO requests at a time
  function cargarTira(ctx, celdas, tiempos) {
    let siguiente = 0;
    let activas = 0;
    function lanzar() {
      while (activas < PARALELO && siguiente < celdas.length && !ctx.cerrado) {
        const i = siguiente++;
        activas++;
        Promise.resolve()
          .then(() => ctx.api.video_frame(ctx.ruta, tiempos[i], 160))
          .then((url) => {
            if (ctx.cerrado) return;
            const foto = el("img");
            foto.alt = "";
            foto.src = url;
            celdas[i].append(foto);
            celdas[i].classList.remove("ed-cargando");
          })
          .catch(() => {
            celdas[i].classList.remove("ed-cargando");
            celdas[i].classList.add("ed-fallo");
          })
          .finally(() => {
            activas--;
            lanzar();
          });
      }
    }
    lanzar();
  }

  async function iniciarTiempo(ctx, tipo) {
    const esVideo = tipo === "video";
    mostrarCarga(ctx, esVideo ? tr("editor.video.loading") : tr("editor.audio.loading"));
    let info;
    try {
      info = await ctx.api.media_info(ctx.ruta);
    } catch (e) {
      if (!ctx.cerrado) mostrarError(ctx, esVideo ? tr("editor.video.readError") : tr("editor.audio.readError"), e);
      return;
    }
    if (ctx.cerrado) return;
    const dur = info && Number(info.duration);
    if (!dur || !Number.isFinite(dur) || dur <= 0) {
      mostrarError(ctx, tr("editor.readError"), tr("editor.time.noDuration"));
      return;
    }

    const sel = { start: 0, end: dur };
    let ultimo = "start";
    let dibujarOndaActual = () => {};

    const pista = crearPista(sel, dur, (cual, t) => {
      const nueva = moverExtremo(sel, cual, t, dur);
      sel.start = nueva.start;
      sel.end = nueva.end;
      ultimo = cual;
      dibujar();
      if (esVideo) pedirVista(sel[cual]);
    });

    // Time fields
    const panel = el("div", "ed-panel");
    const campoIni = crearCampoTiempo(tr("editor.time.start"));
    const campoFin = crearCampoTiempo(tr("editor.time.end"));
    const duracion = el("div", "ed-grupo ed-medida");
    const duracionTexto = el("strong");
    duracion.append(el("span", "", tr("editor.time.duration")), duracionTexto);
    const restablecer = el("button", "ed-boton ed-chico", tr("editor.reset"));
    restablecer.type = "button";
    restablecer.addEventListener("click", () => {
      sel.start = 0;
      sel.end = dur;
      dibujar();
      if (esVideo) pedirVista(sel[ultimo]);
    });
    panel.append(campoIni.grupo, campoFin.grupo, restablecer, duracion);

    const regla = el("div", "ed-regla");
    regla.append(el("span", "", "0:00.00"), el("span", "", formatearTiempo(dur / 2)), el("span", "", formatearTiempo(dur)));
    pista.zona.append(regla);

    // Large preview (video)
    let vista = null;
    let vistaImg = null;
    let vistaSello = null;
    let vistaMensaje = null;
    let contador = 0;
    let temporizador = null;
    if (esVideo) {
      vista = el("div", "ed-vista");
      vistaImg = el("img");
      vistaImg.alt = tr("editor.video.frameAlt");
      vistaImg.hidden = true;
      vistaMensaje = el("div", "ed-sinvista", tr("editor.video.frameLoading"));
      vistaSello = el("div", "ed-sello");
      vista.append(vistaImg, vistaMensaje, vistaSello);
      ctx.limpiezas.push(() => clearTimeout(temporizador));
    }

    function pedirVista(t) {
      vistaSello.textContent = formatearTiempo(t);
      vista.classList.add("ed-espera");
      clearTimeout(temporizador);
      temporizador = setTimeout(() => {
        const id = ++contador;
        const instante = limitar(t, 0, Math.max(0, dur - MIN_HUECO));
        Promise.resolve()
          .then(() => ctx.api.video_frame(ctx.ruta, instante, 560))
          .then((url) => {
            if (ctx.cerrado || id !== contador) return;
            vistaImg.src = url;
            vistaImg.hidden = false;
            vistaMensaje.hidden = true;
            vista.classList.remove("ed-espera");
          })
          .catch((e) => {
            if (ctx.cerrado || id !== contador) return;
            vistaImg.hidden = true;
            vistaMensaje.hidden = false;
            vistaMensaje.textContent = e ? tr("editor.video.frameFailedDetail", { detail: mensajeDe(e) }) : tr("editor.video.frameFailed");
            vista.classList.remove("ed-espera");
          });
      }, 150);
    }

    function dibujar() {
      pista.colocar();
      campoIni.entrada.value = formatearTiempo(sel.start);
      campoFin.entrada.value = formatearTiempo(sel.end);
      duracionTexto.textContent = formatearTiempo(sel.end - sel.start);
      dibujarOndaActual();
      if (!seleccionValida(sel, dur)) ctx.estadoAplicar(false, tr("editor.time.invalid"), true);
      else if (esTodoElTiempo(sel, dur)) ctx.estadoAplicar(false, tr("editor.nothingToTrim"));
      else ctx.estadoAplicar(true, "");
    }

    function confirmarCampo(cual, entrada) {
      const t = parsearTiempo(entrada.value);
      if (t === null) {
        entrada.value = formatearTiempo(sel[cual]);
        ctx.estadoAplicar(!esTodoElTiempo(sel, dur), tr("editor.time.format"), true);
        return;
      }
      const nueva = moverExtremo(sel, cual, t, dur);
      sel.start = nueva.start;
      sel.end = nueva.end;
      ultimo = cual;
      dibujar();
      if (esVideo) pedirVista(sel[cual]);
    }
    for (const [cual, campo] of [["start", campoIni], ["end", campoFin]]) {
      campo.entrada.addEventListener("change", () => confirmarCampo(cual, campo.entrada));
      campo.entrada.addEventListener("keydown", (e) => {
        if (e.key === "Enter") confirmarCampo(cual, campo.entrada);
      });
      campo.entrada.addEventListener("focus", (e) => e.target.select());
    }

    // Background of the track
    if (esVideo) {
      pista.pista.classList.add("ed-tira");
      const tira = el("div", "ed-fotogramas");
      const celdas = [];
      const tiempos = [];
      for (let i = 0; i < FOTOGRAMAS; i++) {
        const celda = el("div", "ed-fotograma ed-cargando");
        celdas.push(celda);
        tiempos.push(Math.min(dur * ((i + 0.5) / FOTOGRAMAS), Math.max(0, dur - MIN_HUECO)));
        tira.append(celda);
      }
      pista.base.append(tira);
      ctx.cuerpo.replaceChildren(vista, pista.zona, panel);
      cargarTira(ctx, celdas, tiempos);
      pedirVista(0);
    } else {
      pista.pista.classList.add("ed-alta", "ed-sobreonda");
      const esqueleto = el("div", "ed-esqueleto");
      esqueleto.setAttribute("aria-label", tr("editor.audio.waveform"));
      pista.base.append(esqueleto);
      ctx.cuerpo.replaceChildren(pista.zona, panel);
      cargarOnda(ctx, pista, sel, dur, (redibujar) => {
        dibujarOndaActual = redibujar;
        dibujar();
      });
    }

    ctx.obtenerParams = () => (seleccionValida(sel, dur) && !esTodoElTiempo(sel, dur) ? { start: sel.start, end: Math.min(sel.end, dur) } : null);
    ctx.depurar = { sel, dur, dibujar };
    dibujar();
    pista.zona.querySelector(".ed-extremo").focus();
  }

  // Requests the waveform and draws it; if it fails, it leaves a simple time bar
  function cargarOnda(ctx, pista, sel, dur, alListo) {
    Promise.resolve()
      .then(() => ctx.api.waveform(ctx.ruta, 300))
      .then((picos) => {
        if (ctx.cerrado) return;
        if (!Array.isArray(picos) || picos.length === 0) throw new Error("sin datos");
        const canvas = el("canvas");
        canvas.setAttribute("aria-hidden", "true");
        pista.base.replaceChildren(canvas);
        const estilo = getComputedStyle(ctx.raiz);
        const colores = {
          dentro: estilo.getPropertyValue("--ed-naranja").trim() || "#f26a1b",
          fuera: estilo.getPropertyValue("--ed-onda-fuera").trim() || "rgba(58,42,29,0.28)",
        };
        const redibujar = () => dibujarOnda(canvas, picos, sel, dur, colores);
        if (typeof ResizeObserver !== "undefined") {
          const observador = new ResizeObserver(redibujar);
          observador.observe(canvas);
          ctx.limpiezas.push(() => observador.disconnect());
        }
        alListo(redibujar);
      })
      .catch(() => {
        if (ctx.cerrado) return;
        pista.base.replaceChildren(el("div", "ed-linea"));
        pista.pista.classList.remove("ed-sobreonda");
      });
  }

  // ---------- Entrada ----------

  function abrir(op) {
    return new Promise((resolver) => {
      const ctx = crearMarco(op, resolver);
      if (op.tipo === "imagen") iniciarImagen(ctx);
      else if (op.tipo === "audio" || op.tipo === "video") iniciarTiempo(ctx, op.tipo);
      else mostrarError(ctx, tr("editor.invalidType"), op.tipo);
      window.Editor.ultimo = ctx; // only for tests
    });
  }

  const puro = {
    limitar, formatearTiempo, parsearTiempo, moverExtremo, tiempoDesdeX, porcentaje, seleccionValida, esTodoElTiempo,
    recuadroCompleto, esImagenCompleta, recuadroValido, pantallaAPixeles, moverRecuadro, redimensionarRecuadro,
    ajustarProporcion, aplicarCampo, extensionDe, sinExtension, nombreInicial, validarNombre,
  };

  if (typeof window !== "undefined") window.Editor = { abrir };
  if (typeof module !== "undefined") module.exports = puro;
})();

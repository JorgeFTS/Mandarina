// Pantalla de ajustes: qué formatos muestra la rueda de cada tipo de archivo.
// Uso: window.Ajustes.abrir({ contenedor, api }) -> Promise<boolean> (true si cambió algo)
// api: get_config, set_formats, reset_formats, get_prefs, set_language, set_accent. Cada cambio se guarda
// y se aplica al momento; alCambiarIdioma (opcional) la pone main.js para repintar la página al cambiar de idioma.
// Va dentro de una función para no chocar con los nombres de main.js.
(function () {
  const tr = (clave, variables) => window.I18n.t(clave, variables);

  function el(etiqueta, clase, texto) {
    const e = document.createElement(etiqueta);
    if (clase) e.className = clase;
    if (texto !== undefined) e.textContent = texto;
    return e;
  }

  // Lista nueva tras marcar o desmarcar un formato: se respeta el orden de los que ya estaban
  // y el nuevo va al final
  function conCambio(activos, formato, marcado) {
    if (!marcado) return activos.filter((f) => f !== formato);
    return activos.includes(formato) ? activos : [...activos, formato];
  }

  // ¿Se puede cambiar este formato sin salirse de los límites? Con más de `max` de serie
  // (audio, vídeo y documento traen 8) siempre se puede quitar uno
  function puedeCambiar(activos, formato, min, max) {
    return activos.includes(formato) ? activos.length > min : activos.length < max;
  }

  // Colores de acento: id (el mismo que valida el backend) -> tono normal y tono claro (hover)
  const ACENTOS = {
    orange: ["#f26a1b", "#ff8a3d"],
    red: ["#e5383b", "#ff6b6b"],
    pink: ["#e0457b", "#ff7aa8"],
    purple: ["#8e5bd6", "#b08af0"],
    blue: ["#2f7de1", "#5fa0f5"],
    green: ["#2e9e5b", "#55c47f"],
  };

  // Pinta toda la app con ese color (las hojas de estilo usan --naranja y --naranja-claro)
  function aplicarAcento(id) {
    const [normal, claro] = ACENTOS[id] || ACENTOS.orange;
    const raiz = document.documentElement.style;
    raiz.setProperty("--naranja", normal);
    raiz.setProperty("--naranja-claro", claro);
  }

  function abrir({ contenedor, api, alCambiarIdioma }) {
    return new Promise((resolver) => {
      let cambio = false;
      let tipos = [];
      let prefs = null;

      const raiz = el("div", "ed-raiz aj-raiz");
      raiz.tabIndex = -1;
      raiz.setAttribute("role", "dialog");
      raiz.setAttribute("aria-label", tr("settings.title"));
      const cabecera = el("div", "ed-cabecera");
      const titulo = el("h2", "ed-titulo", tr("settings.title"));
      cabecera.append(titulo);
      const lista = el("div", "aj-lista");
      const aviso = el("p", "ed-aviso");
      aviso.setAttribute("role", "status");
      const cerrarBoton = el("button", "ed-boton ed-primario", tr("settings.close"));
      cerrarBoton.type = "button";
      const pie = el("div", "ed-botones");
      pie.append(aviso, cerrarBoton);
      raiz.append(cabecera, lista, pie);
      contenedor.replaceChildren(raiz);

      function cerrar() {
        document.removeEventListener("keydown", alPulsarTecla, true);
        resolver(cambio);
      }
      function alPulsarTecla(e) {
        if (e.key !== "Escape") return;
        e.preventDefault();
        e.stopPropagation();
        cerrar();
      }
      cerrarBoton.addEventListener("click", cerrar);
      document.addEventListener("keydown", alPulsarTecla, true);

      function mostrarError(e) {
        aviso.textContent = tr("settings.error", { error: String(e) });
        aviso.classList.add("ed-aviso-error");
      }

      async function guardar(tipo, activos) {
        try {
          await api.set_formats(tipo.key, activos);
          tipo.enabled = activos;
          cambio = true;
          aviso.textContent = "";
          aviso.classList.remove("ed-aviso-error");
        } catch (e) {
          mostrarError(e);
        }
        dibujar();
      }

      async function restaurar(tipo) {
        try {
          await api.reset_formats(tipo.key);
          tipo.enabled = tipo.all.slice();
          cambio = true;
          aviso.textContent = "";
          aviso.classList.remove("ed-aviso-error");
        } catch (e) {
          mostrarError(e);
        }
        dibujar();
      }

      function dibujarTipo(tipo) {
        const seccion = el("section", "aj-tipo");
        const nombre = tr("settings.kind." + tipo.key);
        const cuenta = tipo.enabled.length;
        const cab = el("div", "aj-cab");
        cab.append(el("h3", "aj-nombre", nombre), el("span", "aj-cuenta", tr("settings.count", { count: cuenta, min: tipo.min, max: tipo.max })));
        const porDefecto = tipo.all.length === cuenta && tipo.all.every((f) => tipo.enabled.includes(f));
        const restaurarBoton = el("button", "aj-restaurar", tr("settings.restore"));
        restaurarBoton.type = "button";
        restaurarBoton.disabled = porDefecto;
        restaurarBoton.setAttribute("aria-label", tr("settings.restoreAria", { name: nombre }));
        restaurarBoton.addEventListener("click", () => restaurar(tipo));
        cab.append(restaurarBoton);

        const fichas = el("div", "aj-fichas");
        fichas.setAttribute("role", "group");
        fichas.setAttribute("aria-label", nombre);
        for (const formato of tipo.all) {
          const marcado = tipo.enabled.includes(formato);
          const ficha = el("label", "aj-ficha" + (marcado ? " aj-activa" : ""));
          const caja = document.createElement("input");
          caja.type = "checkbox";
          caja.checked = marcado;
          caja.disabled = !puedeCambiar(tipo.enabled, formato, tipo.min, tipo.max);
          caja.addEventListener("change", () => guardar(tipo, conCambio(tipo.enabled, formato, caja.checked)));
          ficha.append(caja, el("span", "", formato.toUpperCase()));
          fichas.append(ficha);
        }
        seccion.append(cab, fichas);
        return seccion;
      }

      function limpiarAviso() {
        aviso.textContent = "";
        aviso.classList.remove("ed-aviso-error");
      }

      // Los textos fijos de esta pantalla (el resto se vuelve a crear en dibujar)
      function pintarTextosFijos() {
        titulo.textContent = tr("settings.title");
        cerrarBoton.textContent = tr("settings.close");
        raiz.setAttribute("aria-label", tr("settings.title"));
      }

      // Idioma: se guarda y se aplica al momento; alCambiarIdioma repinta la página y esta pantalla
      // se vuelve a dibujar ya en el idioma nuevo
      async function elegirIdioma(valor) {
        try {
          await api.set_language(valor);
          prefs.language = valor;
          if (alCambiarIdioma) await alCambiarIdioma();
          cambio = true;
          limpiarAviso();
          pintarTextosFijos();
        } catch (e) {
          mostrarError(e);
        }
        dibujar();
      }

      async function elegirAcento(id) {
        try {
          await api.set_accent(id);
          prefs.accent = id;
          aplicarAcento(id);
          limpiarAviso();
        } catch (e) {
          mostrarError(e);
        }
        dibujar();
      }

      function dibujarGeneral() {
        const idioma = el("section", "aj-tipo");
        idioma.append(el("h3", "aj-nombre", tr("settings.language")));
        const opciones = el("div", "aj-fichas");
        opciones.setAttribute("role", "radiogroup");
        opciones.setAttribute("aria-label", tr("settings.language"));
        // "Automático" y un botón por idioma (el nombre de cada uno, en su propio idioma, lo da el backend)
        const idiomas = [{ code: "auto", name: tr("settings.language.auto") }, ...prefs.languages];
        for (const { code: valor, name: nombreIdioma } of idiomas) {
          const ficha = el("label", "aj-ficha" + (prefs.language === valor ? " aj-activa" : ""));
          const radio = document.createElement("input");
          radio.type = "radio";
          radio.name = "aj-idioma";
          radio.checked = prefs.language === valor;
          radio.addEventListener("change", () => elegirIdioma(valor));
          ficha.append(radio, el("span", "", nombreIdioma));
          opciones.append(ficha);
        }
        idioma.append(opciones);

        const color = el("section", "aj-tipo");
        color.append(el("h3", "aj-nombre", tr("settings.accent")));
        const muestras = el("div", "aj-fichas");
        muestras.setAttribute("role", "radiogroup");
        muestras.setAttribute("aria-label", tr("settings.accent"));
        for (const id of prefs.accents) {
          const nombre = tr("settings.accent." + id);
          const muestra = el("label", "aj-color" + (prefs.accent === id ? " aj-activa" : ""));
          muestra.title = nombre;
          muestra.style.setProperty("--muestra", (ACENTOS[id] || ACENTOS.orange)[0]);
          const radio = document.createElement("input");
          radio.type = "radio";
          radio.name = "aj-acento";
          radio.checked = prefs.accent === id;
          radio.setAttribute("aria-label", nombre);
          radio.addEventListener("change", () => elegirAcento(id));
          muestra.append(radio);
          muestras.append(muestra);
        }
        color.append(muestras);

        const formatos = el("div", "aj-grupo");
        formatos.append(el("h3", "aj-grupo-titulo", tr("settings.formats")), el("div", "ed-subtitulo", tr("settings.hint", { min: tipos[0].min, max: tipos[0].max })));
        return [idioma, color, formatos];
      }

      function dibujar() {
        lista.replaceChildren(...dibujarGeneral(), ...tipos.map(dibujarTipo));
      }

      Promise.all([api.get_config(), api.get_prefs()])
        .then(([datos, preferencias]) => {
          tipos = datos;
          prefs = preferencias;
          dibujar();
          cerrarBoton.focus();
        })
        .catch((e) => {
          mostrarError(e);
        });
    });
  }

  const puro = { conCambio, puedeCambiar };
  if (typeof window !== "undefined") window.Ajustes = { abrir, aplicarAcento };
  if (typeof module !== "undefined") module.exports = puro;
})();

// Scene of the demo video: a pure function of time (?t=ms). It uses the real interface of the app (main.js in demo mode).
(async function () {
  const T = +(new URLSearchParams(location.search).get("t") || 0);
  const $ = (id) => document.getElementById(id);
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const seg = (t, a, b) => clamp((t - a) / (b - a));
  const inOut = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
  const out = (x) => 1 - Math.pow(1 - x, 3);
  const back = (x) => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); };
  const lerp = (a, b, k) => a + (b - a) * k;
  const until = async (f) => { for (let i = 0; i < 400 && !f(); i++) await new Promise((r) => setTimeout(r, 25)); };
  const fade = (t, a, b, c, d) => (t < a || t > d ? 0 : t < b ? seg(t, a, b) : t < c ? 1 : 1 - seg(t, c, d)); // fades in a-b, fades out c-d

  // ---------- Nautilus window ----------
  const FILES = [["photo.jpg", 0, 0, "#ff9f43"], ["song.mp3", 1, 0, "#8e5bd6"], ["clip.mp4", 2, 0, "#2f7de1"], ["report.docx", 0, 1, "#3b6fd4"], ["data.csv", 1, 1, "#2e9e5b"], ["notes", 2, 1, "#e5a50a"]];
  const GX = 370, GY = 190, DX = 190, DY = 170;
  const fileXY = (i) => [GX + FILES[i][1] * DX + 70, GY + FILES[i][2] * DY + 55];
  const nautilus = $("nautilus");
  const icono = (i) => {
    const c = FILES[i][3];
    if (i === 5) return `<svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" fill="${c}"/></svg>`;
    const glifo = i === 0 ? '<circle cx="9" cy="10" r="1.6" fill="#fff"/><path d="M5 17l4-4 3 3 3-2 4 3" stroke="#fff" fill="none" stroke-width="1.6"/>' : i === 1 ? '<path d="M10 17V8l6-1v8" stroke="#fff" fill="none" stroke-width="1.7"/><circle cx="8.5" cy="17" r="1.8" fill="#fff"/><circle cx="14.5" cy="15" r="1.8" fill="#fff"/>' : i === 2 ? '<path d="M9 8l7 4-7 4z" fill="#fff"/>' : i === 3 ? '<path d="M8 9h8M8 12h8M8 15h5" stroke="#fff" stroke-width="1.6"/>' : '<path d="M6 9h12M6 13h12M6 17h12M10 7v12" stroke="#fff" stroke-width="1.4"/>';
    return `<svg viewBox="0 0 24 24"><rect x="3.5" y="2" width="17" height="20" rx="3" fill="${c}"/>${glifo}</svg>`;
  };
  FILES.forEach((f, i) => {
    const d = document.createElement("div");
    d.className = "archivo"; d.id = "f" + i;
    d.style.left = GX - 190 + f[1] * DX + "px"; d.style.top = GY - 110 + f[2] * DY + "px";
    d.innerHTML = icono(i) + `<span>${f[0]}</span>`;
    nautilus.append(d);
  });

  // ---------- Timeline ----------
  const P1 = fileXY(0), P2 = fileXY(2);
  const segPos = (P, i, n) => { const [x, y] = punto((i + 0.5) * (360 / n), (R_EXT + R_INT) / 2); return [P[0] + x - 230, P[1] + y - 230]; };
  const wheelA = [6300, 10200], cardA = [10200, 14400], wheelB = [16500, 20900], ajustes = [21000, 25600];
  const clicks = [4400, 9500, 15400, 17700, 21900, 22800, 23700];
  const nivelB = T >= 17700 ? "herramientas" : "conversion";
  const seleccion = T >= 15400 ? 2 : T >= 4400 ? 0 : -1;
  const accent = T < 22000 ? "orange" : T < 22900 ? "blue" : "green";
  const lang = T < 23800 ? "en" : "fr";

  // ---------- Scene layers ----------
  $("titulo").style.opacity = fade(T, 0, 700, 2100, 2800);
  $("titulo").style.transform = `scale(${lerp(0.92, 1, out(seg(T, 0, 900)))})`;
  $("fin").style.opacity = fade(T, 25500, 26200, 99999, 100000);
  $("fin").style.transform = `scale(${lerp(0.94, 1, out(seg(T, 25500, 26400)))})`;
  const nOp = Math.min(seg(T, 2400, 3100), T >= cardA[0] && T < 14400 ? lerp(1, 0.4, out(seg(T, 10200, 10600))) : 1) * (T > 20700 ? 1 - seg(T, 20700, 21200) : 1);
  const nVuelve = T >= 14400 && T < 20700 ? lerp(0.4, 1, seg(T, 14400, 14900)) : null;
  nautilus.style.opacity = T < 2400 ? 0 : T < 10200 ? seg(T, 2400, 3100) : T < 14400 ? lerp(1, 0.4, out(seg(T, 10200, 10600))) : T < 20700 ? lerp(0.4, 1, seg(T, 14400, 14900)) : 1 - seg(T, 20700, 21200);
  nautilus.style.transform = `translateY(${lerp(34, 0, out(seg(T, 2400, 3300)))}px)`;
  FILES.forEach((f, i) => $("f" + i).classList.toggle("sel", i === seleccion));

  const teclas = $("teclas");
  const kA = fade(T, 5100, 5400, 6300, 6600), kB = fade(T, 15900, 16200, 16400, 16700);
  teclas.style.opacity = Math.max(kA, kB);
  const press = (a) => (T >= a && T < a + 220 ? 1 : 0);
  teclas.querySelectorAll(".tecla").forEach((k) => { const p = press(5800) || press(16200); k.style.transform = p ? "translateY(6px)" : ""; k.style.background = p ? "#ff8a3d" : "#fff"; k.style.color = p ? "#fff" : "#3a2a1d"; });

  const leyendas = [[3200, 5300, "Select a file in Nautilus"], [5300, 6800, "Press Alt+M"], [6900, 9800, "Pick a format"], [12300, 14400, "Done — saved next to the original"], [16300, 17700, "Tools for every file"], [17800, 20800, "Reduce size, trim, rotate, zip…"], [21400, 25200, "Your language, your colors"]];
  const L = leyendas.find(([a, b]) => T >= a - 250 && T <= b + 250);
  $("leyenda").style.opacity = L ? Math.min(seg(T, L[0] - 250, L[0] + 100), 1 - seg(T, L[1] - 100, L[1] + 250)) : 0;
  if (L) $("leyenda").textContent = L[2];

  // ---------- Cursor ----------
  const wp = [[2900, 1150, 735], [4300, P1[0] + 6, P1[1] + 10], [6800, P1[0] + 6, P1[1] + 10],
    [7300, ...segPos(P1, 0, 7)], [7900, ...segPos(P1, 1, 7)], [8500, ...segPos(P1, 2, 7)], [9100, ...segPos(P1, 1, 7)], [14400, ...segPos(P1, 1, 7)],
    [15300, P2[0] + 6, P2[1] + 10], [16800, P2[0] + 6, P2[1] + 10], [17100, ...segPos(P2, 6, 7)], [17700, ...segPos(P2, 6, 7)],
    [18300, ...segPos(P2, 0, 8)], [19000, ...segPos(P2, 2, 8)], [19700, ...segPos(P2, 7, 8)], [20900, ...segPos(P2, 7, 8)]];
  const extra = () => { if (rects.blue) wp.push([21500, 640, 400], [21900, ...rects.blue], [22800, ...rects.green], [23700, ...rects.fr], [25200, ...rects.fr]); };
  let cx = wp[0][1], cy = wp[0][2];
  const mover = () => { cx = wp[0][1]; cy = wp[0][2]; for (let i = 0; i < wp.length - 1; i++) {
    const [t0, x0, y0] = wp[i], [t1, x1, y1] = wp[i + 1];
    if (T >= t0 && T <= t1) { const k = inOut(seg(T, t0, t1)); cx = lerp(x0, x1, k); cy = lerp(y0, y1, k); break; }
    if (T > t1) { cx = x1; cy = y1; }
  } };
  mover();
  const hover = (P, n) => (T < 0 ? -1 : segmentoEn(cx - P[0], cy - P[1], n));

  // ---------- The real interface of the app ----------
  const esc = $("escenario");
  esc.style.opacity = 0;
  await until(() => typeof vistaActual !== "undefined" && vistaActual === "inicio" && window.I18n);
  window.salir = () => {};
  api.get_targets_many = async (paths) => (String(paths[0]).includes("photo") ? ["png", "webp", "tiff", "avif", "bmp", "pdf"] : ["mov", "mkv", "webm", "avi", "gif", "mp3"]);
  const place = (x, y, scale, opacity) => { esc.style.setProperty("--x", x + "px"); esc.style.setProperty("--y", y + "px"); esc.style.scale = scale; esc.style.opacity = opacity; };
  let rects = {};

  if (T >= wheelA[0] && T < wheelA[1]) {
    await cargarArchivos(["/demo/photo.jpg"]);
    resaltar(T < 7000 ? -1 : hover(P1, 7));
    const k = T < 6900 ? seg(T, wheelA[0], 6900) : 1, sal = seg(T, 9800, 10200);
    place(P1[0], P1[1], lerp(0.55, 1, back(k)) * lerp(1, 0.88, sal), clamp(k * 3) * (1 - sal));
  } else if (T >= cardA[0] && T < cardA[1]) {
    await cargarArchivos(["/demo/photo.jpg"]);
    const ent = seg(T, 10200, 10550);
    if (T < 12300) {
      const { relleno } = mostrarProgreso("Converting to WEBP...");
      relleno.style.transition = "none"; relleno.style.width = inOut(seg(T, 10450, 12100)) * 100 + "%";
      place(640, 360, lerp(0.88, 1, out(ent)), out(ent));
    } else {
      mostrarListo("photo.webp");
      const ent2 = seg(T, 12300, 12650);
      place(640, 360, lerp(0.92, 1, back(ent2)), 1 - seg(T, 14100, 14400));
    }
  } else if (T >= wheelB[0] && T < wheelB[1]) {
    await cargarArchivos(["/demo/clip.mp4"]);
    if (nivelB === "herramientas") irANivel("herramientas");
    resaltar(T < 16900 ? -1 : hover(P2, nivelB === "conversion" ? 7 : 8));
    const k = seg(T, wheelB[0], 17000), sal = seg(T, 20500, 20900);
    place(P2[0], P2[1], lerp(0.55, 1, back(k)) * lerp(1, 0.88, sal) * (T >= 17700 && T < 17900 ? lerp(0.96, 1, seg(T, 17700, 17900)) : 1), clamp(k * 3) * (1 - sal));
  } else if (T >= ajustes[0] && T < ajustes[1]) {
    await cargarArchivos(["/demo/photo.jpg"]);
    PREFS_DEMO.language = lang; PREFS_DEMO.accent = accent;
    await cambiarIdioma();
    window.Ajustes.aplicarAcento(accent);
    abrirAjustes();
    await until(() => document.querySelector(".aj-lista .aj-tipo") && vistaActual === "ajustes");
    const panel = $("vista-ajustes"), k = seg(T, 21000, 21500), sal = seg(T, 25200, 25600);
    panel.style.opacity = clamp(k * 3) * (1 - sal); panel.style.transform = `translateY(-34px) scale(${lerp(0.9, 1, out(k)) * lerp(1, 0.94, sal) * 0.82})`;
    const sw = [...document.querySelectorAll(".aj-color")], rd = [...document.querySelectorAll("input[name=aj-idioma]")];
    const c = (el) => { const r = el.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; };
    rects = { blue: c(sw[4]), green: c(sw[5]), fr: c(rd[4].parentElement) };
  }
  document.documentElement.style.setProperty("--naranja", getComputedStyle(document.documentElement).getPropertyValue("--naranja"));

  extra(); mover();
  const cur = $("cursor");
  cur.style.opacity = T >= 2900 && T < 25400 ? Math.min(seg(T, 2900, 3300), 1 - seg(T, 25100, 25400)) : 0;
  cur.style.transform = `translate(${cx - 6}px,${cy - 3}px) scale(${clicks.some((c) => T >= c && T < c + 160) ? 0.86 : 1})`;
  const onda = $("onda"), c = clicks.find((c) => T >= c && T < c + 450);
  if (c !== undefined) { const k = (T - c) / 450; onda.style.left = cx + "px"; onda.style.top = cy + "px"; onda.style.opacity = 1 - k; onda.style.transform = `scale(${lerp(0.4, 1.8, out(k))})`; }


  document.title = "READY";
  window.__listo = true;
})();

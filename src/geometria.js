// Geometry of the wheel (coordinates of the 460x460 viewBox, center at 230,230)
const CENTRO = 230;
const R_EXT = 200;
const R_INT = 82;
const HUECO = 7; // gap between segments, in px
const ESQUINA = 9; // radius of the rounded corners, in px

// Returns the index of the segment for a point relative to the center, or -1
function segmentoEn(dx, dy, total) {
  const distancia = Math.hypot(dx, dy);
  if (total === 0 || distancia < R_INT || distancia > R_EXT) return -1;
  // the first segment starts at the top and goes clockwise
  let grados = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
  grados = (grados + 360) % 360;
  return Math.min(Math.floor(grados / (360 / total)), total - 1);
}

function punto(grados, radio) {
  const rad = ((grados - 90) * Math.PI) / 180;
  return [CENTRO + radio * Math.cos(rad), CENTRO + radio * Math.sin(rad)];
}

// angle (in degrees) taken up by an arc length at a given radius
function angulo(longitud, radio) {
  return (Math.asin(longitud / radio) * 180) / Math.PI;
}

// point at "dist" pixels from p towards q
function haciaPunto(p, q, dist) {
  const largo = Math.hypot(q[0] - p[0], q[1] - p[1]);
  const t = dist / largo;
  return [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
}

function par(p) {
  return `${p[0].toFixed(2)} ${p[1].toFixed(2)}`;
}

// Segment of the ring with a gap and rounded corners
function rutaSegmento(i, total) {
  const paso = 360 / total;
  const a0 = i * paso;
  const a1 = a0 + paso;
  const dE = angulo(HUECO / 2, R_EXT);
  const dI = angulo(HUECO / 2, R_INT);
  const gE = angulo(ESQUINA, R_EXT);
  const gI = angulo(ESQUINA, R_INT);

  const A = punto(a0 + dE, R_EXT);
  const B = punto(a1 - dE, R_EXT);
  const C = punto(a1 - dI, R_INT);
  const D = punto(a0 + dI, R_INT);

  const grandeExt = paso - 2 * (dE + gE) > 180 ? 1 : 0;
  const grandeInt = paso - 2 * (dI + gI) > 180 ? 1 : 0;

  return [
    `M${par(punto(a0 + dE + gE, R_EXT))}`,
    `A${R_EXT} ${R_EXT} 0 ${grandeExt} 1 ${par(punto(a1 - dE - gE, R_EXT))}`,
    `Q${par(B)} ${par(haciaPunto(B, C, ESQUINA))}`,
    `L${par(haciaPunto(C, B, ESQUINA))}`,
    `Q${par(C)} ${par(punto(a1 - dI - gI, R_INT))}`,
    `A${R_INT} ${R_INT} 0 ${grandeInt} 0 ${par(punto(a0 + dI + gI, R_INT))}`,
    `Q${par(D)} ${par(haciaPunto(D, A, ESQUINA))}`,
    `L${par(haciaPunto(A, D, ESQUINA))}`,
    `Q${par(A)} ${par(punto(a0 + dE + gE, R_EXT))}`,
    "Z",
  ].join(" ");
}

if (typeof module !== "undefined") module.exports = { segmentoEn, rutaSegmento, punto, R_EXT, R_INT };

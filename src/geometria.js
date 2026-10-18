// Geometría de la rueda (coordenadas del viewBox 460x460, centro en 230,230)
const CENTRO = 230;
const R_EXT = 200;
const R_INT = 82;
const HUECO = 7; // separación entre segmentos, en px
const ESQUINA = 9; // radio de las esquinas redondeadas, en px

// Devuelve el índice del segmento para un punto relativo al centro, o -1
function segmentoEn(dx, dy, total) {
  const distancia = Math.hypot(dx, dy);
  if (total === 0 || distancia < R_INT || distancia > R_EXT) return -1;
  // el primer segmento empieza arriba y avanza en sentido horario
  let grados = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
  grados = (grados + 360) % 360;
  return Math.min(Math.floor(grados / (360 / total)), total - 1);
}

function punto(grados, radio) {
  const rad = ((grados - 90) * Math.PI) / 180;
  return [CENTRO + radio * Math.cos(rad), CENTRO + radio * Math.sin(rad)];
}

// ángulo (en grados) que ocupa una longitud de arco a cierto radio
function angulo(longitud, radio) {
  return (Math.asin(longitud / radio) * 180) / Math.PI;
}

// punto a "dist" píxeles de p hacia q
function haciaPunto(p, q, dist) {
  const largo = Math.hypot(q[0] - p[0], q[1] - p[1]);
  const t = dist / largo;
  return [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
}

function par(p) {
  return `${p[0].toFixed(2)} ${p[1].toFixed(2)}`;
}

// Segmento del anillo con separación y esquinas redondeadas
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

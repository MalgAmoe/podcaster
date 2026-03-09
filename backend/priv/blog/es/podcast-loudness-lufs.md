%{
  title: "LUFS para podcasters: por qué tus episodios suenan diferente en cada plataforma",
  description: "Tu podcast suena genial en tu portátil pero bajo en Spotify y fuerte en Apple Podcasts. Este es el número que lo explica — y cómo arreglarlo.",
  date: ~D[2026-03-09]
}
---

Publicas un episodio. Suena bien en tu ordenador. Luego un oyente te escribe: "Oye, tu programa está muy bajo — tengo que subir el volumen al máximo." Otro oyente en otra app dice que suena bien. ¿Qué está pasando?

La respuesta es una unidad de medida llamada LUFS, y una vez que la entiendas, tu podcast sonará consistente en todas partes.

## ¿Qué es LUFS?

LUFS significa Loudness Units relative to Full Scale (Unidades de Loudness relativas a Escala Completa). Mide lo fuerte que el audio *suena* al oído humano — no solo lo alto que llegan los picos de la forma de onda.

Esta distinción importa. Un susurro y un grito pueden tener el mismo nivel de pico si el grito es breve y el susurro sostenido. Pero no suenan igual de fuertes. LUFS tiene esto en cuenta midiendo el loudness a lo largo del tiempo, ponderado para coincidir con la audición humana.

El nivel de pico te dice la ola más alta del océano. LUFS te dice lo agitado que está el mar realmente.

## Por qué los podcasters necesitan preocuparse

Cada plataforma de podcasts tiene un objetivo de loudness. Si tu episodio no coincide, la plataforma lo ajusta — a veces bien, a veces mal.

| Plataforma | Objetivo | Qué pasa si no lo cumples |
|----------|--------|--------------------------|
| **Spotify** | -14 LUFS | Normaliza a -14. Muy bajo = se sube (con ruido). Muy alto = se baja. |
| **Apple Podcasts** | -16 LUFS | Sound Check normaliza si el oyente lo tiene activado. Inconsistente. |
| **YouTube** | -14 LUFS | Normaliza hacia abajo pero no hacia arriba. Lo bajo se queda bajo. |
| **Overcast** | Voice Boost | Tiene su propio procesamiento. Aplica compresión y aumento de volumen sin importar qué. |

El problema: si masterizas a -20 LUFS (común en grabaciones sin procesar) y un oyente está en Spotify, tu episodio se sube — y todo el ruido de fondo sube con él.

## El objetivo de -16 LUFS

La mayoría de profesionales de podcast recomiendan **-16 LUFS**. Se ha convertido en el estándar ampliamente adoptado para voz hablada:

- En Spotify (objetivo -14), tu episodio se baja 2 dB — apenas perceptible.
- En Apple Podcasts (objetivo -16), coincide perfectamente.
- En YouTube (objetivo -14), se baja ligeramente — bien.
- Deja suficiente margen para que tu audio no suene aplastado o sobre-comprimido.

Esto importa más para voz que para música. La música ya está fuertemente comprimida y masterizada, así que llevarla a -14 LUFS está bien. La voz es más dinámica — tiene momentos naturales fuertes y suaves que quieres preservar. Llevar la voz a -14 LUFS requiere más compresión, lo que puede hacerla sonar plana y poco natural. -16 le da a la voz espacio para respirar mientras sigue sonando presente y profesional.

## True peak: el otro número que importa

LUFS mide el loudness promedio. True peak (dBTP) mide el nivel máximo absoluto, incluyendo picos que ocurren entre muestras.

Si tu true peak está en 0 dBTP (máximo), cuando la plataforma re-codifica tu archivo o el dispositivo del oyente reconstruye la señal, esos picos pueden exceder 0 dB y distorsionar. Lo escuchas como un breve crujido en consonantes fuertes.

**El límite seguro: -1 dBTP.** Configura el techo de tu limitador a -1.0 dBTP. Esto da suficiente margen para la conversión de códec y reproducción en cualquier dispositivo.

## Cómo medir LUFS

Necesitas un medidor de loudness:

- **Youlean Loudness Meter** (plugin gratuito) — Muestra LUFS integrado, a corto plazo, momentáneo, true peak y rango de loudness. La mejor opción gratuita.
- **ffmpeg** (gratuito, línea de comandos) — `ffmpeg -i tu_archivo.mp3 -af loudnorm=print_format=json -f null -` te da LUFS integrado y true peak.
- **La mayoría de DAWs** tienen medición de loudness integrada o soportan plugins de medición.

## Cómo alcanzar tu objetivo

### Enfoque manual

1. **Procesa tu audio primero** — reducción de ruido, compresión, EQ, todo lo demás antes del loudness.
2. **Añade un limitador** en la salida. Configura el techo a -1.0 dBTP.
3. **Normaliza a -16 LUFS.** La mayoría de DAWs tienen una función de normalización de loudness.
4. **Verifica con un medidor.** Reproduce el episodio completo y lee el valor de LUFS integrado. Debería estar dentro de 1 dB de tu objetivo.

### Automático

Herramientas como Auphonic, Munchy Cow y otras pueden manejar el loudness objetivo automáticamente. La ventaja de hacerlo como parte de una cadena completa de procesamiento (eliminación de ruido, compresión, EQ) en lugar de solo normalizar audio crudo es que no estás simplemente subiendo lo malo junto con lo bueno.

## Los errores más comunes

### Normalizar sin procesar primero

Si tu audio crudo está a -24 LUFS y simplemente lo normalizas a -16, estás subiendo todo 8 dB — incluyendo el ruido, la reverb y los clicks de boca. Procesa primero, normaliza al final.

### Sobre-comprimir para alcanzar el número

La compresión fuerte puede llevarte a -16 LUFS, pero suena plano y fatigante. Usa compresión suave (ratio de 2:1 a 4:1) y deja que el normalizador suba el nivel general naturalmente.

### Ignorar el true peak

Alcanzaste -16 LUFS pero tu true peak está en 0 dBTP. Los oyentes escuchan distorsión en ciertos dispositivos. Siempre usa un limitador de pico real configurado a -1.0 dBTP como lo último en tu cadena.

### Loudness inconsistente entre episodios

El episodio 1 está a -14 LUFS. El episodio 2 a -18 LUFS. El episodio 3 a -15 LUFS. Los oyentes tienen que ajustar el volumen cada episodio. Elige un objetivo y mantente firme.

## Referencia rápida

- **Objetivo: -16 LUFS** loudness integrado
- **Techo de true peak: -1.0 dBTP**
- **Procesa antes de normalizar** — nunca solo subas audio crudo
- **Sé consistente** en cada episodio

---

¿No quieres lidiar con medidores y limitadores? [Sube a Munchy Cow](/users/log-in) y se encarga del loudness automáticamente — junto con la cadena completa de procesamiento que hace que realmente suene bien a ese nivel. 3 horas gratis, sin tarjeta de crédito.

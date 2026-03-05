%{
  title: "Cómo grabar audio limpio en Audacity",
  description: "Consejos prácticos sobre elección de micrófono, ajuste de ganancia, tratamiento acústico y configuración de Audacity para conseguir grabaciones limpias desde el principio.",
  date: ~D[2026-03-05]
}
---

La regla de oro del audio es simple: si grabas mal, ningún plugin te salva. La buena noticia es que dedicar 15 minutos a prepararte bien te ahorra horas de edición y el resultado es mucho mejor.

Aquí tienes todo lo que necesitas para grabar audio limpio en Audacity.

## Elige el micrófono adecuado

Si grabas en una oficina o habitación de casa, **usa un micrófono dinámico**. Modelos como el Rode PodMic, Shure SM58 o Samson Q2U son asequibles y rechazan el ruido de fondo de forma natural. Son menos sensibles que los de condensador — y en un espacio sin tratamiento acústico eso es justo lo que quieres.

Los micrófonos de condensador captan más detalle, pero también recogen cada clic del teclado, el zumbido de un ventilador y los coches que pasan. Resérvalos para espacios tratados acústicamente.

Sea cual sea tu elección, asegúrate de que tenga un **patrón de captación cardioide**. Los cardioides rechazan el sonido de los lados y la parte trasera, manteniendo el ruido de la habitación fuera de tu grabación.

## Acércate al micrófono

Es el consejo que más diferencia marca. Colócate a **15-30 cm** del micrófono. Un truco rápido: unos cuatro dedos de ancho desde la cápsula.

Con un dinámico puedes acercarte aún más (5-8 cm) para un sonido más cálido e íntimo. Pero no pegues los labios — eso genera un exceso de graves por el efecto de proximidad.

**Inclina el micrófono unos 30 grados** respecto a tu boca en lugar de hablar de frente. Reduce las plosivas (esos golpes en la P, B y T) manteniendo un sonido completo y natural. Un filtro antipop también ayuda — hazte con uno si aún no tienes.

## Configura Audacity

Antes de darle a grabar, revisa estos ajustes:

**Frecuencia de muestreo:** 44.100 Hz es el estándar para voz. Usa 48.000 Hz si el audio va a acompañar un vídeo. Más allá de eso no hay beneficio práctico para voz.

**Profundidad de bits:** Mantén el valor por defecto de Audacity en 32-bit float para grabar y editar. Te da un margen enorme y un ruido de fondo muy bajo. Exporta a 16-bit para el archivo final.

**Canales:** Graba en **1 (Mono)**. La voz es una fuente mono — el estéreo duplica el peso del archivo sin ningún beneficio. Una hora en mono ocupa unos 310 MB, frente a 620 MB en estéreo.

## Ajusta bien la ganancia

Aquí es donde la mayoría de los principiantes la lían. Ajusta los niveles en el **hardware** — el botón de ganancia de tu interfaz de audio o micrófono USB — no en el software.

Así se hace:

1. En Audacity, haz clic en el icono del micrófono en la barra de medidores y selecciona **Start Monitoring**
2. Habla a tu volumen normal
3. Ajusta la ganancia de entrada hasta que los picos queden entre **-12 dBFS y -6 dBFS**
4. Deja margen para cuando te rías o te emociones — los picos nunca deben llegar a 0 dBFS

![Agranda el medidor de entrada de Audacity arrastrando su borde](/images/blog/resize-meter.png)

**Consejo:** Arrastra el borde del medidor de entrada para hacerlo más ancho — por defecto es diminuto y no se ve nada.

El clipping digital es una distorsión agresiva e irreparable. Siempre es mejor grabar un poco bajo que un poco alto.

**Error típico:** grabar demasiado bajo y luego subir con Amplify o Normalize en la edición. Eso sube todo — incluido el ruido de fondo. Mejor capturar un buen nivel desde el principio.

## Trata tu habitación (sin gastar mucho)

No necesitas un estudio profesional. Unos pocos cambios simples marcan una gran diferencia:

**Gratis:**
- Graba en una **habitación pequeña y amueblada** — alfombra, cortinas, sofá. Un armario lleno de ropa es una de las mejores cabinas vocales improvisadas que vas a encontrar
- Cuelga **mantas gruesas o edredones** en las paredes detrás y a los lados del micrófono
- Pon una **alfombra en suelos duros** para reducir las reflexiones
- Coloca **estanterías llenas de libros** frente al micrófono — difunden el sonido de forma natural

**Económico (25-30 € por panel):**
- Construye paneles acústicos caseros con marcos de madera, lana de roca y tela
- Colócalos en los puntos de primera reflexión: detrás del micrófono, detrás de ti y en las paredes laterales

**Orden de prioridad:** primero detrás del micrófono, luego detrás de ti, después los laterales y por último el techo.

## Elimina el ruido antes de grabar

La reducción de ruido en la edición siempre es un compromiso. Mucho mejor eliminarlo en su origen:

- **Apaga** el aire acondicionado, ventiladores, calefacción y cualquier sistema de climatización
- **Cierra ventanas y puertas** — incluso una ventana entreabierta deja entrar tráfico y viento
- **Apaga** los aparatos electrónicos innecesarios — monitores, discos duros externos y ordenadores de sobremesa son los culpables más habituales
- Si tienes que grabar cerca de un ordenador, apunta la **parte trasera del micrófono** (su punto de mayor atenuación) hacia el ordenador

**Para zumbidos eléctricos:**
- Enchufa todo el equipo en la **misma regleta** para evitar bucles de tierra
- Usa **cables XLR balanceados** en vez de jack de 3,5 mm — rechazan las interferencias electromagnéticas
- Con un micrófono USB, conéctalo directamente al ordenador — evita los hubs

Aplica estas bases y tus grabaciones sonarán más limpias que la mayoría de los podcasts — antes siquiera de abrir un plugin.

Y cuando quieras pulirlas, [prueba Munchy Cow](/users/log-in). Sube tu grabación y nos encargamos del resto — ruido de fondo, reverberación, niveles desiguales — sin hacerte sonar como un robot.

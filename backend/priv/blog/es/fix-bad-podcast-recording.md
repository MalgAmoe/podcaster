%{
  title: "Cómo rescatar una mala grabación de podcast",
  description: "Tu episodio suena terrible. Antes de regrabarlo o tirarlo a la basura, esto es lo que realmente se puede arreglar — y lo que no.",
  date: ~D[2026-03-08]
}
---

Acabas de terminar de grabar un gran episodio. La conversación fue perfecta. Luego lo escuchas y algo está mal. Quizás todo está mal.

Antes de entrar en pánico, regrabar o tirarlo — la mayoría de problemas de grabación se pueden arreglar en postproducción. Algunos no. Aquí te explico cómo distinguirlos.

## Problemas que puedes arreglar

### Ruido de fondo

Zumbido del aire acondicionado, ventiladores del ordenador, tráfico afuera, ruido de la nevera. No lo notaste mientras grababas porque tu cerebro lo filtró. El micrófono no.

**¿Qué tan malo puede ser y seguir siendo arreglable?** Bastante malo. La reducción espectral de ruido moderna puede eliminar ruido estacionario (zumbidos constantes, siseos, ruido de ventiladores) casi por completo sin afectar tu voz. Incluso el ruido moderado se limpia bien.

**Herramientas:** iZotope RX es el estándar de la industria para reparación manual. Audacity tiene reducción de ruido básica que funciona para casos leves. Adobe Podcast Enhance Speech usa IA y maneja bien el ruido pero puede sonar procesado. Munchy Cow ejecuta eliminación espectral de ruido automatizada y ajustada a cada archivo.

### Problemas de volumen

Una persona está fuerte, la otra baja. O el volumen salta porque alguien se aleja del micrófono.

**La solución:** Compresión y normalización. Un compresor nivela la dinámica. Luego la normalización establece el nivel general.

Si tienes pistas separadas para cada persona (deberías — siempre graba multipista), puedes nivelar a cada persona de forma independiente. Esto da mucho mejores resultados que arreglar una sola pista mezclada.

### Clicks de boca y pops

Esos sonidos húmedos de clic entre palabras, o los pops fuertes de plosivas en P y B. Más notorios con auriculares, y una vez que los oyes, no puedes dejar de oírlos.

**La solución:** Los algoritmos de de-clicking los detectan y eliminan automáticamente. Para las plosivas, un filtro pasa-altos a 80 Hz elimina el golpe de baja frecuencia sin afectar la voz.

### Sibilancia

Sonidos agudos y penetrantes de S y SH. Algunos micrófonos y voces son peores que otros. Causa fatiga auditiva.

**La solución:** Un de-esser detecta las frecuencias sibilantes (generalmente 4-8 kHz) y las reduce automáticamente.

### Sonido opaco o turbio

Tu grabación suena como si hablaras a través de una manta. Generalmente por estar demasiado lejos del micrófono, un micrófono de baja calidad, o reflexiones de la sala enturbiando los medios-bajos.

**La solución:** El EQ correctivo puede cortar la acumulación turbia de 200-400 Hz y añadir presencia en el rango de 2-5 kHz. La diferencia es dramática — es como limpiar el vaho de una ventana.

### Clipping leve

Momentos breves donde el audio distorsionó porque la entrada estaba demasiado alta. Si es ocasional — una risa fuerte o un grito emocionado — generalmente es arreglable.

**La solución:** Los algoritmos de de-clipping reconstruyen los picos de la forma de onda que fueron cortados. Funciona bien para clips breves.

## Problemas difíciles de arreglar

### Reverb fuerte de la sala

Este es el grande. Si tu grabación suena como si estuvieras en un baño o una habitación grande vacía — es extremadamente difícil de eliminar limpiamente.

**Por qué es difícil:** La reverb es tu voz mezclada con cientos de reflexiones de tu voz, todas a retardos y frecuencias ligeramente diferentes. Intentar eliminarla es como intentar separar la crema del café.

**Lo que es realista:**
- Sonido de sala leve (oficina pequeña, dormitorio): El de-reverb lo maneja bien.
- Reverb moderada (sala grande, suelos duros): Mejora notable pero no perfecta.
- Reverb fuerte (baño con azulejos, garaje vacío): Ningún software lo arregla limpiamente. Considera regrabar.

**La solución real:** Trata tu espacio de grabación. Mantas, alfombras, paneles de espuma, incluso grabar en un armario lleno de ropa.

### Clipping severo

Si toda la grabación está distorsionada — los niveles estaban demasiado altos todo el tiempo — no hay salvación. El de-clipping funciona con picos ocasionales, no con distorsión sostenida. Los datos originales de la forma de onda se perdieron.

**Prevención:** Graba a picos de -12 a -6 dBFS. Deja margen. Siempre puedes hacer el audio silencioso más fuerte, pero no puedes des-destruir audio clippeado.

### Crosstalk y filtración

Cuando el micrófono de una persona capta la voz de la otra, obtienes un sonido duplicado y con fase. No existe una solución automática fiable.

**Prevención:** Usa auriculares cerrados, mantén los micrófonos cerca de las bocas y lejos entre sí, y siempre graba pistas separadas.

## El flujo de trabajo de rescate

Si tienes una mala grabación que necesitas salvar, este es el orden de operaciones:

1. **Evalúa honestamente.** Escucha los peores 30 segundos. ¿Hay reverb fuerte o clipping sostenido? Si sí, considera si regrabar es mejor.
2. **Arregla lo arreglable primero.** Eliminación de ruido, de-reverb (si es leve), de-clicking — antes de cualquier otro procesamiento.
3. **Nivela la dinámica.** Compresión y nivelación vienen después de la limpieza.
4. **Moldea el tono.** EQ correctivo para arreglar problemas de frecuencia, luego mejora para añadir claridad.
5. **Establece los niveles finales.** Normalización de loudness y limitación al final.

El orden importa — EQ antes de la reducción de ruido hace el ruido más difícil de eliminar. Compresión antes del de-clicking hace los clicks más fuertes.

## La opción automática

Puedes hacer todo esto manualmente en un DAW con los plugins adecuados. Si sabes lo que haces, obtendrás buenos resultados. Si no, podrías empeorar las cosas — demasiada reducción de ruido suena robótica, EQ mal configurado suena hueco, sobre-compresión suena aplastada.

[Munchy Cow](/users/log-in) ejecuta toda esta cadena automáticamente. Cada archivo se analiza primero — piso de ruido, características de reverb, balance espectral, dinámica — luego cada etapa de procesamiento se ajusta a lo que se encontró.

3 horas gratis. Sin tarjeta de crédito. Sube tu peor grabación y mira qué sale.

## Cómo evitar necesitar rescate la próxima vez

- **Acércate al micrófono.** 15-30 cm. La mejora más grande que la mayoría de personas puede hacer.
- **Usa un micrófono dinámico** si tu sala no está tratada. Rechazan más sonido de sala que los de condensador.
- **Graba pistas separadas.** Siempre. Te da infinitamente más control en postproducción.
- **Monitorea con auriculares cerrados.** Para que escuches los problemas cuando ocurren.
- **Revisa tus niveles antes de empezar.** Apunta a picos alrededor de -12 dBFS.
- **Haz una grabación de prueba de 30 segundos** y escúchala antes de la real. Esto atrapa el 90% de los problemas.

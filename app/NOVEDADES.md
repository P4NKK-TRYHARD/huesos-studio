# Huesos Studio 0.2 — novedades

## Cómo arrancar
En la carpeta `app`: `cargo run --release` (Windows, Linux y Mac).
Prueba Archivo → «Cargar ejemplo de restricciones» y pulsa Espacio.

## Lo nuevo
- **Slots y skins**: cada hueso puede tener varios slots (capas) y cada slot varios adjuntos; el visible se puede animar. Las skins cambian los adjuntos del mismo esqueleto (selector «Skin» arriba).
- **Mallas deformables**: «Convertir en malla deformable» en un adjunto, herramienta **Vértices (M)** para mover puntos. En modo animar crea claves de deformación.
- **Pesos**: marca huesos y pulsa «Calcular pesos» para que la malla se doble con ellos.
- **Restricciones**: IK (1 o 2 huesos, con suavidad, estirar y comprimir), transformación, trayectoria y física. Sus mezclas se pueden animar.
- **Curvas Bézier** por clave con editor de asas, y **vista de gráfico (G)** para editar valores en el tiempo.
- **Eventos**: se definen en el panel izquierdo, se colocan con «✚ Evento» en la línea de tiempo y se ven al reproducir.
- **Color de slot**, **orden de dibujo** y **cizalla** animables; modos de **herencia** del hueso.
- **Recorte**, **caja de colisión** y **punto** como adjuntos.
- **Spine**: Archivo → «Exportar a Spine (JSON + atlas)» y «Importar de Spine (.json)». Compatible con Spine 4.2 (importa también 3.8).
  Para usar la exportación en Unity, Godot, etc. con los runtimes oficiales de Spine hace falta licencia de Spine.

Los proyectos de la versión anterior se abren y se convierten solos.

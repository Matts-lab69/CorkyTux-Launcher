# CorkyTux: sistema visual de Settings > Emulators

## Estado del documento

- Estado: **actualización visual de la fila implementada**: el nombre ya no lleva sufijos de estado; `Linked` conserva una píldora compacta; `System` conserva el badge junto al nombre, muestra un aviso rojo estático `Found in system` y mantiene el botón `Set as linked` como única acción clickeable. El flujo `Working…`, el slot estable y el mapping de acciones permanecen intactos.
- Alcance: jerarquía, espaciado y tratamiento visual de las filas de emuladores, con foco en la identidad, el estado y las acciones disponibles.
- Fuentes verificadas: `src/ui/settings.rs`, `rebuild_emu_rows` y el handler `EmuOp`; `src/backend/theme.rs`, tokens de tema y paleta de acentos; `src/ui/helpers.rs`, CSS de aplicación; `assets/icons/ATTRIBUTION.md` para las licencias de los assets.
- Cambios de esta versión: nombre canónico sin paréntesis; `.emu-status-pill` para `Linked`/`System`/nativo; aviso estático `.emu-system-notice` sin hover, cursor ni callback; botón `Set as linked` con `.emu-link-action`; el callback `link_emulator_in` no cambia. Las secciones de opciones A/B/C se conservan como trazabilidad histórica, pero la implementación vigente es badge + aviso estático + botón.
- Sección "Lista de emuladores instalables (catálogo)": **opción 1+3 fijada como la única viva** (botón estado-consciente + compactación de filas resueltas), con matriz estado → badge/label/clase/ancho. La fila se alinea al estilo de tarjeta de lista de contenido (Prism/Modrinth): clase `.emu-row-card` y slot de ícono cuadrado 48 px `.emu-row-icon` (11 SVG de Papirus; el logo oficial de Azahar queda atribuido pero el resolver conserva el monograma).
- No se toca la lógica de estados, la matriz estado→acción ni los handlers en esta actualización.

## Decisiones fijas y valores por defecto

**Decisiones fijas**

- La jerarquía separa disponibilidad, identidad, estado, feedback operacional y acción.
- El badge comunica estado; el aviso rojo comunica un hallazgo; el botón comunica la acción. `Linked` conserva la píldora pequeña; `System` conserva su badge y usa el aviso estático `Found in system` separado de `Set as linked`.
- No se crean colores fuera de los tokens existentes; los derivados (como `{accent_ui}`) se calculan en la generación de CSS, no se hardcodean.
- `Working…` no reemplaza la señal que describe el `source`: ni la píldora de estado ni el aviso de `System` desaparecen durante la operación.
- La acción conserva semántica de `gtk::Button`, teclado y foco; el aviso es un `gtk::Label` sin señal de puntero.
- El label de estado es un slot estable y está siempre presente en la fila (sección "Slot estable").
- Durante la operación el botón queda insensible; el doble click no produce una segunda operación (sección "Flujo `Working…` y doble click").
- El borde/foco de la acción usa `{accent_ui}`: en claro `color-mix(in srgb, {text_main} 30%, {accent})`; en oscuro `{accent}` (sección "Contraste 3:1").

**Defaults propuestos, pendientes de aprobación**

- Adwaita Sans como cara nominal heredada de libadwaita.
- Centrado vertical sobre un ritmo visual de 36 px.
- Sin animación ornamental ni cross-fade de badges.
- La opción vigente es la composición final: badge `System` + aviso rojo estático + botón `.emu-link-action`; las opciones A/B/C quedan como trazabilidad.

## Objetivo

Hacer que cada fila se lea de izquierda a derecha como una frase operacional inequívoca:

1. disponibilidad;
2. identidad del emulador;
3. estado actual de su fuente;
4. acción disponible.

La regla central es que la señal de estado responda `¿de dónde viene?` y el botón responda `¿qué puedo hacer ahora?`. `Linked` usa una píldora pequeña; `System` conserva su badge, muestra el aviso estático `Found in system` y termina en el botón `Set as linked`.

## Tesis de diseño

Una fila debe sentirse como un control compacto y confiable: superficie limpia, tipografía nativa, ritmo de 4/8 px y un solo acento semántico por función. El estado conserva una cápsula pequeña y transparente; la acción conserva un objetivo táctil de 36 px, pero cambia de superficie, radio, peso o borde para pesar más que el estado sin competir con una acción primaria como `Install`.

## Base tipográfica

- Familia: mantener la tipografía nativa de GTK/libadwaita, con **Adwaita Sans** como cara nominal y el fallback del sistema. Esta decisión local no debe introducir una familia nueva mediante CSS.
- Identidad de la fila: `.details-title`, 16 px, bold, `text_main`.
- Acción: 14 px, bold.
- Estado de proceso `Working…`: `.time-label`, 12 px, `text_sec`.
- Badge `Linked`/nativo: `.proton-path-badge` + `.emu-status-pill`, 11 px, bold, `min-height: 20px` y padding horizontal reducido.
- ~~Punto de disponibilidad: 14 px, `emu-dot-on` u `emu-dot-off`.~~ **Obsoleto**: el dot `[•]` se eliminó de la fila por decisión del usuario (ver matriz y pendientes de la sección catálogo); las clases quedaron sin uso, candidatas a eliminar.
- No usar una segunda familia, mayúsculas decorativas ni texto itálico.

## Tokens existentes

### Superficies y texto

| Token | Tema claro | Tema oscuro | Uso en la fila |
| --- | --- | --- | --- |
| `bg` | `#F4F1F8` | `#000000` | Fondo general de la app. |
| `panel` | `#FFFFFF` | `#121212` | Panel de Settings y superficie de la tarjeta que contiene la lista. |
| `card` | `#FFFFFF` | `#181818` | Superficie de tarjeta. |
| `well` | `#ECE8F2` | `#181818` | Fondo de control secundario. |
| `hover` | `#E3DCEE` | `#282828` | Hover de fila o botón. |
| `border` | `#D8D0E3` | `#282828` | Bordes neutros. |
| `text_main` | `#241F2E` | `#E0E0E0` | Nombre y etiquetas de acciones. |
| `text_sec` | `#5B5468` | `#AAAAAA` | `Working…` y metadatos secundarios. |
| `text_muted` | `#8A8296` | `#777777` | Punto no disponible. |

El patrón destructivo conserva `#E5484D`; `Remove` lo usa como acción destructiva y el aviso estático de `System` lo usa como advertencia, sin ser un badge ni un control clickeable.

### Accent configurable

| ID | Accent | Valor base |
| --- | --- | --- |
| 0 | green | `#1db954` |
| 1 | blue | `#1E88E5` |
| 2 | cyan | `#00BCD4` |
| 3 | purple | `#AB47BC` |
| 4 | pink | `#EC407A` |
| 5 | red | `#EF5350` |
| 6 | orange | `#FFA726` |
| 7 | yellow | `#FFEE58` |
| 8 | teal | `#26A69A` |
| 9 | indigo | `#5C6BC0` |

- El usuario actual tiene `Accent=9`, indigo `#5C6BC0`.
- En tema oscuro, `{accent}` usa el valor base.
- En tema claro, `{accent}` usa `darken(hex, 0.22)`.
- `on_accent` es `#FFFFFF` en oscuro y `text_main` (`#241F2E`) en claro.
- Las especificaciones siguientes deben seguir usando `{accent}`; no se debe hardcodear el indigo ni el resultado del oscurecimiento. El borde derivado `{accent_ui}` responde a esta misma regla: se calcula, no se hardcodea.

### Vocabulario de componentes ya existente

| Componente o patrón | Uso establecido | Decisión para esta fila |
| --- | --- | --- |
| `.add-btn` | 14 px bold, 36 px de alto, radio 20 px, fondo accent y `on_accent`. | Reservado para `Install`, la única acción primaria. |
| `.settings-btn` | 14 px bold, `well`, borde accent de 1.5 px y radio 20 px. | Candidato directo para la opción A. |
| `.danger-btn` | Relleno rojo translúcido, borde `#E5484D`, texto del mismo color y 36 px. | Reservado para `Remove`. |
| `.proton-path-badge` + `.emu-status-pill` | Texto accent de 11 px, borde de 1 px, radio 10 px, `min-height: 20px` y padding reducido. | Píldora compacta para `Linked`/nativo. |
| `.emu-link-action` | Acción táctil de 14 px, 36 px, radio 20 y relleno accent translúcido. | Se usa en `Set as linked`; no altera `Remove` ni `Install`. |
| `.emu-system-notice` | `gtk::Label` estático de 14 px, 36 px, radio 8, fondo/borde/texto rojo suave (`#E5484D`). | Comunica `Found in system`; no tiene `:hover`, `:disabled`, cursor ni `connect_clicked`. |
| `.action-btn` | 11 px bold, radio 18 px y `action_bg` (`#242424` en oscuro, `well` en claro). | No usar para esta acción: su escala corresponde más a una barra compacta. |
| `.filter-btn` | `well`, con checked en `hover`, texto accent y borde accent. | No usar: checked comunica selección de filtro, no acción de vínculo. |
| `.icon-ghost` y `.source-link` | Indicadores transparentes o textuales. | Son referencias visuales para la opción C, no clases para reutilizar sin ajustes. |
| `.destructive-action` | Geometría de 36 px, 14 px bold, padding horizontal de 16 px y radio 20 px. | Aporta ritmo, no color destructivo. |
| `.dark-btn` | Botón negro con borde blanco. | Excluido: tendría demasiado peso para una acción opcional. |
| `.mc-account > button`, `.import-mode:checked`, filas selected/hover | Ya combinan `well`, `hover` y `color-mix` con accent para distinguir superficies y estados. | Confirman que un relleno tenue es un patrón propio del producto, no una invención local. |

## Estructura real verificada

`rebuild_emu_rows` crea un `gtk::Box` horizontal por emulador, con estas propiedades:

- separación horizontal: 8 px;
- margen superior e inferior de cada fila: 2 px;
- lista vertical: separación 4 px, por lo que la separación visual efectiva entre filas vecinas es de 8 px;
- margen inicial de la lista: 16 px;
- orden actual de hijos: icono, bloque de texto (nombre + badge opcional, descripción), slot operacional, aviso opcional y acción opcional;
- el label flexible usa `.details-title`, alineación al inicio y elipsado al final; el nombre ya no concatena sufijos `(system)`, `(linked)`, `(native)`, `(appimage)` ni `(installed)`;
- el estado operacional usa `.time-label` y reserva 12 caracteres; con el slot estable este label existe siempre, vacío en reposo (ver sección "Slot estable");
- `Linked`/`System`/nativo usa `.proton-path-badge` más `.emu-status-pill`; el aviso `System` es un label estático separado y el botón sigue siendo un `gtk::Button`;
- `Found in system` reserva 140 px; `Set as linked` reserva 120 px; las demás acciones reservan 80 px.

El nombre es la identidad canónica y se muestra solo, con `.details-title`; la descripción ocupa la segunda línea con `.time-label`. La señal de estado se renderiza aparte: `Linked` y `System` como píldora compacta, `System` además como aviso no interactivo y `Set as linked` como acción.

## Matriz semántica de la fila

| Source | Señal de disponibilidad | Badge/etiqueta | Acción |
| --- | --- | --- | --- |
| `linked` | punto encendido | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | ninguna; estado terminal |
| `system` | punto encendido | `System` (`.proton-path-badge` + `.emu-status-pill`) + aviso estático `Found in system` (`.emu-system-notice`, 140 px) | `Set as linked` (`.emu-link-action`, 120 px) |
| `appimage` | según disponibilidad reportada | — | `Remove` |
| `none` o vacío, no nativo | punto apagado si no está instalado | — | `Install` o `Remove` según `installed` |
| nativo, source vacío | punto encendido | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | ninguna |

`Working…` describe una operación en curso, no el `source`. Por eso permanece en su slot estable; la acción de `System` permanece visible y se deshabilita durante la operación.

## Sistema de fila

### Jerarquía visual

Orden de lectura recomendado:

```text
[icono 48px] [identidad flexible + badge] [descripción] [Working…] [aviso estático] [acción]
```

- **Dot:** eliminado de esta fila; la señal accionable o la ausencia de botón comunican el estado.
- **Identidad:** bloque principal flexible. Es lo que primero se lee y lo último que debe recortarse.
- **Working…:** feedback transitorio en `text_sec`; no usa un segundo badge.
- **Linked:** vocabulario de estado en una píldora pequeña, transparente, con texto accent, borde de 1 px, radio de 10 px, 11 px bold y altura mínima de 20 px.
- **System:** conserva la píldora de estado junto al nombre; el aviso `Found in system` es estático y no tiene affordance; `Set as linked` es la única acción clickeable.
- **Acción:** último elemento de la fila, con etiqueta imperativa explícita, objetivo táctil de 36 px y 14 px bold.

### Espaciado y alineación

- Mantener 8 px entre los hijos de la fila; el `gtk::Box` resuelve el ritmo.
- No agregar márgenes individuales alrededor de la píldora, el aviso o el botón.
- Centrar verticalmente los hijos sobre una altura visual mínima de 36 px; el slot de icono y el `GtkImage` usan `Center` en ambos ejes.
- Mantener el label de identidad con expansión horizontal y elipsado final.
- Reservar 140 px para el aviso `Found in system` y 120 px para `Set as linked`; el texto de ambas piezas no debe recortarse. Las demás acciones conservan 80 px.
- Mantener `Working…` en un slot estable de 12 caracteres para evitar saltos de layout durante la operación.

### Regla contra dos cápsulas accent iguales

En una misma fila no se permiten dos chips con el mismo lenguaje visual:

1. `Linked`/`System`/nativo usa una única píldora de estado junto al nombre: fondo transparente, 11 px, radio 10 px, borde accent de 1 px y altura mínima de 20 px.
2. Una acción en forma de cápsula debe usar 14 px bold, radio 20 px y distinguishse por su superficie o peso.
3. `Install` conserva `.add-btn` y es la única acción primaria con fondo accent sólido.
4. `Remove` conserva `.danger-btn`: contorno rojo con relleno translúcido.
5. `System` añade un aviso `.emu-system-notice` estático; no es badge, botón, enlace ni elemento con hover.
6. `Set as linked` sigue siendo un `gtk::Button` nativo para conservar teclado, foco y semántica; el handler existente no se duplica.
7. El borde accent de las acciones no se mezcla con el rojo de advertencia: el aviso usa `#E5484D` y el botón conserva `.emu-link-action`.

### Estado transitorio y no-animación

- No animar el cambio de badge ni la altura de la fila. Los cambios de hover/focus y la aparición de `Working…` son inmediatos y utilitarios.
- `Working…` no se convierte en spinner ni duplica el mensaje del badge.

### Flujo `Working…` y doble click (especificación funcional)

Comportamiento actual verificado en el código:

- El click del botón inserta `"Working…"` en el mapa `emu_status`, actualiza el label estable de la fila y lanza un thread que envía `EmuOp` al terminar.
- El aviso `Found in system` y el badge `System` permanecen visibles; el aviso no tiene callback.
- El handler de `EmuOp` (éxito o error) limpia el mapa `emu_status`, escribe feedback en el label de estado global y reconstruye la lista.

Mecanismo vigente:

1. **Reposo:** `System` muestra badge `System`, aviso estático `Found in system`, slot estable vacío y botón sensible `Set as linked`.
2. **Click (síncrono, antes del thread):**
   a. Guard de reentrada: `if !b.is_sensitive() { return; }`.
   b. `b.set_sensitive(false)`.
   c. `set_text("Working…")` en el label de la fila (mutación directa del widget capturado en el closure).
   d. Mantener el insert en `emu_status` para continuidad si ocurre un rebuild a mitad de operación.
   e. Thread + envío de `EmuOp` sin cambios respecto del código actual.
3. **Doble click antes de la respuesta:** el segundo click no dispara nada. GTK4 no despacha eventos de puntero a widgets insensibles, y `set_sensitive(false)` se aplica dentro del primer handler, antes de evaluarse el segundo press. El guard del punto 2 cubre además un `activate()` programático.
4. **Rebuild a mitad de operación:** si un refresco de la lista (`settings.rs:1649`) recrea la fila mientras corre la operación, el botón nuevo nace deshabilitado: al construir una fila `system`, si `emu_status` contiene `"Working…"` para ese emulador, `btn.set_sensitive(false)`. El lock es así rebuild-proof y no depende de la vida del widget clickeado.
5. **Re-activación (cómo y cuándo):** el botón deshabilitado no se re-habilita en el mismo widget; el widget se descarta y la recuperación la decide el resultado vía el rebuild de `EmuOp`:
   - En éxito, `emu_status` se limpia y la lista nueva convierte la fila en `Linked` (badge solo, sin botón; estado terminal).
   - En error, `emu_status` se limpia, el mensaje va al status global y la fila vuelve a `System` con badge, aviso y botón nuevo sensible `Set as linked`.
   No existe una ruta donde un botón viejo se re-habilite.
6. **Durante la operación la acción `System` permanece visible y se deshabilita;** el feedback no la reemplaza.
7. **Feedback:** `Working…` es visible desde el click hasta el rebuild del resultado. En éxito, el label de estado global ya muestra el mensaje del resultado.

### Slot estable (decisión)

**Decisión: reservar el ancho siempre** — el label de estado existe en toda fila, por defecto vacío; no se acepta el corrimiento de layout.

- El label se crea siempre con `set_width_chars(12)` y texto vacío en reposo; el click solo hace `set_text("Working…")` y el resultado vuelve a reconstruir la fila con el slot vacío.
- **Mecanismo:** el label conserva el mismo `set_width_chars(12)` y el rebuild gestiona el estado vacío después del resultado.
- **Por qué:**
  1. El slot estable ya está prometido en este documento y es la base de la mutación directa del flujo anterior.
  2. Mantiene la columna de badges y botones alineada entre filas; la lista se escanea por columnas.
  3. Evita que el objetivo de la acción se mueva mientras el usuario espera el resultado.
- **Costo aceptado:** un hueco vacío de ~12 caracteres en reposo por fila; acotado porque `Working…` ocupa 9 caracteres a 12 px, por lo que el hueco no excede el tamaño ya previsto.
- **Alternativa rechazada** (aceptar el corrimiento): rompe la alineación entre filas, contradice la promesa del documento y mueve el target durante la operación.

### Contraste 3:1 del borde y foco con los 10 acentos

Problema (hallazgo mayor del review): el borde/foco en `{accent}` no garantiza 3:1 (WCAG 1.4.11, no-texto/componentes) con todos los acentos en tema claro. Concretamente, el amarillo ya oscurecido al 22% (`#c6b944`) da **2.01:1** contra el panel `#FFFFFF`; sin el darken previo sería aún más claro (≈1.2:1).

Resolución:

- Se define un token derivado **`{accent_ui}`** para el borde y el foco de la acción, calculado en la generación de CSS (los estilos se construyen en runtime en `helpers.rs` con el accent vigente; no se hardcodea ningún hex):
  - Tema claro: `color-mix(in srgb, {text_main} 30%, {accent})` — donde `{accent}` ya es `darken(hex, 0.22)`.
  - Tema oscuro: `{accent}` sin mezcla: los diez acentos base ya cumplen ≥3.85:1 contra el panel `#121212`.
- El 30% (y no el mínimo matemático de 25%) da margen para la superficie de fondo de ventana `#F4F1F8` y para el anti-aliasing del render.
- `color-mix` ya se usa en la app (`.danger-btn`, `.import-mode:checked`), por lo que la versión de GTK en uso lo soporta; la regla es universal y cubre los 10 acentos sin tabla en runtime.

Verificación en tema claro (accent oscurecido 22%, mezclado 30% con `text_main`):

| Accent | Valor base | `{accent}` claro | Borde `{accent_ui}` | vs `#FFFFFF` | vs `#F4F1F8` |
| --- | --- | --- | --- | --- | --- |
| green | `#1db954` | `#169041` | `#1a6e3b` | 6.29 | 5.63 |
| blue | `#1E88E5` | `#176ab2` | `#1b538a` | 7.92 | 7.09 |
| cyan | `#00BCD4` | `#0092a5` | `#0b6f81` | 5.83 | 5.21 |
| purple | `#AB47BC` | `#853792` | `#683074` | 9.28 | 8.30 |
| pink | `#EC407A` | `#b8315f` | `#8c2c50` | 8.10 | 7.25 |
| red | `#EF5350` | `#ba403e` | `#8d3639` | 7.72 | 6.91 |
| orange | `#FFA726` | `#c6821d` | `#956422` | 5.09 | 4.55 |
| yellow | `#FFEE58` | `#c6b944` | `#958b3d` | 3.48 | 3.11 |
| teal | `#26A69A` | `#1d8178` | `#1f6462` | 6.87 | 6.14 |
| indigo | `#5C6BC0` | `#475395` | `#3c4376` | 9.31 | 8.32 |

Todos ≥3.1:1. En oscuro el mínimo es indigo/purple con 3.85:1 contra `#121212`.

Aplicación al botón `Set as linked`: el borde de 1.5 px usa `{accent_ui}`; el relleno 14%/22% conserva `{accent}` (es decorativo detrás de un texto `text_main` y no constituye el contorno del control); el indicador de foco nativo de GTK se mantiene. El aviso rojo de `System` no usa este token porque no es interactivo.

## Opciones para `Set as linked`

> Las opciones A/B/C siguientes son exploración histórica anterior a la actualización visual. La decisión vigente está en "Estado del documento" y en la matriz de la sección de flujo: `Linked` conserva la píldora compacta; `System` conserva badge + aviso estático `Found in system` y el botón `Set as linked`.

### Opción A: Cápsula anclada

**Reutilizar clase:** `.settings-btn`.

**Descripción visual:** una píldora neutra con superficie `well`, borde accent de 1.5 px, texto `text_main`, radio 20 px y 14 px bold. El badge `System` queda transparente; la acción gana peso por su fondo lleno, su radio más redondeado, su borde más grueso y su texto más grande.

**Regla CSS existente:**

```css
.settings-btn {
  font-size: 14px;
  min-height: 36px;
  padding: 0 16px;
  border-radius: 20px;
  background-color: {well};
  color: {text_main};
  border: 1.5px solid {accent};
  font-weight: bold;
}

.settings-btn:hover {
  background-color: {hover};
}
```

**Claro:** fondo `#ECE8F2` sobre panel `#FFFFFF`, texto `#241F2E` y borde accent oscurecido. La diferencia de superficie es clara aunque el accent sea de luminancia baja.

**Oscuro:** fondo `#181818` sobre panel `#121212`, texto `#E0E0E0` y borde del accent vigente. En el accent actual, el borde usa `#5C6BC0`.

**Durante `Working…`:** la cápsula permanece en el mismo lugar, se desactiva y el feedback aparece separado en `text_sec`. El badge no se convierte en un indicador giratorio ni se duplica el mensaje.

**Pros:**

- máxima coherencia con componentes ya usados;
- objetivo táctil y foco nativos sin crear una familia nueva de estilos;
- `text_main` ofrece mejor contraste que depender del color accent para la etiqueta;
- es fácil de reconocer como botón.

**Contras:**

- sigue habiendo dos siluetas redondeadas adyacentes; se distinguen por relleno, no por eliminación de la segunda cápsula;
- el borde accent puede dar a la acción un aspecto similar al de un filtro o control de configuración;
- reutilizar una clase llamada `settings-btn` documenta estilo, no intención específica de fila.

### Opción B: Cápsula activa

**Clase nueva propuesta:** `.emu-link-action`.

**Descripción visual:** una acción táctil con relleno translúcido accent, borde accent de 1.5 px (token derivado `{accent_ui}`, sección "Contraste 3:1") y texto `text_main`. El badge continúa vacío/transparente; la acción queda claramente rellena. El texto no usa accent para mantener legibilidad con todos los acentos, especialmente los amarillos y claros.

**Regla CSS propuesta:**

```css
.emu-link-action {
  font-size: 14px;
  min-height: 36px;
  padding: 0 16px;
  border-radius: 20px;
  background-color: color-mix(in srgb, {accent} 14%, transparent);
  border: 1.5px solid {accent_ui};
  color: {text_main};
  font-weight: bold;
}

.emu-link-action:hover {
  background-color: color-mix(in srgb, {accent} 22%, transparent);
}

.emu-link-action:disabled {
  background-color: color-mix(in srgb, {accent} 8%, transparent);
  border-color: {text_muted};
  color: {text_muted};
}
```

`{accent_ui}` se deriva en la generación de CSS: `color-mix(in srgb, {text_main} 30%, {accent})` en claro, `{accent}` en oscuro (sección "Contraste 3:1"). No se debe eliminar el indicador de foco nativo de GTK.

**Claro:** el relleno tenue se compone sobre `#FFFFFF`; el accent se oscurece automáticamente 22%; el borde usa `{accent_ui}` (mezcla 30% con `text_main`) y el texto permanece `#241F2E`. El botón se ve como una superficie lavada por el accent, no como otro contorno vacío.

**Oscuro:** el relleno tenue se compone sobre `#121212`; el texto es `#E0E0E0` y el borde usa el accent sin oscurecer (todos los acentos cumplen ≥3.85:1). Para indigo actual, el borde es `#5C6BC0` y el relleno queda dentro de la misma familia tonal.

**Durante `Working…`:** el botón queda insensible y su estado `:disabled` lo comunica; el label transitorio precede al badge `System`, por lo que se lee `identidad -> Working… -> System -> acción`. El mecanismo completo (guard de doble click, lock rebuild-proof y re-activación por resultado) está especificado en la sección "Flujo `Working…` y doble click".

**Pros:**

- mejor equilibrio entre descubribilidad y jerarquía: se ve accionable sin parecer tan primaria como `Install`;
- no duplica el badge porque una cápsula está vacía y la otra tiene relleno;
- mantiene el texto de acción en `text_main`, por lo que la legibilidad no depende de la luminancia del accent;
- reutiliza el lenguaje ya aceptado de `color-mix` en `.import-mode:checked` y `.danger-btn`;
- el borde `{accent_ui}` garantiza 3:1 con los 10 acentos en ambos temas.

**Contras:**

- el relleno al 14% puede ser sutil para acentos claros, especialmente en tema claro (mitigado por el borde `{accent_ui}` bien contrastado);
- requiere una clase localizada para evitar cambiar el significado de `.settings-btn` en toda la app;
- el comportamiento de doble click y re-activación depende de la especificación funcional, no solo del CSS.

### Opción C: Link ghost accesible

**Clase nueva propuesta:** `.emu-link-ghost`.

**Descripción visual:** la fila termina con un texto de 14 px bold, sin cápsula permanente. El label usa `text_sec` para mantener contraste; en hover pasa a `text_main` y se subraya, mientras el foco puede delinearse con `{accent}`. Visualmente queda `badge + enlace`, sin dos cápsulas. Sigue siendo un `gtk::Button`, no una etiqueta clickeable.

**Regla CSS propuesta:**

```css
.emu-link-ghost {
  background-color: transparent;
  border: none;
  color: {text_sec};
  font-size: 14px;
  font-weight: bold;
  min-height: 36px;
  padding: 0 8px;
}

.emu-link-ghost:hover {
  color: {text_main};
  text-decoration-line: underline;
}

.emu-link-ghost:focus {
  outline: 1.5px solid {accent};
  outline-offset: 2px;
}
```

**Claro:** texto `#5B5468` sobre `#FFFFFF`; hover `#241F2E` y contorno con accent oscurecido. No se fuerza texto accent permanente porque el amarillo y otros acentos claros no ofrecen una garantía de contraste consistente.

**Oscuro:** texto `#AAAAAA` sobre `#121212`; hover `#E0E0E0` y contorno con el accent vigente. Para indigo actual, el foco usa `#5C6BC0`.

**Durante `Working…`:** el enlace permanece en su lugar y se desactiva. El espacio de 36 px mantiene el objetivo táctil aunque no haya fondo permanente.

**Pros:**

- es la solución más limpia y la que elimina por completo la competencia entre dos cápsulas;
- reduce ruido visual en una lista que puede contener muchas filas;
- mantiene el foco visible y un objetivo de 36 px;
- el texto de la acción mantiene contraste con `text_sec`/`text_main` en ambos temas.

**Contras:**

- tiene menos peso visual y puede leerse como enlace estático si no se reconoce el texto imperativo;
- un hover necesario para descubrir el indicador de enlace puede ser insuficiente en touchpad o teclado;
- la ausencia de fondo hace que `Set as linked` se mezcle más con `Remove` si las demás acciones no siguen una jerarquía estricta;
- no conviene reutilizar `.source-link` directamente: está diseñado como label de 12 px. `.icon-ghost` solo tampoco sirve porque elimina la altura táctil.

## Comparación

| Criterio | A: Cápsula anclada | B: Cápsula activa | C: Link ghost |
| --- | --- | --- | --- |
| Claridad como botón | alta | alta | media |
| Separación frente al badge | media-alta | alta | muy alta |
| Jerarquía frente a `Install` | correcta | correcta | correcta, más discreta |
| Contraste de etiqueta | alto con `text_main` | alto con `text_main` | alto con `text_sec`/`text_main` |
| Coherencia con tokens | máxima | alta | alta |
| Ruido visual | medio | medio-bajo | mínimo |
| Riesgo principal | dos cápsulas todavía visibles | relleno sutil con acentos claros | baja descubribilidad |

## Decisión

**Composición vigente: badge `System` + aviso estático `Found in system` + botón B (`.emu-link-action`) con texto `Set as linked`.**

Es el mejor equilibrio entre estado, advertencia y acción:

1. **Semántica:** el badge `System` comunica estado; el label rojo comunica el hallazgo; `Set as linked` comunica la acción y conserva su callback.
2. **No duplicación:** el aviso no es un segundo botón ni tiene affordance; el botón conserva texto, foco y semántica nativos.
3. **Jerarquía:** `Install` conserva el único accent sólido; `Set as linked` es secundaria pero descubrible; `Remove` conserva el lenguaje destructivo.
4. **Accesibilidad:** el contenido del botón no depende del contraste del accent; el objetivo mide 36 px y el borde `{accent_ui}` mantiene el contraste del control.
5. **Aviso:** `.emu-system-notice` es un `gtk::Label` estático de 36 px, sin `:hover`, cursor ni `connect_clicked`.
6. **Coherencia:** el slot de icono sigue siendo 48 px y usa `Center`; no se altera `Install`/`Remove`, la matriz estado→acción ni `Working…`.

Las opciones A y C quedan documentadas como alternativas históricas; no se eliminan del documento para conservar la trazabilidad de la decisión.

## Riesgo deliberado y mitigación

El riesgo principal es que el aviso rojo se perciba como una acción. La mitigación es deliberada y acotada: el aviso es un `gtk::Label` sin hover, cursor ni callback, y el botón conserva el lenguaje `.emu-link-action`.

- el badge no tiene relleno;
- la acción usa 14 px frente a 11 px del badge;
- la acción usa radio 20 px frente a 10 px;
- la acción tiene borde de 1.5 px frente a 1 px;
- el label de la acción usa `text_main`, no accent;
- solo `Install` recibe un relleno accent sólido;
- el borde de la acción usa `{accent_ui}` (≥3:1 con los 10 acentos) mientras el badge conserva `{accent}`; el contorno de la acción es más profundo pero permanece dentro de la familia accent.

## Criterios de aceptación para una implementación futura

- La fila conserva el orden, los espaciados y el objetivo existentes.
- El estado y la acción nunca aparecen como dos badges idénticos.
- `System` permanece visible durante `Working…`, con badge, aviso estático y botón en la misma geometría.
- La acción se desactiva durante la operación y se restaura al responder.
- Éxito produce una fila `Linked` sin acción; error restaura `Set as linked` y muestra feedback separado.
- Todas las clases propuestas interpolan tokens existentes; no aparece un hex adicional.
- El foco de teclado permanece visible y la acción conserva un objetivo táctil de al menos 36 px.
- El doble click en `Set as linked` no produce una segunda operación: guard de reentrada + `set_sensitive(false)` síncrono + lock rebuild-proof cuando el mapa contiene `Working…`.
- `Working…` es visible desde el click hasta el rebuild del resultado, no solo al final.
- El label de estado existe en todas las filas (slot estable); el layout no se corre al aparecer el feedback.
- El borde de la acción cumple ≥3:1 con los 10 acentos en claro y oscuro (regla `{accent_ui}`).
- `.emu-link-action` define `:disabled` (relleno 8%, borde y texto `text_muted`).

## Decisión final

- Composición vigente: badge `System`, aviso estático `.emu-system-notice` y botón `.emu-link-action` con texto `Set as linked`.
- Las resoluciones del review quedan especificadas en este documento: flujo `Working…` y doble click, slot estable, contraste 3:1 con los 10 acentos y separación explícita entre aviso y acción.
- La implementación cubre CSS de `.emu-link-action` y `.emu-system-notice`, token derivado `{accent_ui}`, label de estado estable, lock rebuild-proof y handler `link_emulator_in` sin duplicación.

## Lista de emuladores instalables (catálogo)

### Contexto y alcance

Esta sección conserva la especificación del catálogo y su trazabilidad; la implementación actual de la fila está en `rebuild_emu_rows` y en las clases descritas arriba.

Es la misma función `rebuild_emu_rows`, pero en su modo catálogo: cuando `emulator-manager` reporta el catálogo y la mayoría de las entradas están en source `none`, la lista mantiene el botón `Install` para pendientes y compacta las filas resueltas. El filtro por nombre/descripción y la identidad visual de cada fila siguen siendo parte de la opción viva.

Objetivos de esta propuesta: jerarquía (nombre ≠ descripción), exploración (búsqueda), contexto (consola/generación) y un slot estable para el ícono del emulador. El slot usa los 11 SVG verificados de Papirus; el logo oficial de Azahar está disponible y atribuido, pero el resolver conserva el monograma por el alcance de esta actualización.

### Estado actual verificado

- Fila actual: `gtk::Box` horizontal, separación 8 px, márgenes verticales 2 px:
  `[icono 48px] [nombre + badge] [descripción] [slot estable] [aviso estático opcional] [acción]`.
- El nombre usa solo `emu.name` con `.details-title` (16 px bold, `text_main`); ya no concatena sufijos de `source`. La descripción se mantiene en la segunda línea con `.time-label`.
- `Linked`/nativo muestra `Linked` en una píldora de 20 px; `System` muestra `System` en una píldora y un aviso rojo estático; `none` no muestra badge.
- Botón `Install`: `.add-btn` (relleno accent sólido, **30 px, radio 15 px**), ancho fijo 80 px. `Set as linked`: `.emu-link-action`, ancho 120 px. `Remove`: `.danger-btn`, ancho 80 px. Las tres clases comparten métricas: `font-size: 14px`, `min-height: 30px`, `padding: 0 14px`, `border-radius: 15px` — una cápsula de 2,1× la fuente, que es la proporción cómoda; antes eran 36 px (2,6×) y se leían desproporcionadas respecto a las letras. El aviso rojo `Found in system` baja también a `min-height: 30px` para que en la fila `System` el aviso y el botón midan lo mismo y no se descuadre la altura.
- Orden alfabético por nombre impuesto en `plugins.rs:787`. El filtro por nombre/descripción precede a la lista; cada fila usa un ícono Papirus cuando existe y hereda el scroll de la página.
- Datos disponibles: `EmuInfo` (`plugins.rs:79-87`) trae `name`, `path`, `description`, `installed`, `native`, `source`, `settings`. **No hay categoría de consola, ni versión, ni URL de proyecto, ni campo de ícono;** la UI resuelve el asset por nombre. `RegistryEntry` (registry de plugins) tampoco.

### Restricciones de datos

- La consola/generación **no viaja en los datos**: hay que derivarla del lado de la UI. Recomendado: mapa estático `tag → familia` (mismo patrón que ya usa `integration.rs:51-68` para `runner → nombre`). Alternativa rechazada en esta iteración: pedir un campo nuevo al backend (toca el plugin `emulator-manager`, fuera del alcance del repo).
- El ícono no viaja en los datos: la UI mantiene un mapa estático `nombre normalizado → archivo SVG` bajo `assets/icons/emulators/`. El fallback es un marcador tipográfico determinista (monograma de la inicial) para Azahar o un asset no disponible, en la misma geometría.

### Recomendaciones HIG de GNOME aplicadas

1. **Patrón de filas de lista (List Rows):** texto primario (nombre) + texto secundario (descripción/metadata) en dos líneas con pesos distintos, alineación izquierda, elipsis final. La fila actual viola esto al meter todo en una línea bold.
2. **Búsqueda como punto de entrada** para listas largas: `GtkSearchEntry` es ya parte de la app (busca en el sidebar y otras vistas) y el tema ya define `.search-entry` y `.filter-bar`. Para un catálogo que va a crecer, el filtro en vivo por nombre/descripción es el control HIG de entrada.
3. **Agrupación con encabezados de sección** cuando hay categorías reconocibles: en este catálogo las familias son evidentes (Nintendo vs Sony), lo que permite encabezados de grupo en lugar de una lista plana.
4. **Ícono como elemento líder**: la HIG parte de 32 px para contenido reconocible por identidad visual; esta sección escala deliberadamente el slot a 48 px para alinear la fila con las listas de contenido Prism/Modrinth (justificación en "Estilo de fila: tarjeta"). La geometría del slot es fija para no empujar el texto.
5. **Columna de acciones alineada a la derecha** con ancho consistente (140 px para el aviso `Found in system`, 120 px para `Set as linked`, 80 px para `Install`/`Remove`): ya se cumple; se conserva.
6. **Estado vacío explícito** tras un filtro sin resultados (mensaje en `.time-label`), y `hint` inicial que hoy dice solo `"No emulators reported by emulator-manager."`.
7. Virtualización (`GtkListView`) queda anotada como mejora futura para catálogos de cientos de ítems; para ~10-40 la construcción manual actual (`rebuild_emu_rows`) es aceptable y consistente con el resto de la app.

### Decisiones transversales de la fila (fijadas)

1. **Dividir el label combinado en dos líneas:** línea 1 nombre en `.details-title` (16 px bold), línea 2 descripción en `.time-label` (12 px, `text_sec`, una línea con elipsis). La refactorización queda aplicada en `rebuild_emu_rows`.
2. **Slot de ícono líder estable de 48 px, cuadrado y centrado vertical y horizontalmente:** reservado siempre. Los 11 emuladores con SVG verificado usan el asset dentro de la misma geometría; el `GtkImage` mide 40×40 y usa `Center`; Azahar o un asset ausente muestran un monograma determinista. El texto y la columna de acciones no se mueven. Tamaño elegido: 48 px; justificación en "Estilo de fila: tarjeta".
3. **Búsqueda por nombre/descripción** como control superior (`.search-entry` + `.filter-bar`, ya definidos): la opción viva es lista plana con filtro, sin agrupación.
4. **Columna de acciones alineada** y vocabulario de botones sin cambios salvo la presentación de `System`: `.add-btn`, `.emu-link-action`, `.emu-system-notice` y `.danger-btn`; ancho fijo por estado para evitar jitter.

### Opción viva: 1+3 — Botón estado-consciente + compactación

**Estructura:**

```text
[filtro de búsqueda (GtkSearchEntry, .search-entry)]
[ .emu-row-card ]
  [48px icon]   Nombre + badge            (.details-title, bold 16, 1 línea, ellipsis)
  [SVG o monograma]   Descripción — una línea    (.time-label, 12, ellipsis)
                                      [slot 12ch]  [Found in system]  [Set as linked]
```

**Descripción visual:** lista plana de tarjetas de dos líneas (decisiones transversales), con un botón cuyo **label y clase mutan con el estado de la fila** (estado-consciente) y filas resueltas **sin acción pendiente que colapsan a badge** (compactación). La píldora solo aparece donde hay acción real: pendientes, operaciones en vuelo y removibles. La columna de acciones permanece alineada con ancho fijo por estado, sin jitter.

### Estilo de fila: tarjeta tipo lista de contenido (Prism/Modrinth)

La fila adopta el patrón visual de las listas de contenido de launchers de Minecraft Java (Prism Launcher / Modrinth): tarjeta plana con borde neutro, esquinas redondeadas y fondo de panel que separa cada emulador. **Solo cambia el envase de la fila**: botones, badges, slot estable y la matriz de la opción viva 1+3 quedan intactos (sección siguiente). El dot `[•]` se elimina de esta fila por decisión del usuario (el estado usable ya se refleja en el badge de la línea 1 y el tipo de botón); las clases `emu-dot-on`/`emu-dot-off` quedan sin uso, candidatas a eliminar. No se copian los botones de la referencia (`View` / `Versions` / `Install`): esas acciones no existen en `EmuOp`; tampoco se agrega checkbox de selección (`EmuInfo` no lo tiene) ni campo author (no existe en `EmuInfo`).

**Nueva regla `.emu-row-card`** (marco de la fila):

```css
.emu-row-card {
  background-color: {panel};         /* una unidad distinta del fondo de página */
  border: 1px solid {border};        /* gris neutro, no accent */
  border-radius: 6px;                /* esquinas ~6-8 px */
  padding: 8px 10px;                 /* aire interno ~8-10 px */
  margin: 2px 0;                     /* separación entre tarjetas ~4-6 px */
}
```

Justificación de la clase nueva: ninguna clase existente modela una tarjeta de fila neutra. Reutilizar `.mc-row` (lista de contenido de Minecraft) o `.game-row` acoplaría el catálogo de emuladores al estilo de otra sección: si mañana cambia el listado de mods, el de emuladores cambiaría sin relación. Una clase localizada sigue el precedente de `.emu-link-action` (creada para el launcher en vez de reutilizar vocabulario ajeno).

**Bloque de texto (dos líneas, sin campo author):** línea 1 el nombre en `.details-title` (bold 16, single-line, ellipsize); línea 2 la descripción en `.time-label` (12, `text_sec`, single-line, ellipsize). Es la separación que hoy falta: el label actual concatena `"Nombre (badge) — descripción"` en un único `.details-title` (`settings.rs:2452-2461`). El dot de disponibilidad `[•]` **se elimina** de la fila (decisión del usuario): el estado usable ya se refleja en el badge de la línea 1 y el tipo de botón; no se reemplaza por checkbox. Las clases `emu-dot-on`/`emu-dot-off` quedan sin uso (pendiente menor: candidatas a eliminar).

**Slot de ícono (48 px, elegido):**

```css
.emu-row-icon {                       /* NUEVA */
  min-width: 48px;
  min-height: 48px;
  border-radius: 6px;                 /* eco del radio de la tarjeta */
  background-color: {well};           /* monograma como fallback */
  border: 1px solid {border};
}
```

Por qué 48 px (rango pedido 48-56): es el tamaño de las miniaturas de las listas de contenido Prism/Modrinth, la analogía de la referencia; el catálogo mueve 10-12 ítems planos con búsqueda, así que 48 px da presencia al slot sin inflar la altura de fila (56 px costaría densidad de lectura al hacer scroll); y convive con el slot estable de 12 chars: la identidad de dos líneas se lee completa y el monograma (inicial del emulador, ~18-20 px bold) queda legible como fallback para Azahar o si falta un asset. **Alineación:** el slot usa `Center` en ambos ejes y el icono mide 40×40 dentro del slot de 48 px; el bloque de texto y la columna de acciones quedan centrados verticalmente respecto a la altura de la tarjeta. **Estable ante el asset real:** los SVG verificados se sirven en la misma geometría y el layout no se mueve.

**El icono es un `GtkPicture`, no un `GtkImage` (corrección 2026-09-25).** `GtkImage:pixel-size` solo se aplica a imágenes de tipo `ICON_NAME`, así que sobre un *paintable* —como es el caso aquí, que se construye con `set_paintable`— la llamada es un no-op. Los SVG son de 48×48, así que la textura conservaba su tamaño natural completo, y `set_size_request(40, 40)` no podía encogerla porque fija un **mínimo, no un máximo**: el icono llenaba el slot y `Align::Center` se quedaba sin espacio donde actuar, que es exactamente por lo que el centrado no se notaba. `GtkPicture` con `can_shrink(true)` + `ContentFit::Contain` sí respeta los 40×40, y como el paintable es cuadrado se encaja sin deformarse.

**Matriz estado → (badge, texto del botón, clase CSS, ancho fijo):** la columna `Dot` fue **eliminada** de la matriz y de la fila por decisión del usuario: el estado usable se refleja en la píldora compacta o en la acción. Las clases `emu-dot-on`/`emu-dot-off` quedaron sin uso (pendiente menor: candidatas a eliminar).

| Estado | Badge | Texto del botón | Clase CSS | Ancho fijo |
| --- | --- | --- | --- | --- |
| `none`/empty sin instalar | — | `Install` | `.add-btn` | 80 px |
| `EmuOp` en vuelo (Installing) | — | `Installing…` | `.add-btn` + **nueva regla** `.add-btn:disabled` | 96 px |
| `appimage` instalado | — | `Remove` | `.danger-btn` | 80 px |
| `system` detectado | `System` + aviso `Found in system` (estático) | `Set as linked` | `.emu-link-action` + `.emu-system-notice` | 120 px + 140 px |
| `linked` / native | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | sin botón (compactada) | — | — |
| `EmuOp(Err)` con recuperación | — | `Retry` (o re-expuesto `Install`/`Remove` según el estado previo) | `.emu-link-action` | 96 px |

**Slot estable (12 chars, siempre presente):** durante la operación el slot muestra `Working…` (`st.set_width_chars(12)`, `settings.rs:2548`); en reposo queda vacío. `Working…` convive con la píldora o la acción y no empuja el layout. El label del botón (`Installing…`) es complementario, no duplicado.

**Clases:** se reutilizan `.add-btn`, `.danger-btn`, `.emu-link-action`, `.proton-path-badge`, `.details-title`, `.time-label`, `.search-entry`/`.filter-bar`, `.seg-*`. **Reglas nuevas ya implementadas:** `.emu-status-pill` (badge compacto), `.emu-system-notice` (label rojo estático sin affordance), `.emu-row-card` (envase de la fila) y `.emu-row-icon` (slot de ícono 48 px), las dos últimas en "Estilo de fila: tarjeta". **Regla nueva especificada pero NO implementada:** `.add-btn:disabled` para `Installing…`; queda registrada en "Pendientes de esta sección" porque afecta al catálogo de plugins, no a la fila de emuladores. La regla `.emu-link-action:disabled` sí existe y conserva el estado disabled del botón durante `Working…`.

### Apéndice: Opciones evaluadas y descartadas (trazabilidad)

- **Opción A — Fila de dos líneas con búsqueda:** su estructura quedó absorbida como base transversal de la 1+3 (dos líneas + filtro); descartada como propuesta independiente porque no define el tratamiento de estados del botón.
- **Opción B — Encabezados por familia:** descartada; requiere mapa `tag → familia` y helper nuevo (`.catalog-group`) y, con 10 ítems, el overhead vertical de encabezados no compensa.
- **Opción C — Pestañas de familia + búsqueda:** descartada; es la de más estados y sobre-ingeniería para un catálogo de 10-12; retomable solo si el catálogo crece.
- **Opción 2 — Acción imperativa estable + estado en slot/badge:** descartada; cumple estrictamente "badge=estado, botón=acción" pero conserva el muro de `Install` en los pendientes, síntoma que 1+3 corrige.
- **Opción 3 — Compactación (sola):** descartada como opción única; su técnica (colapsar filas resueltas a badge) queda integrada en 1+3.

**Comparación (referencia):**

| Criterio | A: Dos líneas + búsqueda | B: Encabezados familia | C: Pestañas + búsqueda | 1+3 (viva) |
| --- | --- | --- | --- | --- |
| Corrección de jerarquía | alta | alta | alta | alta |
| Contexto de consola | nulo | alto | alto | nulo (mitigado por búsqueda) |
| Muro de `Install` | persiste | persiste | persiste | eliminado (labels + compactación) |
| Datos nuevos necesarios | ninguno | mapa `tag→familia` | mapa `tag→familia` | ninguno |
| Clases nuevas de tema | ninguna | 1 helper (`.catalog-group`) | ninguna | 3 reglas nuevas (`.emu-row-card`, `.emu-row-icon`, `.add-btn:disabled`) |

### Pendientes de esta sección (abiertos)

- **Asset real del ícono:** completado para los 11 emuladores con SVG verificado en Papirus y el logo oficial de Azahar (CC BY 4.0) está descargado y atribuido en `assets/icons/ATTRIBUTION.md`. El slot de 48 px (`.emu-row-icon`) permanece estable; el resolver sigue usando el monograma para Azahar hasta una decisión de activación separada.
- **Clases `emu-dot-on` / `emu-dot-off` huérfanas (candidatas a eliminar)**: tras la decisión del usuario de eliminar el dot `[•]` de la fila, las clases quedaron definidas en `helpers.rs:553-554` pero sin uso en código, y la mención del punto de disponibilidad en la "Base tipográfica" (sección de `Set as linked`) quedó obsoleta. No se borran todavía: como su folclore de contraste (verde `#00E639` / `text_muted`) es compartido, se marcan como candidatas a eliminar en una limpieza futura.
- **Mapa `tag → familia`**: **no aplica a la opción viva** (lista plana con filtro, sin agrupación). Permanece abierto únicamente si el catálogo crece y se retoma B o C del apéndice.
- **Hueco de estado vacío**: el hint actual de la lista (`"No emulators reported by emulator-manager."`) no distingue "sin plugin reportado" de "sin resultados de filtro"; definir un estado vacío explícito para cada caso (la opción viva incluye búsqueda, así que "sin resultados de filtro" es un estado alcanzable).
- **`.add-btn:disabled` sin implementar**: la matriz de esta sección especifica que `Installing…` use `.add-btn` + `.add-btn:disabled` con 96 px, pero `helpers.rs` solo define `.add-btn` y `.add-btn:hover`. Hoy un `Install` deshabilitado durante la instalación cae al estilo default de GTK y rompe la consistencia de la cápsula accent. La regla especificada sería la misma que `.emu-link-action:disabled` (relleno accent al 8%, borde y texto `{text_muted}`). No se implementa en esta iteración: toca el catálogo de plugins y su verificación es visual.
- **Anchos de 96 px sin implementar**: la matriz reserva 96 px para `Installing…` y para `Retry` en recuperación de error, pero `rebuild_emu_rows` fija 80 px para `Install`/`Remove` y 120 px para el pin. Hay que decidir el ancho real de los estados transitorios para que el texto no se recorte y la columna no salte; queda pendiente de la matriz visual.

## Pendientes registrados (fuera de alcance)

- **Contraste del badge `System`/`Linked`:** el mismo riesgo que resolvió la sección "Contraste 3:1" existe preexistente en el vocabulario de estado: el texto y el borde del badge usan `{accent}` y, con acentos claros en tema claro, caen por debajo de 3:1 (amarillo oscurecido 22%: 2.01:1 contra `#FFFFFF`). No se corrige en esta iteración; queda registrado para una tarea futura de contraste del vocabulario de badges.
- **Botón Wine:** pendiente la captura del usuario y la confirmación del tema (claro/oscuro) para diagnosticar el defecto visual reportado; las causas de código ya fueron descartadas.
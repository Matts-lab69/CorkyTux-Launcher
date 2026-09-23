# PENDIENTES.md

Estado de pendientes y hallazgos. Regla: nada se declara "resuelto" sin
evidencia; los ítems que requieren ver la app quedan en espera del usuario
(no se ejecuta la app desde el agente).

## Resuelto (con evidencia)

### Descripciones vacías en el modal "View" (Library/Stores)
Commit: `50dfa79`
- Probe `examples/game_info_probe.rs` invocó `game-info` del plugin
  heroic-store para los **21 juegos** del cache legendary y cruzó el
  resultado contra la metadata local. Conclusión: **no hay bug de
  pipeline**. Los juegos con descripción real (Mindcop, Voidwrought,
  Genshin, ZZZ, Marvel Rivals, Astral Ascent, Honkai Star Rail,
  River City Girls 2, Fortnite…) llegan al modal con texto.
- Los que llegan vacíos son "stub" en origen: la metadata de legendary
  tiene `description == título` (Fall Guys, VALORANT, DOOMBLADE, Rocket
  League, Shogun Showdown, Luftrausers, RCT3, LEGO Fortnite content,
  I Have No Mouth). El modal borraba `desc==title` y no mostraba nada.
- Fix UX: si la descripción queda vacía se muestra el label atenuado
  "Sin descripción disponible" (`opacity 0.6`, clase `time-label`), con
  buen contraste en tema claro y oscuro. Se retiró el TEMP-LOG de
  `game_info`.

### Warnings GTK "No property named: max-width/max-height"
Commits: `05bc3ab`
- Evidencia exacta en `/tmp/corkytux_run*.log`:
  `Theme parser error: <data>:1:5602-5611: No property named "max-width"`.
- GTK4 CSS no define `max-width`/`max-height`. El techo real lo pone
  `card.set_size_request(CARD_W, CARD_H)` de `game_card.rs` más los
  `min-*` del selector `.game-card` (`helpers.rs`). Se eliminaron las dos
  propiedades residuales (`effeb9c`). Verificado en arranque: PID 25598,
  `/tmp/corkytux_run4.log`, cero warnings de parser CSS.

### Sistema de íconos hicolor
Commits: `5101d64` (+ `2f8119c`, `fd8b888` como probes de soporte)
- Única vía real de bundle: fallback universal hicolor con la ruta del
  bundle en primera posición del `search_path` (`main.rs`). Elementos
  del árbol CorkyTux residual eliminados de repo y runtime.
- La resolución efectiva por nombre quedó demostrada con
  `examples/icon_resolve_probe.rs` (replica `for_display` + prefijo de
  ruta): con el tema activo Mint-Breeze-Calm-Green la mayoría de los
  nombres resuelven del tema de sistema; solo 11 caen en nuestro bundle
  por fallback (epicgames, gogdotcom, applications-games,
  applications-engineering, alarm, display-brightness, non-starred,
  application-x-addon, image-x-generic, package-x-generic,
  text-x-generic), y de esos **9 son byte-idénticos a Adwaita** (cero
  cambio respecto a antes) y `epicgames`/`gogdotcom` son los brand
  custom que antes fallaban (mejora intencionada).
- (Actualizado por T3: los 26 nombres estándar pasaron a `corkytux-*` y
  ahora **37/37 resuelven al bundle**; ver sección "Independencia de
  íconos del tema del sistema" más abajo.)

### Descripción en cascada con versión del catálogo Epic (T1)
Commit: `5419224`
- `legendary list --json` NO expone `app_version`; sí trae
  `asset_infos.{Windows|Other|Mac|Linux}.build_version` y
  `metadata.lastModifiedDate`. El plugin usa ahora la búsqueda de la plataforma
  correcta y emite `version` y `last_updated` (`YYYY-MM-DD`) en `game-info`.
  Verificado en directo: Sugar=`++Prime+Update60-CL-528314`/2026-09-02,
  Fall Guys=`EGS_11958`/2024-08-16, DOOMBLADE=`1.2`/2023-05-30,
  VALORANT=`2609-591`/2026-09-16.
- Modal: `ver` se muestra solo con descripción real; si falta,
  cascada: `Última versión: {ver} (última actualización conocida por Epic: {fecha})`
  → `Última versión: {ver} (dato del catálogo de Epic)` → `Sin descripción disponible`.
  Se aclara que la fecha es del catálogo de Epic, no de la instalación local.

### Descripción truncada con ellipsis en el modal View (T2)
Commit: `d2b5d50`
- Causa raíz: el label usaba `set_lines(6)` + `ellipsize End` (recorta el texto)
  y el "Read more" existente se disparaba con `desc.lines().count() > 6`, que
  cuenta saltos de línea, no líneas visuales: un párrafo largo único (Fortnite,
  766 chars) **jamás** activaba el expander → texto irrecuperable.
- Fix: `ScrolledWindow` vertical (policy Automatic, `max_content_height` 200,
  vexpand) con el label wrap completo y SIN ellipsis. GTK mide el tamaño natural
  del label y decide el scrollbar; el diálogo queda acotado. Sin umbral de
  caracteres hardcodeado (robusto a fuente/ancho). Se eliminó el Expander.
- Control: Mindcop (283 chars) no genera scroll; Fortnite (766) sí.

### Independencia de íconos del tema del sistema (T3)
Commit: `5e4f2dc`
- 26 nombres estándar del bundle eran ganados por el tema activo
  (Mint-Breeze-Calm-Green). Se renombran a `corkytux-<nombre>` (git mv) y se
  actualizan las **53** referencias en `src/` (`minecraft_view.rs` 40,
  `details_panel.rs` 5, `sidebar.rs` 4, `settings.rs` 2, `stores_view.rs` 1,
  `game_card.rs` 1). Verificado: 0 referencias colgadas.
- Evidencia `icon_resolve_probe` (mismo probe dinámico ANTES/DESPUÉS):
  - ANTES: 11/37 bundle; 26/37 tema del sistema. (lista completa guardada en
    `/tmp/icon_probe_before.txt`)
  - DESPUÉS: **37/37 bundle** (`/tmp/icon_probe_after.txt`).
- Bundle del runtime sincronizado: `diff -rq` repo == runtime.
- `icon_resolve_probe` ahora escanea los SVGs del bundle (lista dinámica).

### Regresión BUG A — descripción larga se cortaba en el modal View
Commit: `c3b2592`
- Síntoma: Marvel Rivals / Genshin / Fortnite cortaban el texto a ~2 líneas,
  sin scrollbar. Diagnóstico: un `GtkLabel` dentro de `GtkScrolledWindow` **no
  puede desplazarse en vertical**: su altura natural se mide a su ancho natural
  (línea sin envolver, ~2 líneas), el viewport colapsa a ese mínimo y el texto
  se recorta sin barras. Esto anulaba el fix de `d2b5d50`.
- Fix: `GtkTextView` read-only (wrap WordChar, editable/cursor/focus off, clase
  `time-label`) que sí reporta su altura envuelta real; el viewport mide
  `min(alto completo, max_content_height=200)` y GTK decide la barra por altura
  natural, sin umbrales. Cascada T1 y opacidad de fallback conservadas.

### Regresión BUG B — ícono del store se veía como candado
Commit: `64d1dcd`
- El botón Stores pedía desde siempre `system-software-install-symbolic`
  (da1d4ab). Pre-T3 el tema Mint ganaba el lookup (glyph moderno caja+flecha);
  tras prefijar a `corkytux-`, el bundle resolvía primero y exponía la copia
  legacy de Adwaita, que es un **candado** (rectángulo `M3 8h10v7.059…` +
  grillete `M6.793 2.969…`). La referencia NO se renombró mal; el asset era el
  equivocado.
- Fix: ese SVG se sustituye por el trazo del `system-software-install` moderno
  de Mint-Breeze, normalizado a `fill #2e3436` (convención del bundle) y
  documentado en `ATTRIBUTION.md`. Probe: `corkytux-system-software-install`
  → bundle (37/37, sin cambio de resolución).

### Regresión scroll del modal (viewport de 24px) + banner perdido
Commits: `361d6c8` (asunto a) y `6b6ed24` (asunto b)
- Diagnóstico con medidas (TEMP-LOG en `show_game_info`, post-map):
  - `scr` `measure(Vertical)` = **24px** con `min_content_height=-1` y
    `propagates_natural_height=false` → el viewport se alojaba a 24px (texto
    cortado a "2 líneas", barra overlay invisible sin scroll).
  - `top` (portada + título) **`allocH=0`**: `git log -S` demostró que
    `d2b5d50` eliminó `body.append(&top)` → la fila quedó huérfana y el banner
    desapareció del modal (no se borró el código de la portada).
  - `tv` (TextView) sí reportaba su altura envuelta real; el cuello de botella
    era el ScrolledWindow sin tamaño mínimo ni propagación.
- Fix (a) `361d6c8`: `scr` con `min_content_height(120)`, `max(300)`,
  `propagate_natural_height(true)`, `vexpand(true)`; CSS
  `textview`/`textview > text` transparentes en `.modal-bg` (elimina franja
  oscura). Fix (b) `6b6ed24`: restaura `body.append(&top)`.
- Verificación medida (re-run con el fix): desc corta → `tv` 16px, viewport
  120; desc larga real (Marvel Rivals) → `tv` natural 195px **propagado** por
  el `scr` (texto completo sin scroll porque cabe); desc >300 capa en 300 con
  scroll; `top` `allocH=160` (banner visible); `body` 334/362, `content` 421.

## Hallazgos nuevos (anotados, NO arreglados)

### DLC de Fortnite fallan en el modal "View"
- Los add-ons "Contenido de LEGO® Fortnite" (`94bc5ec13f8f438c97fdbef3e9019e27`)
  y "Contenido de Salva el mundo de Fortnite" (`aa31f9e94e844b299ca757d1d0b97a09`)
  dan error `{"type":"error","message":"Game not in Epic library"}` en
  `game-info`. Evidencia: salida real del plugin (ver arriba).
- Aparecerán con descripción vacía en el modal (y ahora con el fallback
  de UX). No está en la lista de pendientes → se anota, no se toca.

### `legacy/system-software-install-symbolic.svg` y `legacy/web-browser-symbolic.svg` sin viewBox
- Con el tema activo (Mint-Breeze) no se usan: ambos resuelven del tema
  de sistema (`apps/symbolic/...`), no de nuestro bundle. En el bundle
  son byte-idénticos a `Adwaita/symbolic/legacy/` (mismo archivo que
  Adwaita distribuye y renderiza bien). No se tocan.
- Si algún día superan al tema activo (p.ej. otro tema sin esos nombres)
  renderizan igual que Adwaita: aceptable.

## En espera de verificación visual (usuario abre la app)

1. **Sweep final de íconos** (Library, Minecraft, Settings, Integrations,
   sidebar): tras T3 (37/37 al bundle via probe) confirmar visualmente que
   el trazo/sombreado de los 26 íconos ahora propios se ve correcto
   (p.ej. `emblem-ok` del check de Minecraft pasa a la copia del bundle).
2. **Cascada de descripción (T1)**: abrir un juego stub (Fall Guys o
   VALORANT) y comprobar el texto "Última versión: … (última actualización
   conocida por Epic: …)" atenuado y bien legible en claro y oscuro.
   Para un juego SIN metadata de Epic: "Sin descripción disponible".
3. **Scroll del modal View (T2 + BUG A + 361d6c8)**: Mindcop (283 chars) sin
   scrollbar; Marvel Rivals / Genshin / Fortnite (largas) con la descripción
   completa (hasta ~300px de viewport, scroll SMOOTH en la cola) y **portada +
   título visibles arriba** (6b6ed24). Diálogo no debe exceder ~300px de
   descripción.
4. **Ícono del store (BUG B)**: en la sidebar, el botón junto a "Your Library"
   debe volver al glyph de instalación moderna (caja+flecha), no candado.
4. Re-render de iconos tras cualquier cambio de tema/íconos en el futuro.
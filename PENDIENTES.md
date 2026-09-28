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

## Auditoría multi-distro (6 críticos; C14 ya corregido)

Detalle completo, con `archivo:línea` y fix mínimo, en
[`docs/multidistro-audit.md`](docs/multidistro-audit.md) (42 hallazgos del
triage: 6 críticos, 30 degradantes, 6 cosméticos). Auditoría estática: no se
compiló ni ejecutó la app. Numeración heredada del triage original.
**Críticos: los 6 corregidos.** C08 a medias (3 de 12 externalidades).

### ~~C14 — `prefix_in_use` falla abierta sin `pgrep`~~ — **CORREGIDO**
Era el único hallazgo con riesgo de pérdida de datos: sin `pgrep` (NixOS,
contenedores) el prefix se declaraba **libre** y el import permanente lo movía
con el juego corriendo.

- **Fix aplicado:** `prefix_usage() -> PrefixUsage { Busy, Free, Unknown }`
  en vez del booleano. `wineserver_pids()` lee `/proc/*/comm` y
  `wineprefix_of(pid)` lee `WINEPREFIX` de `/proc/<pid>/environ`, que es la
  atribución exacta (wineserver nunca lleva el prefix en su línea de comandos,
  así que el substring sobre `pgrep -af` no era fiable). Ambos lados pasan por
  `norm()`.
- **Falla cerrada:** si hay wineservers vivos cuyo entorno es ilegible (otro
  usuario, procfs con `hidepid`), sale `Unknown` y se bloquea con el nuevo
  `Blocker::UsageUndeterminable`, en vez de arriesgar los datos. La UI lo
  muestra sin cambios: `import_manager.rs` usa `b.message()` de forma genérica.
- **Efecto colateral bueno:** desaparecen 2 de las 12 llamadas a `pgrep` de C08.
- **Efecto secundario:** un `wineserver.lock` obsoleto ya no bloquea el import
  para siempre; el estado se decide por procesos vivos, no por el socket.
- `wineserver_pids()`/`wineprefix_of()` quedan privados al módulo;
  `prefix_in_use` se elimina (sus dos únicos llamadores están en el mismo
  archivo).

### C08 — Dependencia dura de binarios externos — **PARCIALMENTE CORREGIDO**
**Hecho: los 8 usos de `timeout` y el único de `ldconfig`.** Quedan 4 sitios
de `pgrep`, `pidof` y `which`.

- **Fix aplicado:** `plugin_process::output_with_timeout()` (`spawn` +
  `try_wait` en bucle de 25 ms contra un `Instant` de corte, `kill` + `wait`
  al agotarlo). Cubre `plugins.rs:552,620,690,727`,
  `integration.rs:601,609,666` y `proton.rs:1314`. Sin binarios externos.
- Los streams se leen en hilos propios, para que un plugin que llene el buffer
  del pipe no se bloquee esperando su salida, y se unen **con margen de 2 s**:
  si el plugin dejó nietos con el pipe abierto, el `join` directo habría
  colgado la interfaz, cosa que `timeout(1)` + `.output()` tampoco cubría.
- `list_emulators_in` sigue devolviendo lista vacía (contrato de sus 4
  llamadores; cambiarlo tocaría la UI) pero ahora registra el motivo en stderr
  como `[emu] corky-list …`, así un fallo ya es diagnosticable.
- Efecto colateral: `wineserver -k` en `proton.rs` deja de perder
  `ws.to_str()`, así que un prefix no-UTF8 ya no degrada a programa vacío.
- **Fix pendiente:** `pgrep` (`proton.rs:261`), `pidof` (`proton.rs:825`,
  sustituible por `/proc`) y `which` (`integration.rs:27`,
  `proton.rs:597,822`, sustituible por una búsqueda en `PATH`).

### C01 — Rutas de Steam hardcodeadas: overlay roto en Flatpak/Snap — **CORREGIDO**
`steam_client_path()` y los dos resolvers del overlay usaban solo
`~/.steam/steam`, mientras `find_steam_runtime` sí listaba las cuatro raíces.
Con Steam Flatpak o Snap, el toggle del overlay quedaba activo y no inyectaba
nada, sin aviso.

- **Fix aplicado:** `steam_roots(home)` es la única lista (las cuatro
  variantes) y la consumen `steam_client_path_for()`,
  `find_steam_runtime` y `steam_overlay_preload()`.
- `~/.steam/steam` va primero porque es la ruta que ya usaba
  `STEAM_COMPAT_CLIENT_INSTALL_PATH` y, en una instalación normal, es un enlace
  a `.local/share/Steam`: no cambia el comportamiento de nadie. Si ninguna raíz
  existe, el fallback sigue siendo `~/.steam/steam` (Proton usa la variable
  aunque no haya Steam).
- `steam_overlay_preload()` sustituye a los dos bloques duplicados (rumbo y
  no-rumbo), recorre todas las raíces y conserva el `':'` inicial que no pisa
  un `LD_PRELOAD` heredado.
- **Hardcode extra que salió al implementarlo:** `legendary_launch_cmd` fijaba
  `STEAM_COMPAT_CLIENT_INSTALL_PATH` por su cuenta, así que arreglar solo
  `steam_client_path()` no cubría el lanzamiento vía Heroic. Ahora usa
  `steam_client_path_for()`, y la lista queda en un único sitio.

### C03 — `component_status` solo reconoce tokens x86 y sondea dos binarios de GameMode — **CORREGIDO**
Decisiones tomadas por el usuario: derivar de `ARCH` + rutas del sistema, y
sondear siempre `gamemoderun`.

- `component_bits(lib)` deriva el soporte de `std::env::consts::ARCH`. En
  aarch64 `installed32` es `false` **por construcción**: no hay distros ARM de
  32 bits con las que contar, y fingir lo contrario sería peor que no medirlo.
- `library_present(lib, bits)` busca por prefijo de nombre base
  (`libMangoHud.so` cubre `libMangoHud.so.1.2`) en el multiarch, `/usr/lib`,
  `/usr/lib64`, `/usr/local/lib`, `/lib`, `lib32`/`libx32` para 32 bits y
  `/run/current-system/sw/lib` para NixOS. El multiarch lo dice
  `gcc -print-multiarch`, con `$MULTIARCH` y `<arch>-linux-gnu` de fallback.
- **Defecto corregido que era independiente de la arquitectura:**
  `installed64` nacía en `available`, así que un `mangohud` sin su biblioteca de
  64 bits se anunciaba como instalado. Los flags solo podían activarse; ahora
  pueden ser negativos.
- Se eliminó la dependencia de `ldconfig`: era el último sitio de ese binario.
- `graphics_component_status()` ahora sondea `gamemoderun` como
  `component_status()`. Se unificó en el cliente porque es lo que CorkyTux
  inyecta al lanzar, y que el daemon esté instalado no significa que GameMode
  se aplique.
- `component_status_text()` no se tocó: el contrato de las etiquetas de la UI es
  idéntico.

### C09 — El registro de plugins toma el primer `.tar.gz` sin filtrar por arch — **CORREGIDO**
- **Severidad real, verificada contra la API el 2026-09-25:** los cinco
  releases publicados usan nombres sin arquitectura
  (`heroic-store-1.0.8.tar.gz`, `minecraft-launcher-1.1.0.tar.gz`,
  `emulator-manager-1.1.0.tar.gz`, `dependency-installer-2.1.3.tar.gz`,
  `heroic-store-1.0.7.tar.gz`), así que el fallo es **latente**: hace falta que
  alguien publique un par de assets por arquitectura. El síntoma (instalación
  correcta y `Exec format error` en cada uso) sería de los más caros de
  diagnosticar, por eso se corrigió igualmente.
- **Fix aplicado:** `asset_arch_ok()` acepta assets sin token de arquitectura
  (los actuales) y, si el token existe, exige que mapee a
  `std::env::consts::ARCH` con los alias habituales: `x86_64`/`amd64`/`x64`,
  `aarch64`/`arm64`, `i386`–`i686`/`x86`, `arm`/`armv7l`/`armhf`,
  `ppc64`/`ppc64le`, `riscv64`/`s390x`. Los tokens se separan por `-` y `.`,
  nunca por `_`, porque `x86_64` lleva guion bajo y `x86` colisionaría.
- `fetch_registry()` recorre todos los `.tar.gz` en vez de romper en el
  primero, y **omite** un release cuyos assets son todos de otra arquitectura
  (motivo en stderr) en vez de ofrecer una descarga imposible.

### C10 — El escaneo de Lutris ignora `XDG_DATA_HOME` y Lutris Flatpak — **CORREGIDO**
El artwork (450 líneas antes en el mismo archivo) sí honraba `XDG_DATA_HOME` y
la ruta Flatpak, pero el escaneo de juegos usaba `~/.local/share/lutris` fijo.
Consecuencia: con `XDG_DATA_HOME` puesto se veían las carátulas y se importaban
0 juegos; con Lutris Flatpak, lo mismo.

- **Fix aplicado:** `lutris_data_roots(home)` devuelve las raíces de datos en
  orden de preferencia — `$XDG_DATA_HOME` si es absoluto y no vacío (con
  `~/.local/share` como default del spec) y
  `~/.var/app/net.lutris.Lutris/data` (home del sandbox Flatpak) — y lo
  consumen tanto el escaneo (`pga.db` y `games/`) como el artwork
  (`coverart`/`banners` e `icons/hicolor/128x128/apps`).
- **Hardcode extra que salió al implementarlo:** `lutris_yml_info()` volvía a
  construir `~/.local/share/lutris/games` por su cuenta, así que arreglar solo
  `scan_lutris` no bastaba. Ahora recibe el `games_dir` del llamador, y la capa
  SQLite lo deriva de `db.parent()`.
- `scan_lutris` deduplica por slug entre raíces: un juego no aparece dos veces
  durante una migración a Flatpak a medias.

## Pendiente separado: carpetas por tienda (Epic-Games / GOG-Games)

- `~/Games/Heroic` es convención de Heroic: no usarla como base propia.
  Nuevos installs: `~/Games/Epic-Games` y `~/Games/GOG-Games` (el launcher
  crea si falta). Condiciones: launcher aparte del plugin; el guard de
  Remove sigue negando `~/Games/Heroic` (legacy) además de las nuevas
  bases y `~/Games`; un único origen del default; Bloody Hell se queda
  donde está (sin reinstalar).

## Hallazgos nuevos (anotados, NO arreglados)

### Botón Stop no detiene juegos lanzados fuera del launcher
- Stop solo controla procesos hijo del launcher. Un juego abierto a mano
  (o por otro launcher) no se detiene desde aquí. Documentar en UI si
  hace falta; no cambiar comportamiento sin diseño.

### Lector duplicado de installs.json en remove_modal.rs
- `remove_modal.rs` lee `plugins/heroic-store/installs.json` con lector
  propio en vez de reutilizar el del backend. Unificar cuando se toque
  ese archivo (un solo origen de lectura).

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
5. **Ciclo logout → re-login de Epic y GOG**: el login quedó confirmado por
   UI en las dos tiendas (`logged: epic/gog = true`, token de Epic escrito en
   `~/.config/legendary/user.json`, token de GOG en `gogdl_auth.json`), pero
   **nadie probó desloguearse y volver a entrar**. Sin comprobar que
   `logout` invalida el token y que un `Log in` posterior vuelve a canjear
   sin dejar restos del intento anterior.
6. **Login de GOG de principio a fin**: el canje y el refresh se corrigieron y
   se verificaron contra un token ya almacenado, pero el intento completo
   (navegar → capturar → canjear → verificar) se confirmó solo tras reintentar
   con el token del intento anterior, no en una única pasada limpia.
7. **Tamaño final de los iconos de acción y el ciclo Play↔Stop** en tema
   claro y oscuro: el fix de escalado y de tinta está aplicado pero sin
   verificación visual.
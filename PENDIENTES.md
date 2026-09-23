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
   sidebar): la evidencia derivada indica cero cambio de trazo respecto a
   antes (9 fallback byte-iguales a Adwaita + brands custom que antes
   fallaban), pero la confirmación visual es del usuario.
2. **Fallback "Sin descripción disponible"**: comprobar contraste y
   tipografía en tema claro y oscuro dentro del modal (abrir un juego
   stub, p.ej. Fall Guys o VALORANT).
3. Re-render de iconos tras cualquier cambio de tema/íconos en el futuro.
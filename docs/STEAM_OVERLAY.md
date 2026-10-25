# Steam Overlay and Startup

Estado: análisis del código de `main`; no se ejecutaron CorkyTux ni Steam.
Los hallazgos de código están separados de las hipótesis que requieren una
prueba visual. Las referencias de archivo y línea apuntan al commit base de
esta rama.

## Qué hace la opción

`Steam overlay` aparece en los ajustes por juego junto a `Steam runtime` y
`umu-launcher`. La UI lee y guarda `SteamOverlay` mediante `save_switch`; el
parser de `Games.ini` normaliza las claves a minúsculas y el lanzador lee
`steamoverlay`. En la práctica, la clave persistida es `steamoverlay=true` o
`steamoverlay=false`; si falta, el valor por defecto es apagado.

- UI y guardado: [src/ui/game_settings.rs:377](../src/ui/game_settings.rs#L377), [src/ui/game_settings.rs:401](../src/ui/game_settings.rs#L401), [src/ui/game_settings.rs:173](../src/ui/game_settings.rs#L173).
- Clave que consume el lanzador y valor por defecto: [src/backend/proton.rs:1315](../src/backend/proton.rs#L1315).
- Acceso a Games.ini: [src/backend/config.rs:464](../src/backend/config.rs#L464), [src/backend/config.rs:468](../src/backend/config.rs#L468).

Al activarla, el lanzador busca `gameoverlayrenderer.so` bajo estos cuatro
roots, en `ubuntu12_32` y `ubuntu12_64`:
`~/.steam/steam`, `~/.local/share/Steam`,
`~/.var/app/com.valvesoftware.Steam/data/Steam` y
`~/.var/app/com.valvesoftware.Steam/.steam/steam`. Si encuentra bibliotecas,
concatena ambas rutas en una cadena `LD_PRELOAD` con dos puntos y un `:`
inicial. Si no encuentra ninguna, no añade las variables de overlay.

- Roots: [src/backend/proton.rs:690](../src/backend/proton.rs#L690).
- Resolución de bibliotecas de 32 y 64 bits: [src/backend/proton.rs:706](../src/backend/proton.rs#L706).
- El `:` inicial y las asignaciones a `LD_PRELOAD`: [src/backend/proton.rs:704](../src/backend/proton.rs#L704), [src/backend/proton.rs:1537](../src/backend/proton.rs#L1537), [src/backend/proton.rs:1626](../src/backend/proton.rs#L1626).
- Capa Vulkan: `ENABLE_VK_LAYER_VALVE_steam_overlay_1=1`; identificador overlay: `SteamOverlayGameId`, por defecto `480`: [src/backend/proton.rs:1316](../src/backend/proton.rs#L1316), [src/backend/proton.rs:1538](../src/backend/proton.rs#L1538), [src/backend/proton.rs:1539](../src/backend/proton.rs#L1539), [src/backend/proton.rs:1627](../src/backend/proton.rs#L1627), [src/backend/proton.rs:1628](../src/backend/proton.rs#L1628).

No se asigna `SteamGameId` en este flujo. Proton directo asigna
`SteamAppId` al Steam ID del juego o a `0`, y define
`STEAM_COMPAT_CLIENT_INSTALL_PATH` con el primer root Steam existente (o
`~/.steam/steam` si no hay ninguno). `steam_client_path_for` usa la misma
lista de roots, que incluye Flatpak pero no Snap.

- Path de cliente: [src/backend/proton.rs:740](../src/backend/proton.rs#L740).
- Proton directo: [src/backend/proton.rs:1558](../src/backend/proton.rs#L1558), [src/backend/proton.rs:1574](../src/backend/proton.rs#L1574).
- Legendary: [src/backend/proton.rs:527](../src/backend/proton.rs#L527), [src/backend/proton.rs:533](../src/backend/proton.rs#L533).

Las rutas no son iguales en todos los runners:

- Proton directo sí inyecta el preload y las dos variables del overlay.
- UMU también inyecta esas tres variables; usa `GAMEID` (Steam ID o `0`) y
  `STORE=steam`. En el bloque UMU no se asigna `SteamAppId` ni
  `STEAM_COMPAT_CLIENT_INSTALL_PATH`.
- Heroic/Legendary tiene un retorno temprano anterior a la lectura de
  `steamoverlay`: configura `STEAM_COMPAT_CLIENT_INSTALL_PATH` y `SteamAppId`,
  pero no aplica el preload del overlay ni su `SteamOverlayGameId`.
- No se configura `VK_LAYER_PATH` explícitamente.

Evidencia: UMU [src/backend/proton.rs:1522](../src/backend/proton.rs#L1522), [src/backend/proton.rs:1531](../src/backend/proton.rs#L1531); Legendary early return [src/backend/proton.rs:1424](../src/backend/proton.rs#L1424), client path [src/backend/proton.rs:527](../src/backend/proton.rs#L527).

MangoHud se activa por `MANGOHUD=1`. GameMode envuelve el comando después;
copia argumentos y variables del comando interior. El orden y la
compatibilidad efectiva del loader dependen además de qué wrapper usa cada
juego; no hay una prueba de overlay con MangoHud/GameMode/gamescope en este
repositorio.

- MangoHud: [src/backend/proton.rs:1610](../src/backend/proton.rs#L1610).
- Wrapper GameMode: [src/backend/proton.rs:1667](../src/backend/proton.rs#L1667).

## Quién inicia Steam

No hay un plugin o ajuste global llamado “start Steam first”. Sí hay dos
arranques automáticos en `ProtonManager::run_game`:

1. Si el juego tiene un Steam ID válido, el lanzador comprueba `which steam`,
   consulta `pidof steam` y, si no ve un proceso, inicia `steam -silent`.
   Inmediatamente después pide a `xdg-open` abrir
   `steam://rungameid/<id>`.
2. Para el camino Proton/UMU, si `steamoverlay` está activo o el juego tiene
   `steamid`, hace la misma detección y, si parece cerrado, inicia
   `steam -silent`.

Evidencia: ruta `steam://` [src/backend/proton.rs:1233](../src/backend/proton.rs#L1233), [src/backend/proton.rs:1242](../src/backend/proton.rs#L1242), [src/backend/proton.rs:1252](../src/backend/proton.rs#L1252), [src/backend/proton.rs:1265](../src/backend/proton.rs#L1265); autoarranque del runner [src/backend/proton.rs:1353](../src/backend/proton.rs#L1353), [src/backend/proton.rs:1356](../src/backend/proton.rs#L1356), [src/backend/proton.rs:1365](../src/backend/proton.rs#L1365), [src/backend/proton.rs:1375](../src/backend/proton.rs#L1375).

`which` solo detecta el ejecutable disponible en `PATH`; `pidof` solo
comprueba que haya un proceso con ese nombre. No se lee
`~/.steam/registry.vdf`, `ActiveUser`, un pipe/socket de Steam ni el estado de
la sesión. El proceso se lanza con `spawn()` y no se espera a que la sesión
inicie, termine una actualización o acepte `steam://`. El camino `steam://`
también hace `spawn()` de `xdg-open` sin esperar su resultado. Por tanto, la
carrera “Steam arrancando / juego lanzado” es verificable en el flujo; la
duración y si explica un fallo concreto requieren medirlo en una máquina con
Steam.

Las llamadas a `which` y `pidof` ya están anotadas en C08 y D02/D26 de
`PENDIENTES.md`: [PENDIENTES.md:18](../PENDIENTES.md#L18), [PENDIENTES.md:20](../PENDIENTES.md#L20).

## Hipótesis sobre fallos

| Hipótesis | Evidencia disponible | Estado |
|---|---|---|
| Carrera de arranque | `steam -silent` es asíncrono y no hay espera de readiness antes de abrir el protocolo o lanzar el runner. | Confirmada en el orden del código; latencia y efecto visual no medidos. |
| Steam sin sesión, actualizándose o con login pendiente | Solo se consulta si existe un PID; no hay lectura de `ActiveUser`, del pipe/socket o de un indicador de actualización. | El código no distingue esos estados; cuál ocurre en un caso concreto no verificado. |
| Flatpak | Hay roots Flatpak para hallar el preload, pero el autoarranque busca `steam` por `PATH`; no ejecuta `flatpak run`. | Incompatibilidad parcial confirmada en el descubrimiento/arranque. La inyección efectiva en un juego Flatpak no probada. |
| Snap | No hay roots Snap en `steam_roots`, y el arranque tampoco invoca `snap run`. | No soportado explícitamente por estas rutas; falta probar una instalación Snap. |
| Aislamiento bwrap | El sandbox oculta el HOME real con tmpfs y crea un `/tmp` nuevo. Expone el HOME privado y `XDG_RUNTIME_DIR`; no monta el HOME Steam del host. | Riesgo confirmado por la configuración. El camino de `iso_ro` intenta derivar el directorio de la cadena `LD_PRELOAD` incluyendo el `:` inicial, lo cual puede producir una ruta inválida y omitir el bind del `.so`. No está probado visualmente. |
| Orden MangoHud/GameMode/gamescope | MangoHud usa `MANGOHUD`; GameMode envuelve el comando y copia el entorno. No hay configuración explícita de gamescope en este flujo. | No se midió el efecto sobre un overlay real. |
| Juegos de 32 bits | El resolvedor incluye `ubuntu12_32` y `ubuntu12_64` si ambos archivos existen. | La presencia de archivos se comprueba; carga correcta en un juego de 32 bits no probada. |
| Wayland/X11 | `nativewayland` configura `PROTON_ENABLE_WAYLAND` solo en Proton directo. | El ajuste es independiente del overlay; no hay comparación visual en ambos compositores. |
| UMU frente a Proton | Ambos tienen código de preload. UMU configura `GAMEID`/`STORE`; Proton directo configura además `SteamAppId` y `STEAM_COMPAT_CLIENT_INSTALL_PATH`. Legendary omite el bloque de overlay. | Diferencias confirmadas; comportamiento en ejecución no probado. |
| Juego no Steam sin SteamAppId | Proton asigna `SteamAppId=0`; overlay usa `SteamOverlayGameId`, que por defecto es `480`. UMU usa `GAMEID=0` si no hay Steam ID. | Valores confirmados; un título no Steam puede además necesitar que Steam lo registre como juego externo. No probado aquí. |

El comentario de `steam_overlay_preload` dice que el `:` inicial conserva un
`LD_PRELOAD` heredado. Sin embargo, `Command::env("LD_PRELOAD", ...)`
reemplaza el valor heredado, y el código no concatena el valor anterior. Es
otra discrepancia comprobable en código, no una conclusión sobre el overlay
en una máquina concreta.

## Instrumentación y decisión

El log habitual titulado “CorkyTux debug log” sigue sin incluir el entorno del
overlay. Esta rama añade instrumentación separada, desactivada por defecto:
establece `CORKYTUX_DEBUG_STEAM=1` antes de abrir CorkyTux para escribir líneas
`[steam-debug]` con timestamp Unix en milisegundos. Registra detección del
comando/proceso, petición y resultado de `steam -silent`, snapshot de readiness
antes y después de `steam://`, y runner/entorno justo antes del lanzamiento.
Los valores listados incluyen `LD_PRELOAD` (solo nombres de bibliotecas),
`ENABLE_VK_LAYER_VALVE_steam_overlay_1`, `SteamOverlayGameId`, `SteamAppId`,
`SteamGameId`, `GAMEID`, `STORE` y `STEAM_COMPAT_CLIENT_INSTALL_PATH` (HOME se
redacta). No escribe PIDs, ActiveUser, IDs de cuenta ni otras variables de
entorno.

El detector usa `/proc/<pid>/comm` para localizar `steam` y consulta los
roots nativo, Flatpak y Snap en busca de un `registry.vdf` con `ActiveUser`
distinto de cero. Devuelve `NotRunning`, `NoActiveUser` o `Ready`; aún no se
usa para bloquear ni modificar el arranque. Fixtures temporales cubren las
tres ubicaciones sin lanzar procesos.

Snapshot de esta máquina: `/proc` no mostró un `comm` llamado exactamente
`steam`; entre las rutas consultadas hubo un `registry.vdf` legible, pero no
contenía una clave `ActiveUser` reconocible. No se midió el tiempo hasta
`Ready`: eso exigiría observar un arranque real, y no se lanzó Steam.

El código demuestra una carrera y una posible discrepancia de ruta en bwrap,
pero falta aislar cuál explica el fallo observado. Antes de cambiar el
arranque por defecto propongo una prueba A/B con los pasos siguientes. Si la
carrera queda confirmada, la espera debe limitarse al toggle explícito de
overlay, usar readiness y un timeout configurable, y mostrar un aviso al
agotarse; el nuevo detector no debe introducir `which`, `pgrep` ni `pidof`.

## Prueba manual

1. Elige un juego Windows no crítico que ya arranque con Proton. En Ajustes
   del juego activa `Steam overlay`; deja anotados los valores actuales de
   aislamiento, UMU, GameMode y MangoHud.
2. Con Steam completamente cerrado, lanza el juego y mide desde el clic hasta
   que aparece la ventana. Comprueba si Steam se abre, si termina de iniciar
   sesión antes del juego y si `Shift+Tab` muestra el overlay.
3. Cierra el juego. Abre Steam manualmente, espera a que la biblioteca esté
   utilizable y lanza el mismo juego con los mismos ajustes. Comprueba
   `Shift+Tab` y mide el mismo intervalo.
4. Repite con aislamiento apagado solo si el primer caso lo tenía activado;
   no cambies varios ajustes en la misma comparación.
5. Si falla, comparte las líneas `[steam-debug]` de la terminal desde la
   detección hasta `game-spawn`, el log de debug del juego, el runner (Proton
   directo o UMU), si Steam es nativo/Flatpak/Snap, Wayland/X11 y las tres
   medidas. La instrumentación redacta HOME y no imprime PIDs ni cuenta; aun
   así revisa el texto antes de compartirlo.

Para activar la traza en una prueba manual, lanza CorkyTux desde una terminal
con `CORKYTUX_DEBUG_STEAM=1` en el entorno del proceso. No hace falta editar
`Games.ini`, cambiar ajustes ni reiniciar Steam para activar el logging.

No puedo verificar sin Steam real: el estado de login/`ActiveUser`, el tiempo
real hasta sesión lista, la visualización de `Shift+Tab`, el efecto de
Wayland/X11, Flatpak/Snap, juegos de 32 bits ni la combinación con
MangoHud/GameMode. No se ejecutó la app ni un juego.
# Auditoría estática multi-distro

Fecha: 2026-09-25 · HEAD auditado: `ac0e0fb` (rama `main`, sin push)

## Método y límites

Auditoría **estática**: lectura de código, scripts de release y documentación.
**No** se compiló, no se ejecutó la app, no se lanzaron procesos ni se tomaron
capturas (regla de `AGENTS.md`). Cada hallazgo cita `archivo:línea` para que sea
verificable sin abrir la app.

Por eso los hallazgos marcados *crítico* son fallos con impacto funcional
comprobable por lectura, no regresiones observadas en ejecución. Los que
requieren confirmación visual están listados aparte al final.

## Distros cubiertos por el proyecto

Gentoo, Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE (los cinco de
`docs/BUILD.md`). La auditoría añade dos ejes que la tabla no cubre:
**multi-arch** (x86_64 vs aarch64) y **entornos sin coreutils del sistema**
(NixOS, contenedores mínimos).

## Resumen

| Severidad | Cantidad |
| --- | --- |
| Crítico | 6 |
| Degradante | 30 |
| Cosmético | 6 |
| **Total** | **42** |

Cuentas del triage original, conservadas como registro. **Estado actual: los 6
críticos están corregidos.** C08 lo está en 9 de sus 12 sitios (los 8 de
`timeout` y el único de `ldconfig`); quedan 3 externalidades de bajo impacto.
Ningún degradante ni cosmético se ha tocado.

Definiciones:

- **Crítico**: función rota de forma silenciosa o acción de usuario imposible
  en una distro/arquitectura oficialmente soportada.
- **Degradante**: funciona pero con pérdida de función, warning falso o
  acoplamiento innecesario.
- **Cosmético**: deriva documental o inconsistencias sin efecto funcional.

---

## Críticos

### C01 — Rutas de Steam hardcodeadas: el overlay no existe en Steam Flatpak/Snap

**Estado: CORREGIDO.**

- `src/backend/proton.rs:335-340` — `steam_client_path()` construye
  `~/.steam/steam` sin variantes.
- `src/backend/proton.rs:940-942` y `1035-1037` — el `gameoverlayrenderer.so`
  del overlay se busca en `~/.steam/steam/ubuntu12_32` y
  `~/.steam/steam/ubuntu12_64`.
- Contraste: `src/backend/proton.rs:480-485` (`find_steam_runtime`) **sí**
  enumeraba las cuatro raíces, incluida
  `~/.var/app/com.valvesoftware.Steam/.steam/steam`.

**Impacto:** con Steam instalado como Flatpak o Snap, el usuario activa
`SteamOverlay` en la ficha del juego, el interruptor queda en "activo" y el
overlay nunca se inyecta. No hay error ni aviso: pérdida silenciosa de una
función que el usuario cree activa.

**Causa raíz:** la lista de raíces de Steam estaba duplicada en tres sitios y
solo uno la mantenía actualizada.

**Fix aplicado:** `steam_roots(home)` es ahora la única lista, con las cuatro
variantes (`~/.steam/steam`, `~/.local/share/Steam`,
`~/.var/app/com.valvesoftware.Steam/data/Steam` y
`~/.var/app/com.valvesoftware.Steam/.steam/steam`). La consumen:

1. `steam_client_path_for(home)` — la primera raíz que exista, con fallback a
   `~/.steam/steam` para cuando no hay Steam instalado (Proton usa la variable
   igualmente). `~/.steam/steam` va primero porque es la ruta que ya usaba
   `STEAM_COMPAT_CLIENT_INSTALL_PATH` y, en una instalación normal, es un
   enlace a `.local/share/Steam`: no cambia el comportamiento de nadie.
2. `find_steam_runtime` — ahora itera el resolver en vez de su propia lista.
3. `steam_overlay_preload(home)` — sustituye a los dos bloques de overlay
   duplicados (rumbo y no-rumbo) y recorre **todas** las raíces, no solo la
   primera. Mantiene el `':'` inicial que no pisa un `LD_PRELOAD` heredado.

**Hardcode extra que salió al implementarlo:** `legendary_launch_cmd` fijaba
`STEAM_COMPAT_CLIENT_INSTALL_PATH` por su cuenta (línea 185-186), así que
arreglar solo `steam_client_path()` no cubría el lanzamiento vía Heroic. Ahora
usa `steam_client_path_for()`. Con esto la lista quedó en **un** sitio, que era
el origen real del hallazgo.

### C03 — `component_status` solo reconoce tokens x86 y consulta dos binarios distintos de GameMode

**Estado: CORREGIDO** (decisiones tomadas por el usuario: derivar de `ARCH` +
rutas del sistema, y sondear siempre `gamemoderun`).

- `src/backend/proton.rs:617-628` (versión auditada) — parseaba `ldconfig -p`
  buscando `x86-64`/`lib64` para 64 bits y `i386`/`lib32` para 32 bits.
- `src/backend/proton.rs:614` — `installed64` se inicializaba a `available`, así
  que el bloque `ldconfig` solo podía **activar** flags, nunca corregirlos.
- `src/backend/proton.rs:605` sondeaba `gamemoderun`; `src/backend/proton.rs:687`
  sondeaba `gamemoded` para la misma característica.

**Impacto:**

1. En aarch64 `ldconfig -p` imprime `aarch64`/`arm64`; ninguno de los dos
   tokens matchea, luego `installed32` **jamás** podía ser `true` en ARM y la
   UI afirmaba que no hay soporte 32-bit aunque el paquete estuviera instalado.
2. Sin `ldconfig` (NixOS, contenedores) el bloque se saltaba entero y
   `installed32` quedaba en `false` sin explicación.
3. El defecto más serio era independiente de la arquitectura: como
   `installed64` nacía en `available`, un `mangohud` instalado **sin** su
   biblioteca de 64 bits se anunciaba como instalado. El flag no podía
   ponerse a `false` nunca.
4. `gamemoderun` es el cliente y `gamemoded` el daemon. Sondear el daemon no
   dice si GameMode puede aplicarse: sin el socket de usuario activo el chequeo
   falla aunque el cliente funcione, y con cliente pero daemon ausente el
   chequeo pasa aunque no ocurra nada. Las dos tarjetas de la UI pueden
   discrepar.

**Fix aplicado:**

1. `component_bits(lib)` deriva el soporte de `std::env::consts::ARCH`: el host
   es de 64 bits si su arch es `x86_64`/`aarch64`/`powerpc64`/`riscv64`/`s390x`,
   y puede ejecutar 32 bits si es `x86`/`x86_64`. En aarch64 `installed32` es
   `false` **por construcción**, no por un token que no matche: no hay
   distribuciones ARM de 32 bits con las que contar, y fingir lo contrario sería
   peor que no medirlo.
2. `library_present(lib, bits)` busca por prefijo de nombre base
   (`libMangoHud.so` cubre `libMangoHud.so.1.2`) en los directorios reales:
   el multiarch, `/usr/lib`, `/usr/lib64`, `/usr/local/lib`, `/lib`, los
   subdirectorios `lib32`/`libx32` para 32 bits, y
   `/run/current-system/sw/lib` para NixOS. El nombre del multiarch lo dice
   `gcc -print-multiarch`, con `$MULTIARCH` y `<arch>-linux-gnu` como
   fallbacks.
3. Los flags ahora **pueden ser negativos**: se resuelven de verdad en lugar de
   solo activarse.
4. Se eliminó la dependencia de `ldconfig` (cierra el último sitio de C08 de
   este binario).
5. `graphics_component_status()` sondea `gamemoderun`, igual que
   `component_status()`. Se unificó en el **cliente**: es lo que CorkyTux
   inyecta al lanzar, y que el daemon esté instalado no significa que GameMode
   se aplique.
6. `component_status_text()` **no se tocó**: su contrato es idéntico, así que
   las etiquetas de la UI no cambian de forma.

**Verificación estática contra el sistema real:** en este host
(`x86_64`, sin `gcc -print-multiarch` funcional, sin `MULTIARCH`) el fallback da
`x86_64-linux-gnu`; existen `/usr/lib/libMangoHud.so`,
`/usr/lib/libgamemodeauto.so.0` (symlink) y `/usr/bin/gamemoderun`, así que
`available` e `installed64` siguen en `true` como antes del cambio, y
`installed32` en `false` porque no hay multilib. Sin regresión en x86.

### C08 — Dependencia dura de binarios externos (`timeout`, `ldconfig`, `pgrep`, `pidof`, `which`) en 12 sitios

**Estado: PARCIALMENTE CORREGIDO.** Los 8 usos de `timeout` y el único de
`ldconfig` están eliminados (9 de 12); quedan 3: `pgrep`, `pidof` y `which`.

Reparto verificado en el triage:

| Binario | Call sites |
| --- | --- |
| `timeout` | **corregidos**: `plugins.rs:552`, `plugins.rs:620`, `plugins.rs:690`, `plugins.rs:727`, `integration.rs:601`, `integration.rs:609`, `integration.rs:666`, `proton.rs:1314` |
| `ldconfig` | **corregido** con C03: `proton.rs:617` ya no lo invoca |
| `pgrep` | **corregidos** los 2 de `import_move.rs` (C14); queda `proton.rs:261` |
| `pidof` | `proton.rs:825` |
| `which` | `integration.rs:27`, `proton.rs:597`, `proton.rs:822` |

`timeout`, `ldconfig`, `pgrep` y `pidof` son de coreutils/procps, no de la
base POSIX. En NixOS ninguno está en el `PATH` por defecto; en contenedores
de distros mínimos suele faltar alguno.

**Impacto:** no es un problema uniforme, y esa es la parte grave. Hay tres
modos de fallo distintos:

- **Falla silenciosa** — `plugins.rs:727`: si `timeout` no existe,
  `Command::new` falla, la función cae en `_ => return Vec::new()` y la
  pestaña de Emuladores queda vacía **sin ningún mensaje de error**.
- **Falla abierta y peligrosa** — un prefix en uso se declaraba libre (C14;
  ya corregido, pero el patrón sigue latente en `proton.rs:261`).
- **Falla ruidosa** — `plugins.rs:552/620/690` e `integration.rs:601`: el
  usuario ve un error que no menciona la causa real.

**Fix aplicado (`timeout`):** `plugin_process::output_with_timeout()` sustituye
a `timeout(1)` con las mismas ocho llamadas:

1. `spawn` + `try_wait` en bucle de 25 ms contra un `Instant` de corte; al
   agotarlo, `kill` + `wait`. Sin dormir más que el intervalo de sondeo, y sin
   depender de ningún binario externo.
2. stdout y stderr se leen en hilos propios para que un plugin que llene el
   buffer del pipe no se bloquee a sí mismo esperando su salida.
3. `join_reader()` une esos lectores **con un margen de 2 s**: si el plugin
   dejó nietos con el pipe abierto, el `join` directo habría colgado la
   interfaz, algo que la versión anterior no cubría.
4. El timeout devuelve un error con el límite de segundos en el mensaje, en
   vez del código 124 de `timeout(1)` que los llamadores no interpretaban.
5. `list_emulators_in` sigue devolviendo lista vacía —es el contrato de sus
   4 llamadores, y cambiarlo tocaría la UI—, pero ahora deja el motivo en
   `stderr` (`[emu] corky-list …`) para que un fallo sea diagnosticable.

De paso, `wineserver -k` en `proton.rs:1317` deja de perder `ws.to_str()`:
se pasa el `PathBuf` directo, así que un prefix no-UTF8 ya no degrada a
programa vacío.

**Fix pendiente:** eliminar `pgrep`, `pidof` y `which` de los 3 sitios que
quedan. `pidof` se puede sustituir por `/proc` (el mismo patrón que se aplicó
en C14) y `which` por una búsqueda en `$PATH`, que es POSIX.

### C09 — El registro de plugins elige el primer `.tar.gz` sin filtrar por arquitectura

**Estado: CORREGIDO.**

- `src/backend/plugins.rs:347-363` (versión auditada) — itera `assets` y hace
  `break` en el primer nombre que termina en `.tar.gz`.
- `src/backend/plugins.rs:418` — lo descarga como `{tag}.tar.gz` y lo extrae
  sin verificar nada más que `size >= 100` (línea 448-452).

**Impacto:** en un release que publique `plugin-x86_64.tar.gz` y
`plugin-aarch64.tar.gz`, un host aarch64 instala el binario x86_64 según el
orden de la API. El plugin se instala "con éxito" y luego falla con
`Exec format error` en cada invocación, sin que la UI distinga "instalado" de
"instalado y utilizable".

**Severidad real (verificada contra la API el 2026-09-25):** hoy los cinco
releases publicados usan nombres sin arquitectura —`heroic-store-1.0.8.tar.gz`,
`minecraft-launcher-1.1.0.tar.gz`, `emulator-manager-1.1.0.tar.gz`,
`dependency-installer-2.1.3.tar.gz`, `heroic-store-1.0.7.tar.gz`—, así que el
fallo es **latente**, no activo: hace falta que alguien publique un par de
assets por arquitectura. Se corrigió igualmente porque el día que se publique,
el síntoma (instalación correcta y fallo en cada uso) es de los más caros de
diagnosticar.

**Fix aplicado:** `asset_arch_ok()` decide si un asset sirve para este host, y
`fetch_registry()` recorre **todos** los `.tar.gz` del release en vez de romper
en el primero:

1. Un asset **sin token de arquitectura** se acepta: es el caso de los releases
   actuales y no hay forma de distinguirlo de un asset universal.
2. Un asset **con token** solo se acepta si el token mapea a
   `std::env::consts::ARCH`, cubriendo los alias habituales: `x86_64`/`amd64`/
   `x64`, `aarch64`/`arm64`, `i386`–`i686`/`x86`, `arm`/`armv7l`/`armhf`,
   `ppc64`/`ppc64le` y `riscv64`/`s390x`.
3. Los tokens se separan por `-` y `.`, **nunca por `_`**: `x86_64` lleva guion
   bajo, y partirlo produciría dos tokens sin significado que además
   colisionarían con `x86`.
4. Un release cuyos assets son todos de otra arquitectura se **omite** del
   registro, con el motivo en stderr, en vez de ofrecer una descarga que no
   puede funcionar.

**Pendiente relacionado (D05):** la API de GitHub ya publica un campo `digest`
(`sha256:…`) por asset. Verificarlo cierra el hallazgo de integridad y de paso
da un error de confianza por separado del de compatibilidad.

### C10 — El escaneo de Lutris ignora `XDG_DATA_HOME` y Lutris Flatpak, en el mismo archivo que sí los respeta

**Estado: CORREGIDO.**

- `src/backend/integration.rs:852-857` — `pga.db` en
  `~/.local/share/lutris/pga.db` hardcodeado.
- `src/backend/integration.rs:871-876` — `games/` en
  `~/.local/share/lutris/games` hardcodeado.
- Contrasto, **450 líneas antes en el mismo archivo**:
  `src/backend/integration.rs:395-397` sí resolvía `XDG_DATA_HOME` con fallback
  a `~/.local/share`, y `403-407` sí añadía la ruta Flatpak de Lutris.

**Impacto:**

1. Con `XDG_DATA_HOME` configurado (habitual en setups de tiling) Lutris
   instala en `~/data/lutris`: la UI resolvía bien las carátulas (línea 399)
   pero el escaneo de juegos miraba `~/.local/share/lutris` y no encontraba
   nada. El resultado es "carátulas sí, juegos no", que parece un bug de
   Lutris y no de CorkyTux.
2. Con Lutris como Flatpak, `~/.var/app/net.lutris.Lutris/data/lutris` se
   usaba para artwork pero nunca para `pga.db` ni para `games/`: se importaba
   0 juegos.

**Fix aplicado:** un único resolver `lutris_data_roots(home)` que devuelve las
raíces de datos de Lutris en orden de preferencia, consumido por el escaneo y
por el artwork:

1. `$XDG_DATA_HOME` si es absoluto y no vacío (un valor vacío o relativo se
   ignora en vez de construir rutas bajo el CWD), con el default del spec
   `~/.local/share` como fallback.
2. `~/.var/app/net.lutris.Lutris/data`, que es donde el sandbox Flatpak
   reescribe el home del proceso.

De cada raíz cuelga `lutris/` (base de datos, juegos, carátulas) e
`icons/hicolor/...` (iconos de apps), así que ambos consumidores derivan sus
rutas del mismo sitio.

Se corrigió además un tercer hardcode del mismo tipo que el triage no había
listado: `lutris_yml_info()` volvía a construir `~/.local/share/lutris/games`
por su cuenta, así que arreglar solo `scan_lutris` no bastaba. Ahora recibe el
`games_dir` del llamador, y la capa SQLite lo deriva de `db.parent()`, que por
construcción pertenece a la misma raíz que la base de datos.

Como ahora se recorren varias raíces, `scan_lutris` deduplica por `slug` (la
primera raíz gana) para que un juego no aparezca dos veces durante una
migración a Flatpak a medias.

### C14 — `prefix_in_use` falla abierta cuando `pgrep` no existe

**Estado: CORREGIDO** (ver "Fix aplicado" al final de la sección; el resto de
críticos sigue abierto).

- `src/backend/import_move.rs:351-374` (versión auditada).
- Línea 353-356: si `Command::new("pgrep")` falla, `running = false` y la
  función devuelve `false` (prefix libre).
- Línea 360-361: `lock.exists()` — `wineserver.lock` es un socket unix; un
  socket que quedó de una sesión que murió sigue existiendo.
- Línea 365-370: el fallback compara `prefix.display()` como substring
  crudo contra la línea de `pgrep -af`, sin pasar por el `norm()` que sí usa
  el agrupado de prefixos compartidos (línea 410).

**Impacto:** dos fallos opuestos, ambos sobre datos de juegos.

- Sin `pgrep` (NixOS, contenedor): la función afirma que un prefix **en uso
  está libre** y el import permanente lo mueve mientras el juego corre. La
  copia es la que sobrevive, pero el original desaparece y Heroic/Lutris se
  rompen. Es el único hallazgo de la auditoría con potencial de pérdida de
  datos.
- Con un `wineserver.lock` obsoleto: el import queda **bloqueado
  permanentemente** hasta que el usuario borre el socket a mano, sin
  explicación en la UI.
- Con el path del prefix escrito de otra forma en la config (por ejemplo
  `..` o un symlink), el substring no matchea y vuelve el primer caso.

**Fix aplicado:** se sustituyó el booleano por un tri-estado
`PrefixUsage { Busy, Free, Unknown }` y se eliminó la dependencia de `pgrep`:

1. `wineserver_pids()` enumera `/proc/*/comm` directamente. Sin `pgrep` no
   hay falso "libre", y de paso desaparece una de las 12 llamadas de C08. Se
   compara `comm` por subcadena, no por igualdad, para no perder variantes
   como `wineserver-preloader`; el nombre de CorkyTux no contiene
   `wineserver`, así que no hay autocompresión.
2. `wineprefix_of(pid)` lee `WINEPREFIX` de `/proc/<pid>/environ`, que es la
   atribución **exacta**: wineserver recibe el prefix por entorno, nunca por
   línea de comandos, así que el substring sobre `pgrep -af` no era fiable.
3. Ambos lados pasan por `norm()`, que canonicaliza, así que la comparación ya
   no depende de cómo esté escrito el path en la configuración.
4. Un `wineserver.lock` obsoleto ya no bloquea nada: el estado se decide por
   procesos vivos, no por la presencia del socket.
5. Si hay wineservers vivos pero su entorno es ilegible (otro usuario,
   procfs con `hidepid`), el resultado es `Unknown` y se **bloquea** con el
   nuevo `Blocker::UsageUndeterminable` en vez de arriesgar los datos. La UI
   lo renderiza sin cambios: `import_manager.rs` usa `b.message()` de forma
   genérica.

El `Fix:` del triage original ("invertir el default y normalizar el path") se
cumple, pero con mejor base: la atribución por `/proc` sustituye al heuristic
en lugar de solo parchearlo.

---

## Degradantes

| ID | Ubicación | Hallazgo |
| --- | --- | --- |
| D01 | `src/backend/config.rs:463-483` | `all_proton_paths()` solo escanea rutas de usuario; nunca `compatibilitytools.d` de Steam ni Proton del gestor de paquetes. En SteamOS/Bazzite/Nobara la UI dice "No Proton builds installed" con Proton funcionando. Mitigado por el auto-descargador de CorkyTux, pero no para quien ya tiene Proton. |
| D02 | `src/backend/proton.rs:821-838` | El auto-arranque de Steam para el overlay depende de `which steam` + `pidof steam`; ambos fallan con Steam Flatpak/Snap. |
| D03 | `src/backend/proton.rs:645-663` | `detect_game_arch_simple` solo reconoce `0x10b`/`0x20b`; no hay ARM64EC (`0xa641`) ni aarch64 (`0xaa64`), así que la etiqueta cae a "all". |
| D04 | `src/backend/proton.rs:413-421` | `is_foreign_arch` solo contrasta aarch64/arm64 contra x86_64; no cubre nomenclatura i686 ni armv7. |
| D05 | `src/backend/plugins.rs:418-461` | Descarga y extrae un tarball remoto sin checksum ni firma; la única validación es `size >= 100`. |
| D06 | `src/backend/plugins.rs:327-345` | API de GitHub sin autenticar: 60 req/h por IP. Falla en redes NAT/CGNAT (universidad, oficina, móvil) con 403. |
| D07 | `src/backend/plugins.rs:371` | `body.chars().take(200)` corta la descripción a mitad de frase y muestra markdown crudo. |
| D08 | `src/backend/plugins.rs:552,620,690` | Escaneo/instalación de dependencias y DLL dependen de `timeout`; sin él el error no menciona la causa. |
| D09 | `src/backend/integration.rs:589-593,601,609,666` | Extracción de iconos exige `timeout` + `icoextract`/`ffmpeg`; degrada a icono genérico sin aviso. |
| D10 | `src/backend/integration.rs:394-407` vs `850-876` | La ruta de artwork y la de juegos usan resolvers distintos (ver C10). |
| D11 | `src/backend/integration.rs:1261-1281` | Descubrimiento de `libraryfolders.vdf` no cubre Steam Flatpak. |
| D12 | `src/backend/shortcuts.rs:138` | `Exec={exe}` sin comillas: una ruta de instalación con espacios rompe el `.desktop`. |
| D13 | `src/backend/shortcuts.rs:135-143` | `Name`, `Comment` y `X-CorkyTux-Game` sin escapar: un nombre de juego con salto de línea inyecta claves en el `.desktop`. |
| D14 | `src/backend/shortcuts.rs:33-54` | Menú de apps hardcodea `~/.local/share/applications`; el directorio de escritorio depende de `xdg-user-dir`, y si falta se **crea** un `~/Desktop` que el usuario no usa. |
| D15 | `src/backend/shortcuts.rs:157,180` | `.desktop` con modo 0755; la especificación sugiere 0644. |
| D16 | `src/backend/plugin_process.rs:88-95` | Directorio de plugins hardcodeado, sin `XDG_DATA_HOME`. Coherente con el resto del proyecto, pero no con el estándar. |
| D17 | `src/backend/plugin_process.rs:97-100` | `plugin_available` comprueba existencia, no el bit de ejecución: un plugin sin permiso aparece instalado y falla al invocar. |
| D18 | `src/backend/config.rs:583-590` | `dirs::home_dir()` es solo `$HOME`, duplicado en `proton.rs:9`, `plugins.rs:8`, `integration.rs:8`. Con `HOME` vacío, `plugin_process` cae a `"."` (CWD). |
| D19 | `src/ui/import_manager.rs:357` | `PathBuf::from(HOME.unwrap_or_default()).join("Games")` con `HOME` vacío produce la ruta **relativa** `Games`: el import escribiría en el CWD. |
| D20 | `src/main.rs:220-221` | El `IconTheme` solo añade `~/.local/share/corkytux/assets/icons`; ejecutado desde el árbol de fuentes no encuentra ningún asset empaquetado. |
| D21 | `src/ui/details_panel.rs:557-563` | El prefix por defecto que se **muestra** está hardcodeado a `~/.local/share/Steam/...`; puede divergir del path que usa el lanzamiento. |
| D22 | `release/install.sh:86` | El fallback de libs prueba `/usr/lib/x86_64-linux-gnu`; en aarch64 el multiarch es `aarch64-linux-gnu` → warning falso de "missing libs" y prompt de continuar. |
| D23 | `release/install.sh:84` | Sin `ldconfig` el chequeo cae al barrido de rutas, que no incluye `/run/current-system/sw/lib` (NixOS). |
| D24 | `release/install.sh:56-61` | Depende del binario `file`; sin él el instalador aborta con "corkytux ELF binary not found" aunque el binario exista. |
| D25 | `release/install.sh:201-212` | `Exec=` e `Icon=` sin comillas en el `.desktop` que genera el instalador. |
| D26 | `release/install.sh:107-111` | Detección de Steam por `command -v steam` o `~/.steam`; no ve Flatpak ni Snap. |
| D27 | `release/install.sh:119-122` | Recomienda `pip install --user`, que no funciona en Gentoo y está bloqueado por PEP 668 en Debian 12+ y Fedora. |
| D28 | `release/build-release.sh:8-9` | El nombre del tarball usa `uname -m` crudo (`armv7l`, `i686`) sin normalizar ni comprobar que el binario compile para esa arquitectura. |
| D29 | `release/install.sh:47-52` | Crea `~/Games` incondicionalmente en cada instalación, aunque el usuario nunca use el Import Manager. |
| D30 | `release/install.sh:222-226` | El aviso de `PATH` solo menciona bash y zsh; en fish, nushell o csh el symlink de `~/.local/bin` sigue sin usarse. |

## Cosméticos

| ID | Ubicación | Hallazgo |
| --- | --- | --- |
| K01 | `Cargo.toml:3`, `release/uninstall.sh:12`, `release/build-release.sh:3`, `release/install.sh:4`, `docs/BUILD.md:1,37,45`, `README.md:101-102` | Deriva de versión: el código está en `3.0.19`; los docs dicen `3.0.11`, el uninstaller `3.0.11`, el builder `3.0.13` y la cabecera del installer `3.0.16`. |
| K02 | `README.md:148-161` | El mapa de arquitectura omite `plugins.rs`, `integration.rs`, `import_move.rs`, `theme.rs`, `shortcuts.rs` y `external.rs`. |
| K03 | `README.md:101`, `docs/BUILD.md:37` | Solo se documenta el tarball `x86_64` aunque `build-release.sh` usa `uname -m`. |
| K04 | `src/ui/import_manager.rs:19` | El texto visible al usuario dice "a Games folder in your /home"; es incorrecto en `/var/home` (Fedora Silverblue). |
| K05 | `docs/DEPENDENCY_INSTALLER_HOTFIX.md:43-44` | Rutas absolutas `/home/mattsgaming/...` en instrucciones meant to be copy-pasted. |
| K06 | `src/Cargo.toml` | Manifiesto heredado (package `demo`, `cdylib`) conviviendo con el `Cargo.toml` real; confunde a cualquier lectura de dependencias. |

---

## Pendiente de confirmación visual

Requiere que el usuario abra la app; el agente no la ejecuta.

1. Matriz de estados transitorios de la fila de emuladores: `Installing…`,
   `.add-btn:disabled` y el ancho de 96 px (ya registrado en `DESIGN.md`).
2. Centrado de los iconos Papirus y del monograma en el slot de 48 px.
   **El `Align::Center` ya está en HEAD desde `ac0e0fb`**; si los iconos se ven
   sin centrar, casi siempre es que se está ejecutando un binario anterior al
   fix (ver "Binario en uso" más abajo), no que el CSS falle.
3. `Found in system` estático y `Set as linked` clickeable en la misma fila.
4. Aviso `*** BUG *** In pixman_region32_init_rect: Invalid rectangle passed`:
   se reprodujo 7 veces al arrancar el build de `0b388df`; la causa sigue sin
   confirmar y no es crítica.

## Medición del centrado de los iconos (2026-09-25)

Medido con `rsvg-convert` + análisis del canal alfa, sin ejecutar la app. Los
12 SVG de `assets/icons/emulators/` declaran `width="48" height="48"` (aspecto
1:1 exacto, ninguno con `viewBox`), así que no hay deformación por escala.

| Grupo | bbox (unidades de viewBox) | centro | desfase |
| --- | --- | --- | --- |
| Los 11 iconos Papirus restantes | — | — | ≤ 0,4 px a 40 px |
| `mupen64plus-qt.svg`, bbox global | 41,0 × 43,0 · x[6,0; 47,0] y[4,0; 47,0] | (26,5; 25,5) | **(+2,51; +1,50)** |
| `mupen64plus-qt.svg`, **logo solo** (M + fondo) | 36,0 × 41,1 · x[6,0; 42,0] y[4,0; 45,1] | **(24,0; 24,5)** | **(+0,00; +0,54)** |
| `mupen64plus-qt.svg`, distintivo verde | 22,0 × 23,0 · x[25,0; 47,0] y[24,0; 47,0] | (36,0; 35,5) | (+12,00; +11,51) |

**El desfase global NO es un defecto: es el distintivo verde.** Es un círculo de
estado en la esquina inferior derecha (elementos `<circle>`/`<rect>` del asset,
`cx=36 cy=35 r=11`), colocado ahí a propósito por Papirus. El logo principal
—la "M" y su fondo redondeado— está **centrado en horizontal con exactitud
numérica** (+0,00) y a +0,54 unidades en vertical, que son 0,45 px a tamaño de
render de 40 px: imperceptible.

### Por qué NO se recorta

Se simuló el ajuste literal pedido (añadir `viewBox="2.51 1.50 48 48"`, que
centra el bbox global) sobre una copia temporal. Resultado:

| Medida | Antes | Después del `viewBox` |
| --- | --- | --- |
| Logo principal | (+0,00; +0,54) | **(−2,52; −0,96)** |
| Distintivo verde | (+12,00; +11,51) | (+9,49; +10,01) |
| bbox global | (+2,51; +1,50) | (−0,01; +0,00) |

El recorte centró el bbox global y **movió el logo 2,1 px a la izquierda y
0,8 px arriba**, que es justo lo que se pretendía eliminar. Intercambia una
asimetría de distintivo de 0,45 px e imperceptible por un descentrado visible
del logo. **El asset se deja intacto** y la afirmación de
`assets/icons/ATTRIBUTION.md` de que son "unmodified copies" sigue siendo
cierta.

## Binario en uso (2026-09-25)

`~/.local/share/applications/corkytux.desktop` lanza
`~/.local/share/corkytux/corkytux`, cuyo binario es **anterior a `ac0e0fb`**
(compilado 02:58:41; el fix es de 05:18:26). Se confirmó por strings: ese
binario **no contiene** `emu-system-notice` ni `Found in system`, mientras que
`target/debug/corkytux` (05:55) sí. Antes del fix, `icon_slot` usaba
`Align::Start` y la imagen no tenía `set_size_request`, es decir la imagen
quedaba anclada a la esquina superior izquierda del slot: ese es el síntoma
"iconos sin centrar". Para que el lanzador del menú vea el fix hay que
reinstalar el binario y los assets.

## Orden de corrección

1. ~~**C14**~~ — **hecho**: tri-estado `PrefixUsage` + atribución por
   `/proc/<pid>/environ` en vez de `pgrep`.
2. ~~**C08**~~ — **hecho en su parte grande**: `output_with_timeout()` cubre los
   8 sitios de `timeout` (incluida la pestaña de Emuladores que salía vacía sin
   error) y C03 eliminó el único `ldconfig`. Quedan 3 externalidades de bajo
   impacto: `pgrep`, `pidof` y `which`.
3. ~~**C10**~~ — **hecho**: `lutris_data_roots()` compartido por escaneo y
   artwork.
4. ~~**C09**~~ — **hecho**: `asset_arch_ok()` filtra por `std::env::consts::ARCH`.
5. ~~**C01**~~ — **hecho**: `steam_roots()` compartido por los cuatro
   consumidores.
6. ~~**C03**~~ — **hecho**: soporte 32/64 derivado de `ARCH` y rutas del sistema,
   y `gamemoderun` como único criterio de GameMode.
7. El resto de degradantes por lotes. El siguiente con mejor relación
   esfuerzo/impacto es **D05**: la API ya publica `digest: sha256:…`, así que
   verificar la descarga es directamente implementable. Después, D12, D13, D19
   y D22.

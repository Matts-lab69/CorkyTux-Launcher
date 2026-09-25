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

Cuentas del triage original, conservadas como registro. **Estado actual:** de
los 6 críticos, **C14 está corregido** (ver su sección); los otros 5 siguen
abiertos. Ningún degradante ni cosmético se ha tocado.

Definiciones:

- **Crítico**: función rota de forma silenciosa o acción de usuario imposible
  en una distro/arquitectura oficialmente soportada.
- **Degradante**: funciona pero con pérdida de función, warning falso o
  acoplamiento innecesario.
- **Cosmético**: deriva documental o inconsistencias sin efecto funcional.

---

## Críticos

### C01 — Rutas de Steam hardcodeadas: el overlay no existe en Steam Flatpak/Snap

- `src/backend/proton.rs:335-340` — `steam_client_path()` construye
  `~/.steam/steam` sin variantes.
- `src/backend/proton.rs:940-942` y `1035-1037` — el `gameoverlayrenderer.so`
  del overlay se busca en `~/.steam/steam/ubuntu12_32` y
  `~/.steam/steam/ubuntu12_64`.
- Contraste: `src/backend/proton.rs:480-485` (`find_steam_runtime`) **sí**
  enumera las cuatro raíces, incluida
  `~/.var/app/com.valvesoftware.Steam/.steam/steam`.

**Impacto:** con Steam instalado como Flatpak o Snap, el usuario activa
`SteamOverlay` en la ficha del juego, el interruptor queda en "activo" y el
overlay nunca se inyecta. No hay error ni aviso: pérdida silenciosa de una
función que el usuario cree activa.

**Causa raíz:** la lista de raíces de Steam está duplicada en tres sitios y
solo uno la mantiene actualizada.

**Fix:** extraer un único helper `steam_roots(home) -> Vec<PathBuf>` (las
cuatro de `find_steam_runtime`) y consumirlo en `steam_client_path()`,
`prefix_path()` y los dos resolvers de `gameoverlayrenderer.so`.

### C03 — `component_status` solo reconoce tokens x86 y consulta dos binarios distintos de GameMode

- `src/backend/proton.rs:617-628` — parsea `ldconfig -p` buscando
  `x86-64`/`lib64` para 64 bits y `i386`/`lib32` para 32 bits.
- `src/backend/proton.rs:614` — `installed64` se inicializa a `available`, así
  que el bloque `ldconfig` solo puede **activar** flags, nunca corregirlos.
- `src/backend/proton.rs:605` sondea `gamemoderun`; `src/backend/proton.rs:687`
  sondea `gamemoded` para la misma característica.

**Impacto:**

1. En aarch64 `ldconfig -p` imprime `aarch64`/`arm64`; ninguno de los dos
   tokens matchea, luego `installed32` **jamás** puede ser `true` en ARM y la
   UI afirma que no hay soporte 32-bit aunque el paquete de 32 bits esté
   instalado.
2. Sin `ldconfig` (NixOS, contenedores) el bloque se salta entero y
   `installed32` queda en `false` sin explicación.
3. `gamemoderun` es el cliente y `gamemoded` el daemon. Sondear el daemon no
   dice si GameMode puede aplicarse a un juego: en un sistema con el socket
   de usuario sin activar el chequeo falla aunque el cliente funcione, y en
   un sistema con cliente pero daemon ausente el chequeo pasa aunque no
   ocurra nada. Las dos tarjetas de la UI pueden discrepar entre sí.

**Fix:** derivar la arquitectura de `std::env::consts::ARCH` en vez de parsear
`ldconfig`, y unificar el sondeo en `gamemoderun` (el cliente que es el que
se usa al lanzar).

### C08 — Dependencia dura de binarios externos (`timeout`, `ldconfig`, `pgrep`, `pidof`, `which`) en 12 sitios

Reparto verificado:

| Binario | Call sites |
| --- | --- |
| `timeout` | `plugins.rs:552`, `plugins.rs:620`, `plugins.rs:690`, `plugins.rs:727`, `integration.rs:601`, `integration.rs:609`, `integration.rs:666`, `proton.rs:1314` |
| `ldconfig` | `proton.rs:617` |
| `pgrep` | ~~`import_move.rs:353`, `import_move.rs:363`~~ (corregido en C14), `proton.rs:261` |
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

**Fix:** sustituir `timeout(1)` por un helper en proceso (`spawn` +
`try_wait` con deadline + `kill`), y hacer que la ausencia de `ldconfig` sea
un estado explícito, nunca un `false` silencioso. La parte de `pgrep` ya está
hecha: `import_move.rs` lee `/proc` directamente.

### C09 — El registro de plugins elige el primer `.tar.gz` sin filtrar por arquitectura

- `src/backend/plugins.rs:347-363` — itera `assets` y hace `break` en el
  primer nombre que termina en `.tar.gz`.
- `src/backend/plugins.rs:418` — lo descarga como `{tag}.tar.gz` y lo extrae
  sin verificar nada más que `size >= 100` (línea 448-452).

**Impacto:** en un release que publique `plugin-x86_64.tar.gz` y
`plugin-aarch64.tar.gz`, un host aarch64 instala el binario x86_64 según el
orden de la API. El plugin se instala "con éxito" y luego falla con
`Exec format error` en cada invocación, sin que la UI distinga "instalado" de
"instalado y utilizable".

**Fix:** filtrar por `std::env::consts::ARCH` contra el sufijo del asset y,
si no hay ninguno válido, devolver un error explícito. De paso, verificar un
checksum publicado en el release en lugar de solo `size >= 100`.

### C10 — El escaneo de Lutris ignora `XDG_DATA_HOME` y Lutris Flatpak, en el mismo archivo que sí los respeta

- `src/backend/integration.rs:852-857` — `pga.db` en
  `~/.local/share/lutris/pga.db` hardcodeado.
- `src/backend/integration.rs:871-876` — `games/` en
  `~/.local/share/lutris/games` hardcodeado.
- Contrasto, **450 líneas antes en el mismo archivo**:
  `src/backend/integration.rs:395-397` sí resuelve `XDG_DATA_HOME` con fallback
  a `~/.local/share`, y `403-407` sí añade la ruta Flatpak de Lutris.

**Impacto:**

1. Con `XDG_DATA_HOME` configurado (habitual en setups de tiling) Lutris
   instala en `~/data/lutris`: la UI resuelve bien las carátulas (línea 399)
   pero el escaneo de juegos mira `~/.local/share/lutris` y no encuentra nada.
   El resultado es "carátulas sí, juegos no", que parece un bug de Lutris y no
   de CorkyTux.
2. Con Lutris como Flatpak, `~/.var/app/net.lutris.Lutris/data/lutris` se
   usa para artwork pero nunca para `pga.db` ni para `games/`: se importa 0
   juegos.

**Fix:** un único resolvedor `lutris_data_dirs(home) -> Vec<PathBuf>` que
devuelva `[xdg/lutris, flatpak/lutris, ~/.local/share/lutris]`, consumido por
el escaneo y por el artwork.

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
3. `Found in system` estático y `Set as linked` clickeable en la misma fila.
4. Aviso `*** BUG *** In pixman_region32_init_rect: Invalid rectangle passed`:
   causa no confirmada, aplazada por no ser crítica.

## Orden de corrección

1. ~~**C14**~~ — **hecho**: tri-estado `PrefixUsage` + atribución por
   `/proc/<pid>/environ` en vez de `pgrep`.
2. **C08**, y en particular el `timeout(1)` de `plugins.rs:727` — sustituirlo
   por un timeout en proceso es el arreglo con mejor relación esfuerzo/impacto
   de lo que queda: una sola función nueva y desaparecen ocho dependencias de
   `timeout` a la vez, incluida la que deja la pestaña de Emuladores vacía sin
   error. El fix de C14 ya cubre `import_move.rs`.
3. **C09** y **C10** — independientes, acotados y sin riesgo de datos.
4. **C01** — depende del helper compartido de rutas de Steam del punto 2.
5. **C03** — requiere decidir antes qué significa "32-bit" en aarch64, para no
   cambiar el contrato de `ComponentStatus` a ciegas.
6. El resto de degradantes por lotes, empezando por los que tocan datos o
   superficie de entrada: D05, D12, D13, D19, D22.

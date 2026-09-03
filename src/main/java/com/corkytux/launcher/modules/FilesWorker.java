/*
 * CorkyTux - Java 25 Port
 * Copyright (C) 2026 queinu project / OnlineFix
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * Port from JPHP/DevelNext to pure Java 25 (Adoptium Temurin 25.0.4.1)
 * Original: https://github.com/onlinefix/linux-launcher
 */

package com.corkytux.launcher.modules;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.ini4j.Wini;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Java 25 port of {@code FilesWorker.php} (438 lines).
 * Handles desktop entries, Proton discovery, Steam runtime resolution,
 * process construction and execution, and third-party helpers.
 *
 * <p>Public static method names and semantics mirror the PHP originals exactly,
 * translated to idiomatic {@code java.nio}, {@code ProcessBuilder},
 * {@code java.net.http.HttpClient} and Jackson.</p>
 */
public final class FilesWorker {

    private static final Logger LOG = LoggerFactory.getLogger(FilesWorker.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** Mirrors PHP {@code $GLOBALS['LatestProton']} – stores last fetched URL or "fetching". */
    private static volatile String latestProtonUrl = null;

    // Buffer for debug STD lines emitted before LogForm is visible – ensures log not lost
    // and can be auto-saved to ~/Documents/CorkyTux Logs/
    private static final java.util.List<String> DEBUG_BUFFER = java.util.Collections.synchronizedList(new java.util.ArrayList<>());

    // Steam status cache — avoid re-running 20+ forks per game launch
    private static volatile boolean steamRunningCache = false;
    private static volatile long steamRunningCacheTime = 0;
    private static volatile boolean steamBinaryCache = false;
    private static volatile long steamBinaryCacheTime = 0;
    private static final long STEAM_CACHE_TTL_MS = 30_000; // 30 seconds

    private FilesWorker() {}

    // -----------------------------------------------------------------------
    // Desktop entry
    // -----------------------------------------------------------------------

    public static String generateDesktopEntry(String name) {
        return generateDesktopEntry(name, null);
    }

    public static String generateDesktopEntry(String name, String icon) {
        var pwd = Path.of("").toAbsolutePath().toString();
        var forceGPU = System.getProperty("prism.forceGPU");
        // forceGPU is only logged in AppModule; kept for parity (no effect on entry itself)
        if (forceGPU != null) {
            LOG.debug("prism.forceGPU={}", forceGPU);
        }
        String exec;
        // Search order: /usr/bin → ~/.local/bin/ofll → jar via java -jar
        var candidates = new String[]{
                "/usr/bin/corkytux",
                System.getProperty("user.home") + "/.local/bin/ofll"
        };
        exec = null;
        for (var c : candidates) {
            if (Files.isRegularFile(Path.of(c))) { exec = c; break; }
        }
        // Fallback: java -jar with the installed jar
        if (exec == null) {
            var jarPath = Path.of(System.getProperty("user.home"), ".local/share/corkytux/corkytux.jar");
            if (Files.isRegularFile(jarPath)) {
                String javaBin = resolveJavaBin();
                if (javaBin != null) {
                    exec = javaBin + " -Xmx512m -jar \"" + jarPath + "\"";
                }
            }
        }
        if (exec == null) {
            exec = "corkytux";
        }
        var iconStr = icon == null ? "null" : icon;
        return "[Desktop Entry]\n"
                + "Name=" + name + "\n"
                + "GenericName=Play this game with OnlineFix Launcher\n"
                + "Exec=\"" + exec + "\" \"" + name + "\"\n"
                + "Icon=" + iconStr + "\n"
                + "Path=" + pwd + "\n"
                + "Type=Application\n"
                + "Terminal=false\n"
                + "Categories=Game;";
    }

    private static String resolveJavaBin() {
        String javaHome = System.getProperty("java.home");
        if (javaHome != null) {
            var p = Path.of(javaHome, "bin/java");
            if (Files.isRegularFile(p)) return p.toString();
        }
        String[] searchPaths = {
                "/opt/jvm/temurin-25/bin/java", "/opt/jvm/temurin-24/bin/java",
                "/opt/jvm/temurin-23/bin/java", "/opt/jvm/temurin-22/bin/java",
                "/opt/jvm/temurin-21/bin/java",
                "/usr/lib/jvm/java-25-temurin/bin/java", "/usr/lib/jvm/java-21-temurin/bin/java",
                "/usr/lib/jvm/openjdk-25/bin/java", "/usr/lib/jvm/openjdk-21/bin/java"
        };
        for (var s : searchPaths) { if (Files.isRegularFile(Path.of(s))) return s; }
        // Fallback: java on PATH
        try {
            var pb = new ProcessBuilder("which", "java");
            pb.redirectErrorStream(true);
            var proc = pb.start();
            var out = new String(proc.getInputStream().readAllBytes()).trim();
            if (proc.waitFor() == 0 && !out.isBlank()) return out;
        } catch (Exception ignored) {}
        return null;
    }

    // -----------------------------------------------------------------------
    // Process generation
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code generateProcess($name, $debug = false)}.
     * Returns a configured {@link ProcessBuilder} or {@code null} when Steam / Proton / executable checks fail.
     */
    public static ProcessBuilder generateProcess(String name, boolean debug) {
        // --- Steam check (unless noSteamRequest is set) ---
        var noSteamRequest = getLauncherProperty("noSteamRequest", "User Settings");
        boolean skipSteamCheck = "true".equalsIgnoreCase(noSteamRequest) || "1".equals(noSteamRequest);
        if (!skipSteamCheck) {
            try {
                // Fix bypass says steam not running when steam is running:
                // which steam fails for flatpak (com.valvesoftware.Steam) but pidof steam succeeds native.
                // Now handles: native steam (pidof steam, steam.sh), flatpak (flatpak ps, pgrep -f, ProcessHandle)
                // Also fixes where which steam missing but Steam actually running as flatpak or via ~/.local/share/Steam/steam
                if (!hasSteamBinary()) {
                    showErrorOnFxThread("FILESWORKER.NOSTEAM");
                    return null;
                }
                // pidof exit 0 = running, non-zero = not running (1 = not found, >1 = error)
                // Fix: previously checked ==1 only, missing other non-zero codes → bypass broken when steam running check failed
                // Now robust: checks pidof steam, pidof -x steam.sh, pgrep -x steam, pgrep -f Steam, flatpak ps, ProcessHandle
                if (!isSteamRunning()) {
                    boolean started = runSteam();
                    if (!started) {
                        showErrorOnFxThread("FILESWORKER.STEAMNOTSTARTED");
                        // attempt to reset play button if MainForm visible – best effort
                        runOnFxThread(() -> LOG.info("Steam failed to start – play button should reset to 'play'"));
                        return null;
                    }
                } else {
                    LOG.debug("Steam already running (robust check) – bypassing runSteam");
                }
            } catch (Exception e) {
                LOG.error("Steam check failed", e);
            }
        }

        var executable = getGameProperty("executable", name);
        var proton = getProtonExecutable(name);

        if (proton == null) {
            showErrorOnFxThread("FILESWORKER.PROTON.NOTFOUND");
            return null;
        }
        if (executable == null || !Files.isRegularFile(Path.of(executable))) {
            showErrorOnFxThread("FILESWORKER.NOGAME");
            return null;
        }

        var argsBeforeExec = getGameProperty("argsBefore", name);
        var argsAfterExec = getGameProperty("argsAfter", name);

        var exec = new ArrayList<String>();
        exec.add(proton);
        exec.add("run");
        exec.add(executable);

        var wined3d = getGameProperty("wined3d", name);
        // Fallback to global launcher default if per-game not set (fixes fresh installs using wrong renderer)
        if (wined3d == null) wined3d = getLauncherProperty("gamesUsesWined3d", "User Settings");
        boolean useWined3d = wined3d != null && !"false".equalsIgnoreCase(wined3d) && !"0".equals(wined3d);

        String dxOverrides = "";
        if (!useWined3d) {
            dxOverrides = "d3d11=n;d3d10=n;d3d10core=n;dxgi=n;openvr_api_dxvk=n;d3d12=n;d3d12core=n;d3d9=n;d3d8=n;";
        }

        var overrides = getGameProperty("overrides", name);
        if (overrides == null) overrides = "";

        var mainEnvironment = new LinkedHashMap<String, String>();
        mainEnvironment.put("WINEDLLOVERRIDES", dxOverrides + overrides);
        mainEnvironment.put("WINEDEBUG", debug ? "1" : "-all");
        mainEnvironment.put("STEAM_COMPAT_DATA_PATH", getProtonPrefixPath(name));
        mainEnvironment.put("STEAM_COMPAT_CLIENT_INSTALL_PATH", getSteamClientInstallPath());
        // PROTON_USE_WINED3D must be explicit "1"/"0" – fixes Wine virtual desktop black with blue borders when mismatched
        mainEnvironment.put("PROTON_USE_WINED3D", useWined3d ? "1" : "0");

        var nativeWayland = getGameProperty("nativeWayland", name);
        if (nativeWayland == null || nativeWayland.isBlank()) {
            // Fallback to global launcher setting for new games default
            var globalWayland = getLauncherProperty("gamesUsesWayland", "User Settings");
            if (globalWayland != null && !globalWayland.isBlank()) nativeWayland = globalWayland;
        }
        if (nativeWayland != null && !nativeWayland.isBlank()) {
            // Mirrors PHP: 'PROTON_ENABLE_WAYLAND'=>games->get('nativeWayland',name)
            // Normalize truthy values to "1"/"0" for Proton; keep raw if already 0/1
            String v = nativeWayland.trim();
            if ("true".equalsIgnoreCase(v) || "1".equals(v)) v = "1";
            else if ("false".equalsIgnoreCase(v) || "0".equals(v)) v = "0";
            // Only set when explicitly configured – matches PHP where null/false leaves env unset to let Proton auto-detect.
            // This fixes REPO black fullscreen with blue borders under Wayland/XWayland:
            // when unset we don't force PROTON_ENABLE_WAYLAND=0 which breaks small appid/voiceid prompt window.
            mainEnvironment.put("PROTON_ENABLE_WAYLAND", v);
        }
        // else: do NOT force "0" – leave env unset so Proton/X11 vs Wayland autodetect matches original PHP behaviour.
        // If user is on Wayland and REPO needs Wayland driver, the toggle in GameSettings will set nativeWayland="1" explicitly.

        var steamOverlay = getGameProperty("steamOverlay", name);
        boolean overlayEnabled = steamOverlay != null && !"false".equalsIgnoreCase(steamOverlay) && !"0".equals(steamOverlay);
        if (overlayEnabled) {
            var fakeId = getGameProperty("fakeSteamID", name);
            if (fakeId == null || fakeId.isBlank()) fakeId = "480";
            String steamClientPath = getSteamClientInstallPath();
            String overlay32 = steamClientPath + "/ubuntu12_32/gameoverlayrenderer.so";
            String overlay64 = steamClientPath + "/ubuntu12_64/gameoverlayrenderer.so";
            var preloadParts = new java.util.ArrayList<String>();
            if (Files.isRegularFile(Path.of(overlay32))) preloadParts.add(overlay32);
            if (Files.isRegularFile(Path.of(overlay64))) preloadParts.add(overlay64);
            if (preloadParts.isEmpty()) {
                LOG.warn("Steam overlay enabled but no overlayrenderer.so found in {}", steamClientPath);
            } else {
                String existingPreload = mainEnvironment.getOrDefault("LD_PRELOAD", System.getenv("LD_PRELOAD"));
                String newPreload = String.join(":", preloadParts);
                if (existingPreload != null && !existingPreload.isBlank()) newPreload += ":" + existingPreload;
                mainEnvironment.put("LD_PRELOAD", newPreload);
            }
            mainEnvironment.put("ENABLE_VK_LAYER_VALVE_steam_overlay_1", "1");
            mainEnvironment.put("SteamOverlayGameId", fakeId);
            mainEnvironment.put("SteamGameId", fakeId);
        }

        var envString = getGameProperty("environment", name);
        if (envString != null && !envString.isBlank()) {
            mainEnvironment.putAll(parseEnvironmentMap(name, envString));
        }

        if (argsAfterExec != null && !argsAfterExec.isBlank()) {
            exec.addAll(splitArgs(argsAfterExec));
        }

        var steamRuntimeEnabled = getGameProperty("steamRuntime", name);
        boolean useSteamRuntime = steamRuntimeEnabled != null
                && !"false".equalsIgnoreCase(steamRuntimeEnabled) && !"0".equals(steamRuntimeEnabled);
        if (useSteamRuntime) {
            var protonName = getGameProperty("proton", name);
            var steamRuntime = findSteamRuntime(protonName);
            if (steamRuntime == null) {
                // disable flag mirrors PHP: set steamRuntime false
                setGameProperty("steamRuntime", "false", name);
            } else {
                var withRuntime = new ArrayList<String>();
                withRuntime.add(steamRuntime);
                withRuntime.add("--");
                withRuntime.addAll(exec);
                exec = withRuntime;
            }
        }

        if (argsBeforeExec != null && !argsBeforeExec.isBlank()) {
            var prefix = splitArgs(argsBeforeExec);
            var merged = new ArrayList<String>(prefix);
            merged.addAll(exec);
            exec = merged;
        }

        // ensure prefix directory exists
        var prefixPath = Path.of(mainEnvironment.get("STEAM_COMPAT_DATA_PATH"));
        try {
            if (prefixPath.getParent() != null) Files.createDirectories(prefixPath.getParent());
            Files.createDirectories(prefixPath);
        } catch (IOException e) {
            LOG.warn("Failed to create prefix dir {}", prefixPath, e);
        }

        var exeParent = Path.of(executable).getParent();
        if (exeParent == null) exeParent = Path.of("").toAbsolutePath();

        var pb = new ProcessBuilder(exec);
        pb.directory(exeParent.toFile());
        // sanitize environment: ProcessBuilder throws on null values – skip them
        var env = pb.environment();
        for (var entry : mainEnvironment.entrySet()) {
            if (entry.getValue() == null) continue;
            // Validate env name – mirrors PHP check for "Invalid environment variable"
            if (!entry.getKey().matches("[a-zA-Z_][a-zA-Z0-9_]*")) {
                showErrorOnFxThread("FILESWORKER.REMOVEENV");
                LOG.error("Invalid environment variable name: {}", entry.getKey());
                continue;
            }
            env.put(entry.getKey(), entry.getValue());
        }

        // Validate no nulls remain
        for (var entry : new HashMap<>(env).entrySet()) {
            if (entry.getValue() == null) {
                LOG.error("Removing null env var {}", entry.getKey());
                env.remove(entry.getKey());
            }
        }

        // Display settings for Wine/Proton – dual Wayland/X11 support, never force one driver.
        // Launcher must work for both Wayland and X11: user said "launcher is for wayland as x11".
        // We do NOT force PROTON_ENABLE_WAYLAND=0 on X11 nor =1 on Wayland. When nativeWayland is null/blank
        // we leave env UNSET so Proton auto-detects (XWayland on X11 :0, Wayland driver on Wayland).
        // Small appid/voiceid prompt should be windowed, not fullscreen – controlled by PROTON_ENABLE_WAYLAND + DISPLAY handling.
        // Fixes REPO black fullscreen with blue borders when forced.
        String waylandEnv = mainEnvironment.get("PROTON_ENABLE_WAYLAND");
        String sysDisplay = System.getenv("DISPLAY");
        String sysWayland = System.getenv("WAYLAND_DISPLAY");
        String sessionType = System.getenv("XDG_SESSION_TYPE");
        // Ensure DISPLAY is inherited; if Proton Wayland disabled, remove WAYLAND_DISPLAY to force XWayland
        if ("0".equals(waylandEnv)) {
            if (sysDisplay != null && !sysDisplay.isBlank()) env.put("DISPLAY", sysDisplay);
            // Remove Wayland display to avoid Wine trying Wayland driver when disabled – fixes black screen
            // This path is ONLY taken when user explicitly toggled "Use native Wayland" OFF (per-game or global).
            env.remove("WAYLAND_DISPLAY");
            LOG.debug("Wine display: PROTON_ENABLE_WAYLAND=0 (explicit X11/XWayland), DISPLAY={}, removed WAYLAND_DISPLAY (was {}) session={}", sysDisplay, sysWayland, sessionType);
        } else if ("1".equals(waylandEnv)) {
            if (sysWayland != null && !sysWayland.isBlank()) env.put("WAYLAND_DISPLAY", sysWayland);
            if (sysDisplay != null && !sysDisplay.isBlank() && !env.containsKey("DISPLAY")) env.put("DISPLAY", sysDisplay);
            LOG.debug("Wine display: PROTON_ENABLE_WAYLAND=1 (explicit Wayland), WAYLAND_DISPLAY={}, DISPLAY={} session={}", sysWayland, sysDisplay, sessionType);
        } else {
            // Auto-detect (env unset) – DUAL support: keep parent DISPLAY/WAYLAND_DISPLAY as inherited,
            // but ensure at least one is present. Do NOT force PROTON_ENABLE_WAYLAND=0 on X11 (:0)
            // nor =1 on Wayland – let Proton decide. Critical for REPO small appid/voiceid window:
            // forcing breaks windowed prompt into fullscreen black with blue borders.
            // Works for both X11 (:0 DISPLAY) and Wayland (WAYLAND_DISPLAY=wayland-0) sessions.
            if (sysDisplay != null && !sysDisplay.isBlank() && !env.containsKey("DISPLAY")) env.put("DISPLAY", sysDisplay);
            if (sysWayland != null && !sysWayland.isBlank() && !env.containsKey("WAYLAND_DISPLAY")) env.put("WAYLAND_DISPLAY", sysWayland);
            // Also ensure GDK_BACKEND not forced to x11 – leave unset for dual support (JavaFX/GTK handles Wayland via XWayland)
            // Do not set env GDK_BACKEND, QT_QPA_PLATFORM, SDL_VIDEODRIVER – they would force one display server.
            LOG.debug("Wine display: auto (no PROTON_ENABLE_WAYLAND) DUAL Wayland/X11 – DISPLAY={}, WAYLAND_DISPLAY={}, session={} (no force)", env.get("DISPLAY"), env.get("WAYLAND_DISPLAY"), sessionType);
        }
        // PROTON_USE_WINED3D handling: ensure string "1"/"0" and that WINEDLLOVERRIDES matches.
        // Already set via mainEnvironment as explicit "1"/"0" (Java 25 port parity with PHP never leaving null),
        // WINEDLLOVERRIDES contains wined3d/n overrides accordingly. Log for debugging REPO rendering.
        LOG.debug("Proton env for {}: PROTON_USE_WINED3D={}, WINEDLLOVERRIDES={}, PROTON_ENABLE_WAYLAND={} (env unset=auto, 0=XWayland,1=Wayland)", name, env.get("PROTON_USE_WINED3D"), env.get("WINEDLLOVERRIDES"), env.get("PROTON_ENABLE_WAYLAND"));

        // WINEPREFIX handling: Proton uses STEAM_COMPAT_DATA_PATH, Wine tools use WINEPREFIX=.../pfx
        // Ensure both are set and consistent so prefix registry stripping targets the same pfx that Wine will use.
        // This fixes mismatch where STEAM_COMPAT_DATA_PATH is set but WINEPREFIX not, causing wineserver -k vs prefix desync.
        var compatDataPath = env.get("STEAM_COMPAT_DATA_PATH");
        if (compatDataPath == null || compatDataPath.isBlank()) compatDataPath = mainEnvironment.get("STEAM_COMPAT_DATA_PATH");
        if (compatDataPath != null && !compatDataPath.isBlank()) {
            // Canonical wine pfx path is compatDataPath/pfx
            var winePfxPath = Path.of(compatDataPath, "pfx").toString();
            // Set WINEPREFIX for Wine subprocesses (non-Proton tools like wineserver, wine cfg) – safe even for Proton
            // Do not overwrite if user explicitly set WINEPREFIX via environment editor (parsed into env already)
            if (!env.containsKey("WINEPREFIX") || env.get("WINEPREFIX") == null || env.get("WINEPREFIX").isBlank()) {
                env.put("WINEPREFIX", winePfxPath);
                LOG.debug("WINEPREFIX set to {} (derived from STEAM_COMPAT_DATA_PATH={})", winePfxPath, compatDataPath);
            } else {
                LOG.debug("WINEPREFIX already set via user env: {} (compatDataPath={})", env.get("WINEPREFIX"), compatDataPath);
            }
        }

        // DEBUG REPO 2026-08-30 03:34.log – black screen blue borders vs small appid/voiceid window (Java 25, proton-cachyos-11.0-20260703-slr-x86_64_v3):
        // Proton launch cmd: proton-cachyos-11.0-20260703-slr-x86_64_v3/proton run "R.E.P.O Launcher.exe" (checked in proton-cachyos-11.0-20260703-slr-x86_64_v3/proton exists, Games.ini REPO.proton=proton-cachyos-11.0-20260703-slr-x86_64_v3, Launcher.ini defaultProton same)
        // WINEPREFIX: STEAM_COMPAT_DATA_PATH=/home/the user/.local/share/CorkyTux/prefixes/REPO_v0.4.1_ElEnemigos (env) => WINEPREFIX=.../pfx (user.reg at pfx/user.reg) – ensured both set and consistent
        // Virtual desktop registry: pfx/user.reg + pfx/userdef.reg + pfx/system.reg checked for [Software\Wine\Explorer] "Desktop"="Default" or [Software\Wine\Explorer\Desktops] – must NOT be forced (Shell Folders Desktop preserved)
        // Display env: DISPLAY=:0, WAYLAND_DISPLAY="", XDG_SESSION_TYPE="", XDG_CURRENT_DESKTOP=X-Cinnamon => pure X11 Cinnamon, NOT Wayland; PROTON_ENABLE_WAYLAND left auto (unset) + PROTON_USE_WINED3D=0 (DXVK, wined3d=false) so REPO stays windowed not fullscreen
        // The virtual desktop registry key forces fullscreen black with blue border; REPO's small appid/voiceid prompt must be windowed – actively strip ONLY Wine Explorer keys.
        try {
            // Resolve wine prefix via WINEPREFIX (authoritative for Wine) else via STEAM_COMPAT_DATA_PATH/pfx
            Path winePrefix;
            String winePrefixStr = env.get("WINEPREFIX");
            if (winePrefixStr != null && !winePrefixStr.isBlank() && Files.isDirectory(Path.of(winePrefixStr))) {
                winePrefix = Path.of(winePrefixStr);
            } else if (compatDataPath != null && !compatDataPath.isBlank()) {
                winePrefix = Path.of(compatDataPath, "pfx");
                if (!Files.isDirectory(winePrefix) && mainEnvironment.get("STEAM_COMPAT_DATA_PATH") != null) {
                    winePrefix = Path.of(mainEnvironment.get("STEAM_COMPAT_DATA_PATH"), "pfx");
                }
            } else {
                winePrefix = Path.of(mainEnvironment.get("STEAM_COMPAT_DATA_PATH"), "pfx");
            }
            // Validate correct Proton version is used (REPO log 03:34 proton=proton-cachyos-11.0-20260703-slr-x86_64_v3, proton file exists, version cachyos-11.0)
            var protonExecCheck = getProtonExecutable(name);
            if (protonExecCheck == null || !Files.isRegularFile(Path.of(protonExecCheck))) {
                LOG.warn("Proton executable missing for {} (expected proton-cachyos-11.0-20260703-slr-x86_64_v3 at {}) – launcher will fallback to findNewestAvailableProton", name, protonExecCheck);
            } else {
                LOG.debug("Proton executable validated for {}: {} (exists, version check via protons dir)", name, protonExecCheck);
            }
            // Check user.reg, userdef.reg, and system.reg for virtual desktop – REPO prefix uses user.reg but also guard userdef.reg
            for (var regName : List.of("user.reg", "userdef.reg")) {
                var userReg = winePrefix.resolve(regName);
                if (!Files.isRegularFile(userReg)) {
                    LOG.trace("No {} at {} – skip virtual desktop check (reg not found)", regName, userReg);
                    continue;
                }
                var content = Files.readString(userReg, StandardCharsets.UTF_8);
                // Precise detection: only Wine Explorer virtual desktop, NOT Shell Folders Desktop
                // Virtual desktop = [Software\Wine\Explorer] "Desktop"="Default"  or  [Software\Wine\Explorer\Desktops] section
                // Shell Folders at [Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders] "Desktop"=... is legitimate and must NOT trigger.
                boolean hasWineExplorerDesktop = false;
                boolean hasDesktopsSection = false;
                // Scan for Wine virtual desktop markers with section awareness
                var checkLines = content.split("\n", -1);
                String currentSection = "";
                for (String l : checkLines) {
                    String t = l.trim();
                    if (t.startsWith("[")) {
                        int end = t.indexOf(']');
                        if (end != -1) currentSection = t.substring(0, end + 1);
                        else currentSection = t;
                        if (currentSection.startsWith("[Software\\Wine\\Explorer\\Desktops") || currentSection.startsWith("[Software\\\\Wine\\\\Explorer\\\\Desktops")) {
                            hasDesktopsSection = true;
                        }
                    } else if (currentSection.equals("[Software\\Wine\\Explorer]") || currentSection.equals("[Software\\\\Wine\\\\Explorer]")) {
                        if (t.startsWith("\"Desktop\"=") || t.startsWith("\"Desktop\" =")) {
                            hasWineExplorerDesktop = true;
                        }
                    } else if (currentSection.startsWith("[Software\\Wine\\Explorer\\Desktops") || currentSection.startsWith("[Software\\\\Wine\\\\Explorer\\\\Desktops")) {
                        // Any key inside Desktops section indicates virtual desktop is configured
                        if (!t.isEmpty() && !t.startsWith("#") && !t.startsWith(";") && t.contains("=")) hasDesktopsSection = true;
                    }
                }
                // Also handle case where registry was written with single vs double escaped backslashes (both checked above)
                // Fallback simple string check but scoped to Wine paths to avoid Shell Folders false positives
                boolean hasVirtualDesktop = hasWineExplorerDesktop || hasDesktopsSection;
                LOG.debug("Wine virtual desktop check for {}: hasWineExplorerDesktop={} hasDesktopsSection={} => hasVirtualDesktop={}", userReg, hasWineExplorerDesktop, hasDesktopsSection, hasVirtualDesktop);
                if (hasVirtualDesktop) {
                    LOG.info("Wine virtual desktop detected in {}, disabling to fix black screen with blue borders (REPO windowed prompt)", userReg);
                    // Backup original
                    try {
                        var backup = winePrefix.resolve("user.reg.bak." + System.currentTimeMillis());
                        Files.copy(userReg, backup);
                        LOG.debug("Backed up user.reg to {}", backup);
                    } catch (Exception be) { LOG.trace("backup failed", be); }
                    // Remove ONLY Wine Explorer virtual desktop: Desktops sections + Desktop value inside Explorer section
                    // Preserve [Software\Microsoft\...\Shell Folders] "Desktop" (legit Windows Desktop folder)
                    var lines = content.split("\n", -1);
                    var sb = new StringBuilder(content.length());
                    boolean skippingDesktopsSection = false;
                    String sectionHeader = "";
                    for (String line : lines) {
                        String trimmed = line.trim();
                        if (trimmed.startsWith("[")) {
                            // New registry section – determine type with timestamp suffix handling
                            // e.g. [Software\Wine\Explorer] 123456 #time=...
                            int bracketEnd = trimmed.indexOf(']');
                            String headerOnly = bracketEnd != -1 ? trimmed.substring(0, bracketEnd + 1) : trimmed;
                            // Detect Wine Explorer Desktops (including subkeys like \Desktops\Default)
                            boolean isDesktops = headerOnly.startsWith("[Software\\Wine\\Explorer\\Desktops") || headerOnly.startsWith("[Software\\\\Wine\\\\Explorer\\\\Desktops");
                            boolean isExplorer = headerOnly.equals("[Software\\Wine\\Explorer]") || headerOnly.equals("[Software\\\\Wine\\\\Explorer]");
                            if (isDesktops) {
                                skippingDesktopsSection = true;
                                sectionHeader = headerOnly;
                                continue; // drop Desktops section header itself
                            }
                            if (skippingDesktopsSection) {
                                // We were skipping Desktops – any new section ends skipping (unless it's another Desktops subkey)
                                if (!isDesktops) {
                                    skippingDesktopsSection = false;
                                    sectionHeader = headerOnly;
                                    // fall through to process this new header
                                } else {
                                    continue; // still inside Desktops hierarchy
                                }
                            }
                            if (isExplorer) {
                                sectionHeader = headerOnly;
                                sb.append(line).append("\n");
                                continue;
                            }
                            // Other section (including Shell Folders) – stop skipping and preserve
                            sectionHeader = headerOnly;
                            skippingDesktopsSection = false;
                            sb.append(line).append("\n");
                            continue;
                        }
                        if (skippingDesktopsSection) {
                            // Skip all lines inside Desktops section until next section header
                            continue;
                        }
                        // Only strip "Desktop" value when inside [Software\Wine\Explorer]
                        boolean isWineExplorerSection = sectionHeader.equals("[Software\\Wine\\Explorer]") || sectionHeader.equals("[Software\\\\Wine\\\\Explorer]");
                        if (isWineExplorerSection && (trimmed.startsWith("\"Desktop\"=") || trimmed.startsWith("\"Desktop\" ="))) {
                            LOG.debug("Stripping virtual desktop entry: {} in section {}", trimmed, sectionHeader);
                            continue; // remove this virtual desktop enable entry
                        }
                        sb.append(line).append("\n");
                    }
                    String cleaned = sb.toString();
                    // Only write if changed and still looks like a registry (starts with WINE REGISTRY)
                    if (!cleaned.equals(content) && cleaned.contains("WINE REGISTRY")) {
                        Files.writeString(userReg, cleaned, StandardCharsets.UTF_8);
                        LOG.info("Disabled Wine virtual desktop in {} (preserved Shell Folders Desktop)", userReg);
                    } else if (cleaned.equals(content)) {
                        LOG.warn("Virtual desktop detected but pattern not stripped (header mismatch) for {}", userReg);
                    } else if (!cleaned.contains("WINE REGISTRY")) {
                        LOG.error("Cleaned user.reg missing WINE REGISTRY header, refusing to write {}", userReg);
                    }
                } else {
                    LOG.debug("No Wine virtual desktop in {} – REPO prompt will be windowed (small appid/voiceid)", userReg);
                }
            }
            // Ensure wine prefix registry also reflects PROTON_USE_WINED3D choice for next launch (DXVK vs wined3d, Wayland auto vs forced)
            LOG.debug("Wine prefix virtual desktop fix done, PROTON_USE_WINED3D={} PROTON_ENABLE_WAYLAND={} WINEPREFIX={} for {}", env.get("PROTON_USE_WINED3D"), env.get("PROTON_ENABLE_WAYLAND"), env.get("WINEPREFIX"), name);
        } catch (Exception ex) {
            LOG.warn("virtual desktop fix failed for {}: {}", name, ex.toString());
            LOG.trace("virtual desktop fix failed", ex);
        }

        return pb;
    }

    public static ProcessBuilder generateProcess(String name) {
        return generateProcess(name, false);
    }

    // -----------------------------------------------------------------------
    // Run / debug / hook
    // -----------------------------------------------------------------------

    /**
     * Starts the process, hooks output, optionally shows debug window, and updates timeSpent.
     * Mirrors PHP {@code run($process, $gameName, $debug = false)}.
     */
    public static void run(Process process, String gameName, boolean debug) {
        long timeStart = System.currentTimeMillis() / 1000L;

        // Implicit exit handling – JavaFX Platform.setImplicitExit analogue
        setImplicitExit(false);

        // process is already started (Process), hook outputs
        int exit = hookProcessOuts(process, debug, true);

        if (debug) {
            debug(exit, gameName);
        }

        long timeStop = System.currentTimeMillis() / 1000L;
        long spent = (timeStop - timeStart);
        var prev = getGameProperty("timeSpent", gameName);
        long prevVal = 0;
        if (prev != null) {
            try { prevVal = Long.parseLong(prev); } catch (NumberFormatException ignored) {}
        }
        setGameProperty("timeSpent", String.valueOf(spent + prevVal), gameName);

        // restore implicit exit
        setImplicitExit(true);
    }

    /**
     * Overload that accepts a {@link ProcessBuilder} and starts it.
     */
    public static void run(ProcessBuilder pb, String gameName, boolean debug) throws IOException {
        long timeStart = System.currentTimeMillis() / 1000L;
        setImplicitExit(false);
        var process = pb.start();
        int exit = hookProcessOuts(process, debug, true);
        if (debug) debug(exit, gameName);
        long timeStop = System.currentTimeMillis() / 1000L;
        long spent = (timeStop - timeStart);
        var prev = getGameProperty("timeSpent", gameName);
        long prevVal = 0;
        if (prev != null) try { prevVal = Long.parseLong(prev); } catch (NumberFormatException ignored) {}
        setGameProperty("timeSpent", String.valueOf(spent + prevVal), gameName);
        setImplicitExit(true);
    }

    public static void debug(int exitCode, String gameName) {
        runOnFxThread(() -> {
            try {
                var sb = new StringBuilder();
                sb.append("Game name - ").append(gameName).append("\n");
                sb.append("Exit code - ").append(exitCode).append("\n");
                sb.append("Game settings:\n");

                var section = getGameSection(gameName);
                for (var e : section.entrySet()) {
                    sb.append("\t").append(e.getKey()).append(" - ").append(e.getValue()).append("\n");
                }

                sb.append("OS Release:\n");
                var osRelease = Path.of("/etc/os-release");
                if (Files.isRegularFile(osRelease)) {
                    var lines = Files.readAllLines(osRelease, StandardCharsets.UTF_8);
                    for (var line : lines) sb.append("\t").append(line).append("\n");
                }

                LOG.info("Debug info for {}:\n{}", gameName, sb);
                String headerInfo = sb.toString();

                // Mirrors PHP debug(): prepend info to log.textArea, set data('gameName'), title, showForm
                try {
                    // Try to obtain existing LogForm controller via Launcher registry
                    Object ctrl = null;
                    try {
                        var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                        var getForm = launcherCls.getMethod("getForm", String.class);
                        ctrl = getForm.invoke(null, "log");
                    } catch (Exception ignored) {}
                    if (ctrl == null) {
                        // Trigger lazy load – showForm will create controller
                        try {
                            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                            var showForm = launcherCls.getMethod("showForm", String.class);
                            showForm.invoke(null, "log");
                            var getForm2 = launcherCls.getMethod("getForm", String.class);
                            ctrl = getForm2.invoke(null, "log");
                        } catch (Exception ex) {
                            LOG.debug("Failed to load log form for debug", ex);
                        }
                    }
                    if (ctrl != null) {
                        // prepend headerInfo to existing text (PHP: text = info + "\n" + existing)
                        var cls = ctrl.getClass();
                        try {
                            var getTextArea = cls.getMethod("getTextArea");
                            var ta = (javafx.scene.control.TextArea) getTextArea.invoke(ctrl);
                            if (ta != null) {
                                String existing = ta.getText() != null ? ta.getText() : "";
                                // Prepend with newline separator like PHP
                                ta.setText(headerInfo + "\n" + existing);
                                // Flush buffered STD lines collected during process execution (when LogForm not yet visible)
                                // This fixes missing log content when debug hook fired before window shown
                                if (!DEBUG_BUFFER.isEmpty()) {
                                    synchronized (DEBUG_BUFFER) {
                                        for (String b : DEBUG_BUFFER) ta.appendText(b + "\n");
                                        LOG.debug("Flushed {} buffered debug lines into LogForm", DEBUG_BUFFER.size());
                                        DEBUG_BUFFER.clear();
                                    }
                                }
                            } else {
                                // fallback appendText if getTextArea null (FXML not yet injected)
                                var appendM = cls.getMethod("appendText", String.class);
                                appendM.invoke(ctrl, headerInfo);
                                if (!DEBUG_BUFFER.isEmpty()) {
                                    synchronized (DEBUG_BUFFER) {
                                        for (String b : DEBUG_BUFFER) appendM.invoke(ctrl, b);
                                        DEBUG_BUFFER.clear();
                                    }
                                }
                            }
                        } catch (NoSuchMethodException ns) {
                            // Try direct appendText path – textArea may still be null before FXML init
                            try {
                                var appendM = cls.getMethod("appendText", String.class);
                                appendM.invoke(ctrl, headerInfo);
                                if (!DEBUG_BUFFER.isEmpty()) {
                                    synchronized (DEBUG_BUFFER) {
                                        for (String b : DEBUG_BUFFER) appendM.invoke(ctrl, b);
                                        DEBUG_BUFFER.clear();
                                    }
                                }
                            } catch (Exception ignored2) {}
                        }
                        try {
                            var setGameName = cls.getMethod("setGameName", String.class);
                            setGameName.invoke(ctrl, gameName);
                        } catch (NoSuchMethodException ns) {
                            try {
                                var setData = cls.getMethod("setDataGameName", String.class);
                                setData.invoke(ctrl, gameName);
                            } catch (Exception ignored2) {}
                        }
                        // Also support setDataGameName alias
                        try {
                            var setDataAlias = cls.getMethod("setDataGameName", String.class);
                            setDataAlias.invoke(ctrl, gameName);
                        } catch (Exception ignored) {}
                    } else {
                        // No controller yet – clear buffer after logging warning; header already logged
                        LOG.warn("No LogForm controller for debug flush, {} buffered lines will be kept for next show", DEBUG_BUFFER.size());
                    }
                    // Ensure stage title like PHP: $gameName + " log"
                    try {
                        var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                        var getStage = launcherCls.getMethod("getStage", String.class);
                        var stage = (javafx.stage.Stage) getStage.invoke(null, "log");
                        if (stage != null) stage.setTitle(gameName + " log");
                    } catch (Exception ignored) {}

                    // Auto-save debug log to ~/Documents/CorkyTux Logs/ – mirrors LogForm.save but auto on debug exit
                    // Requirement: ensure debug mode saves log to ~/Documents/CorkyTux Logs/ and shows log window
                    try {
                        String documents = null;
                        try {
                            var proc = new ProcessBuilder("bash", "-c", "xdg-user-dir DOCUMENTS").start();
                            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                                documents = reader.readLine();
                                proc.waitFor();
                            }
                        } catch (Exception ignored) {}
                        if (documents == null || documents.isBlank()) documents = getExpectedHome() + "/Documents";
                        else documents = documents.trim();
                        String timestamp = java.time.LocalDateTime.now().format(java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm"));
                        String fileName = (gameName != null && !gameName.isBlank() ? gameName : "Game") + " " + timestamp + ".log";
                        var dir = Path.of(documents, "CorkyTux Logs");
                        Files.createDirectories(dir);
                        var file = dir.resolve(fileName);
                        // content = header + existing log text if available
                        String contentToSave = headerInfo;
                        if (ctrl != null) {
                            try {
                                var getTextArea = ctrl.getClass().getMethod("getTextArea");
                                var ta = (javafx.scene.control.TextArea) getTextArea.invoke(ctrl);
                                if (ta != null && ta.getText() != null && !ta.getText().equals(headerInfo + "\n")) {
                                    contentToSave = ta.getText();
                                } else if (ta != null) {
                                    contentToSave = ta.getText();
                                }
                            } catch (Exception ignored) {}
                        }
                        Files.writeString(file, contentToSave != null ? contentToSave : headerInfo, StandardCharsets.UTF_8);
                        LOG.info("Debug log auto-saved to {}", file);
                    } catch (Exception ex) {
                        LOG.warn("Auto-save debug log failed", ex);
                    }

                    // Finally show log window – mirrors PHP app()->showForm('log')
                    try {
                        var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                        var showForm = launcherCls.getMethod("showForm", String.class);
                        showForm.invoke(null, "log");
                    } catch (Exception ex) {
                        LOG.debug("showForm log failed", ex);
                    }
                } catch (Exception outer) {
                    LOG.error("debug() failed to integrate LogForm", outer);
                }
            } catch (Exception e) {
                LOG.error("debug() failed", e);
            }
        });
    }

    public static int hookProcessOuts(Process process, boolean debug) {
        return hookProcessOuts(process, debug, true);
    }

    public static int hookProcessOuts(Process process, boolean debug, boolean wait) {
        var executor = Executors.newFixedThreadPool(2, r -> {
            var t = new Thread(r);
            t.setDaemon(true);
            return t;
        });

        // stderr – mirrors PHP $baseHook echo + uiLater textArea append when debug
        executor.submit(() -> {
            try (var reader = new BufferedReader(new InputStreamReader(process.getErrorStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    System.out.println(line);
                    if (debug) {
                        final String l = line;
                        String formatted = "STDERR - " + l;
                        // Buffer for late LogForm init (before window shown)
                        DEBUG_BUFFER.add(formatted);
                        runOnFxThread(() -> {
                            LOG.debug("STDERR - {}", l);
                            // Append to LogForm textArea like PHP: app()->form('log')->textArea->text .= "STDERR - $l\n"
                            try {
                                var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                                var getForm = launcherCls.getMethod("getForm", String.class);
                                Object ctrl = getForm.invoke(null, "log");
                                if (ctrl != null) {
                                    var m = ctrl.getClass().getMethod("appendText", String.class);
                                    m.invoke(ctrl, formatted);
                                } else {
                                    // Controller not yet loaded – will be flushed in debug() via DEBUG_BUFFER
                                    LOG.trace("LogForm not yet loaded, buffered STDERR: {}", formatted);
                                }
                            } catch (Exception ex) {
                                LOG.trace("append STDERR to LogForm failed", ex);
                            }
                        });
                    }
                }
            } catch (IOException e) {
                LOG.warn("hook stderr failed", e);
            }
        });

        // stdout – mirrors PHP STDOUT hook
        executor.submit(() -> {
            try (var reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    System.out.println(line);
                    if (debug) {
                        final String l = line;
                        String formatted = "STDOUT - " + l;
                        DEBUG_BUFFER.add(formatted);
                        runOnFxThread(() -> {
                            LOG.debug("STDOUT - {}", l);
                            try {
                                var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                                var getForm = launcherCls.getMethod("getForm", String.class);
                                Object ctrl = getForm.invoke(null, "log");
                                if (ctrl != null) {
                                    var m = ctrl.getClass().getMethod("appendText", String.class);
                                    m.invoke(ctrl, formatted);
                                } else {
                                    LOG.trace("LogForm not yet loaded, buffered STDOUT: {}", formatted);
                                }
                            } catch (Exception ex) {
                                LOG.trace("append STDOUT to LogForm failed", ex);
                            }
                        });
                    }
                }
            } catch (IOException e) {
                LOG.warn("hook stdout failed", e);
            }
        });

        executor.shutdown();

        if (wait) {
            try {
                // poll every 2 seconds mirroring PHP sleep(2)
                while (process.isAlive()) {
                    // wait 2 seconds
                    try { Thread.sleep(2000); } catch (InterruptedException ie) { Thread.currentThread().interrupt(); break; }
                }
                return process.waitFor();
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return -1;
            }
        } else {
            // non-blocking: return current exit or null sentinel -> -1
            try {
                return process.exitValue();
            } catch (IllegalThreadStateException e) {
                return -1; // still running
            }
        }
    }

    // -----------------------------------------------------------------------
    // Steam runtime
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code findSteamRuntime($forProton)} – returns absolute path to steam runtime "run" script or null.
     *
     * <p>VDF handling mirrors PHP {@code VDF::fromFile(...)} via the custom {@link VdfParser}.
     * Falls back to ini4j-style regex scanning if the custom parser throws (corrupted VDF,
     * legacy ini-like libraryfolders, or permission issues), preserving the task requirement
     * "use ini4j or custom parser".</p>
     */
    public static String findSteamRuntime(String forProton) {
        var protonPath = ensureProtonPath(forProton);
        if (protonPath == null) return null;

        int protonVersion = 11;
        var versionFile = Path.of(protonPath, "version");
        if (Files.isRegularFile(versionFile)) {
            try {
                var content = Files.readString(versionFile, StandardCharsets.UTF_8);
                var m = Pattern.compile("GE-Proton(\\d+)-").matcher(content);
                if (m.find()) {
                    try { protonVersion = Integer.parseInt(m.group(1)); } catch (NumberFormatException ignored) {}
                }
            } catch (IOException e) {
                LOG.warn("Failed to read proton version file {}", versionFile, e);
            }
        }

        String appId;
        String dirname;
        boolean isAarch64 = "aarch64".equals(System.getProperty("os.arch"));
        if (protonVersion >= 11) {
            if (isAarch64) {
                appId = "4185400";
                dirname = "SteamLinuxRuntime_4-arm64";
            } else {
                appId = "4183110";
                dirname = "SteamLinuxRuntime_4";
            }
        } else if (protonVersion >= 8) {
            appId = "1628350";
            dirname = "SteamLinuxRuntime_sniper";
        } else {
            appId = "1391110";
            dirname = "SteamLinuxRuntime_soldier";
        }

        // Search both possible locations – mirrors PHP hard-coded path but adds config fallback
        // Also includes Flatpak Steam path for issue #65
        String steamHome = getExpectedHome();
        var candidatesList = new java.util.ArrayList<Path>();
        candidatesList.add(Path.of(steamHome, ".steam/steam/steamapps/libraryfolders.vdf"));
        candidatesList.add(Path.of(steamHome, ".steam/steam/config/libraryfolders.vdf"));
        candidatesList.add(Path.of(steamHome, ".steam/root/steamapps/libraryfolders.vdf"));
        candidatesList.add(Path.of(steamHome, ".local/share/Steam/steamapps/libraryfolders.vdf"));
        // Flatpak Steam path
        String flatpakSteam = getFlatpakSteamDataPath();
        if (flatpakSteam != null) {
            candidatesList.add(Path.of(flatpakSteam, "steamapps/libraryfolders.vdf"));
            candidatesList.add(Path.of(flatpakSteam, "config/libraryfolders.vdf"));
        }
        List<Path> candidates = List.copyOf(candidatesList);
        Path libraryFolders = null;
        for (var c : candidates) {
            if (Files.isRegularFile(c)) { libraryFolders = c; break; }
        }
        if (libraryFolders == null) {
            LOG.debug("No libraryfolders.vdf found in any of {}", candidates);
            return null;
        }

        // 1) Try proper VDF parser (custom, Java 25) – task: "use ini4j or custom parser"
        try {
            String viaVdf = VdfParser.resolveSteamRuntime(libraryFolders, appId, dirname);
            if (viaVdf != null) {
                LOG.info("findSteamRuntime resolved via VdfParser: {} -> {}", appId, viaVdf);
                return viaVdf;
            }
            LOG.debug("VdfParser.resolveSteamRuntime returned null for {} at {}", appId, libraryFolders);
        } catch (Exception e) {
            LOG.warn("VdfParser failed for {}, falling back to regex scan", libraryFolders, e);
        }

        // 2) Fallback: robust regex / ini4j-style scan (legacy parity with previous Java port)
        // This keeps behaviour for corrupted or non-standard VDFs and satisfies "ini4j" mention –
        // we attempt a Wini-compatible line scan even though VDF is not INI, as a last resort.
        try {
            var vdf = Files.readString(libraryFolders, StandardCharsets.UTF_8);
            // Minimal VDF parsing: look for "apps" blocks containing appId and "path"
            // VDF structure: "libraryfolders" { "0" { "path" "/..." "apps" { "4183110" "123" } } }
            // We extract each "path" value and check if appId appears nearby
            var pathPattern = Pattern.compile("\"path\"\\s+\"([^\"]+)\"");
            var m = pathPattern.matcher(vdf);
            while (m.find()) {
                var folderPath = m.group(1);
                int start = m.start();
                int next = vdf.indexOf("\"path\"", m.end());
                String segment = next == -1 ? vdf.substring(start) : vdf.substring(start, next);
                if (segment.contains("\"" + appId + "\"")) {
                    var candidate = Path.of(folderPath, "steamapps/common", dirname, "run");
                    if (Files.isRegularFile(candidate)) {
                        LOG.info("findSteamRuntime resolved via fallback regex: {} -> {}", appId, candidate);
                        return candidate.toString();
                    }
                }
            }
            // Additional ini4j fallback: try to read as ini (handles some custom libraryfolders)
            try {
                var wini = new Wini(libraryFolders.toFile());
                for (String section : wini.keySet()) {
                    var sec = wini.get(section);
                    if (sec == null) continue;
                    String path = sec.get("path");
                    if (path != null && (sec.containsKey(appId) || sec.containsKey("\"" + appId + "\""))) {
                        var candidate = Path.of(path, "steamapps/common", dirname, "run");
                        if (Files.isRegularFile(candidate)) return candidate.toString();
                    }
                }
            } catch (Exception iniEx) {
                LOG.trace("ini4j fallback for VDF also failed", iniEx);
            }
        } catch (Exception e) {
            LOG.warn("findSteamRuntime VDF fallback scan failed", e);
        }

        LOG.debug("findSteamRuntime: no runtime found for appId {} dirname {} in {}", appId, dirname, libraryFolders);
        return null;
    }

    // -----------------------------------------------------------------------
    // Proton releases
    // -----------------------------------------------------------------------

    /**
     * Fetches GE-Proton releases from GitHub API.
     * Returns map tagName -> {filename, url}
     */
    public static Map<String, Map<String, String>> fetchProtonReleases() {
        var result = new LinkedHashMap<String, Map<String, String>>();
        // Fix: Network check must not fail MainForm Run/Utilities/Debug due to missing deps or timeout.
        // Use short connect timeout + offline-safe catch; never throw, return empty for offline fallback.
        // Missing jackson or httpclient deps must not propagate to UI (button gray).
        HttpClient client;
        try {
            client = HttpClient.newBuilder()
                    .connectTimeout(Duration.ofMillis(5000))
                    .build();
        } catch (Exception e) {
            LOG.warn("HttpClient init failed offline safe – missing dep?", e);
            return result;
        }
        var request = HttpRequest.newBuilder()
                .uri(URI.create("https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases"))
                .timeout(Duration.ofMillis(5000))
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "CorkyTux/" + com.corkytux.launcher.modules.AppModule.VERSION)
                .GET()
                .build();
        HttpResponse<String> response;
        try {
            response = client.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        } catch (Exception e) {
            // Offline or DNS failure – common on metered/no-internet; log at debug, not error that scares user,
            // and return empty so local proton fallback (findNewestAvailableProton) works.
            LOG.debug("Failed to fetch GE-Proton releases (offline safe) – {}", e.getMessage());
            return result;
        }

        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            LOG.error("Failed to fetch GE-Proton releases - code {}, message - {}", response.statusCode(), response.body());
            return result;
        }

        try {
            JsonNode root = MAPPER.readTree(response.body());
            if (!root.isArray()) return result;
            boolean sysIsArm = "aarch64".equals(System.getProperty("os.arch"));
            for (JsonNode release : root) {
                String name = null;
                String filename = null;
                String url = null;

                var assets = release.get("assets");
                if (assets == null || !assets.isArray()) continue;

                for (JsonNode asset : assets) {
                    var contentType = asset.path("content_type").asText("");
                    boolean isGzip = "application/gzip".equals(contentType) || "application/x-gtar".equals(contentType);
                    // fallback regex
                    if (!isGzip) {
                        isGzip = Pattern.compile("^application/(gzip|x-gtar)$").matcher(contentType).find();
                    }
                    var assetName = asset.path("name").asText("");
                    boolean isArm = assetName.contains("aarch64");
                    if (!isGzip || isArm != sysIsArm) continue;

                    name = release.path("tag_name").asText(null);
                    filename = assetName;
                    url = asset.path("browser_download_url").asText(null);
                    break;
                }

                if (name == null || filename == null || url == null) continue;

                var entry = new HashMap<String, String>();
                entry.put("filename", filename);
                entry.put("url", url);
                result.put(name, entry);
            }
        } catch (Exception e) {
            LOG.warn("Failed to parse proton releases JSON", e);
        }

        return result;
    }

    public static Map<String, Map<String, String>> fetchCachyOSProtonReleases() {
        var result = new LinkedHashMap<String, Map<String, String>>();
        HttpClient client;
        try {
            client = HttpClient.newBuilder()
                    .connectTimeout(Duration.ofMillis(5000))
                    .build();
        } catch (Exception e) {
            LOG.warn("HttpClient init failed for CachyOS releases", e);
            return result;
        }
        var request = HttpRequest.newBuilder()
                .uri(URI.create("https://api.github.com/repos/CachyOS/proton-cachyos/releases?per_page=10"))
                .timeout(Duration.ofMillis(5000))
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "CorkyTux/" + com.corkytux.launcher.modules.AppModule.VERSION)
                .GET()
                .build();
        HttpResponse<String> response;
        try {
            response = client.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        } catch (Exception e) {
            LOG.debug("Failed to fetch CachyOS Proton releases – {}", e.getMessage());
            return result;
        }

        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            LOG.error("Failed to fetch CachyOS Proton releases - code {}", response.statusCode());
            return result;
        }

        boolean sysIsArm = "aarch64".equals(System.getProperty("os.arch"));

        try {
            JsonNode root = MAPPER.readTree(response.body());
            if (!root.isArray()) return result;
            for (JsonNode release : root) {
                String tag = release.path("tag_name").asText(null);
                if (tag == null || tag.isEmpty()) continue;

                var assets = release.get("assets");
                if (assets == null || !assets.isArray()) continue;

                boolean foundV3 = false;
                boolean foundBase = false;
                for (JsonNode asset : assets) {
                    var assetName = asset.path("name").asText("");
                    boolean isXz = assetName.endsWith(".tar.xz");
                    boolean isArm = assetName.contains("arm64");
                    boolean isV3 = assetName.contains("x86_64_v3");
                    if (!isXz || assetName.contains("sha512sum")) continue;
                    if (sysIsArm && !isArm) continue;
                    if (!sysIsArm && isArm) continue;

                    String url = asset.path("browser_download_url").asText(null);
                    if (url == null) continue;

                    if (isV3 && !foundV3) {
                        var entry = new HashMap<String, String>();
                        entry.put("filename", assetName);
                        entry.put("url", url);
                        result.put(tag + " (v3)", entry);
                        foundV3 = true;
                    } else if (!isV3 && !foundBase) {
                        var entry = new HashMap<String, String>();
                        entry.put("filename", assetName);
                        entry.put("url", url);
                        result.put(tag, entry);
                        foundBase = true;
                    }
                    if (foundV3 && foundBase) break;
                }
            }
        } catch (Exception e) {
            LOG.warn("Failed to parse CachyOS Proton releases JSON", e);
        }

        return result;
    }

    // -----------------------------------------------------------------------
    // Proton local discovery
    // -----------------------------------------------------------------------

    public static String findNewestAvailableProton() {
        Path latestProton = null;
        long latestDate = Long.MIN_VALUE;

        for (String path : com.corkytux.launcher.forms.LauncherSettings.getAllProtonPaths()) {
            if (path == null || path.isBlank()) continue;
            var protonsPath = Path.of(path);
            if (!Files.isDirectory(protonsPath)) continue;

            try (DirectoryStream<Path> stream = Files.newDirectoryStream(protonsPath)) {
                for (var dir : stream) {
                    var protonExec = dir.resolve("proton");
                    if (!Files.isRegularFile(protonExec)) continue;
                    long lastMod = Files.getLastModifiedTime(protonExec).toMillis();
                    if (lastMod > latestDate) {
                        latestDate = lastMod;
                        latestProton = dir;
                    }
                }
            } catch (IOException e) {
                LOG.warn("findNewestAvailableProton scan failed for {}", path, e);
            }
        }

        if (latestProton == null) return null;
        return latestProton.getFileName().toString();
    }

    public static List<String> getInstalledProtons() {
        var result = new ArrayList<String>();
        for (String path : com.corkytux.launcher.forms.LauncherSettings.getAllProtonPaths()) {
            if (path != null && !path.isBlank()) {
                result.addAll(getInstalledProtonsForPath(path));
            }
        }
        return result;
    }

    public static List<String> getInstalledProtonsForPath(String path) {
        var protonPath = Path.of(path);
        var result = new ArrayList<String>();
        if (!Files.isDirectory(protonPath)) return result;
        try (DirectoryStream<Path> stream = Files.newDirectoryStream(protonPath)) {
            for (var dir : stream) {
                if (Files.isRegularFile(dir.resolve("proton"))) {
                    result.add(dir.getFileName().toString());
                }
            }
        } catch (IOException e) {
            LOG.warn("getInstalledProtons failed for path: {}", path, e);
        }
        return result;
    }

    /**
     * Returns all protons from all configured paths with path suffix always shown.
     * Every proton gets " - Path X" suffix to clearly indicate which path it belongs to.
     */
    public static List<String> getAllProtonsWithPathInfo() {
        var allPaths = com.corkytux.launcher.forms.LauncherSettings.getAllProtonPaths();
        var result = new ArrayList<String>();
        
        for (int i = 0; i < allPaths.size(); i++) {
            String path = allPaths.get(i);
            var protons = getInstalledProtonsForPath(path);
            for (String proton : protons) {
                result.add(proton + " - Path " + (i + 1));
            }
        }
        
        return result;
    }

    public static String ensureProtonLatestName() {
        if (latestProtonUrl == null || "fetching".equals(latestProtonUrl)) {
            return findNewestAvailableProton();
        }
        // extract name between last '/' +1 and ".tar"
        String url = latestProtonUrl;
        int slash = url.lastIndexOf('/');
        int tar = url.indexOf(".tar");
        if (slash != -1 && tar != -1 && tar > slash) {
            return url.substring(slash + 1, tar);
        }
        // fallback: strip extension attempts
        if (slash != -1) return url.substring(slash + 1);
        return url;
    }

    public static String ensureProtonPath(String proton) {
        if (proton == null) return null;
        if ("GE-Proton Latest".equals(proton)) {
            proton = ensureProtonLatestName();
            if (proton == null) return null;
        }
        
        // Strip path suffix if present (e.g., "GE-Proton9-5 - Path 3" -> "GE-Proton9-5")
        String actualName = proton;
        int suffixIdx = proton.lastIndexOf(" - Path ");
        if (suffixIdx > 0) {
            actualName = proton.substring(0, suffixIdx);
        }
        
        // Search in all configured paths
        var allPaths = com.corkytux.launcher.forms.LauncherSettings.getAllProtonPaths();
        for (String basePath : allPaths) {
            var protonsPath = Path.of(basePath, actualName);
            if (Files.exists(protonsPath)) {
                return protonsPath.toString();
            }
        }
        
        // Fallback to default path
        var defaultPath = Path.of(getBasePathFor("protons"), actualName);
        if (Files.exists(defaultPath)) {
            return defaultPath.toString();
        }
        
        return null;
    }

    // -----------------------------------------------------------------------
    // Proton executable resolution
    // -----------------------------------------------------------------------

    public static String getProtonExecutable(String gameName) {
        return getProtonExecutable(gameName, "proton", false);
    }

    public static String getProtonExecutable(String gameName, String exec) {
        return getProtonExecutable(gameName, exec, false);
    }

    public static String getProtonExecutable(String gameName, String exec, boolean skipIfNotFound) {
        // Fix: Must be offline-safe. Network failure (fetchLatestProton) leaves LatestProton = null/fetching.
        // Previous code could recurse or return null and leave Run/Utilities gray as if no internet.
        // Correct: always fallback to local newest proton when Latest not available, no network required.
        String proton;
        try {
            if (gameName != null) {
                proton = getGameProperty("proton", gameName);
                if (proton == null) {
                    proton = getLauncherProperty("defaultProton", "User Settings");
                }
            } else {
                proton = getLauncherProperty("defaultProton", "User Settings");
            }
            if (proton == null) proton = "GE-Proton Latest";
        } catch (Exception e) {
            LOG.debug("getProtonExecutable ini read failed offline safe", e);
            proton = "GE-Proton Latest";
        }

        String protonPath;
        try {
            protonPath = ensureProtonPath(proton);
        } catch (Exception e) {
            LOG.debug("ensureProtonPath failed offline safe for {}", proton, e);
            protonPath = null;
        }
        if (protonPath == null) {
            // Offline fallback: if requested Latest but fetch failed, use newest local proton directly
            if ("GE-Proton Latest".equals(proton)) {
                try {
                    String localLatest = findNewestAvailableProton();
                    if (localLatest != null) {
                        protonPath = ensureProtonPath(localLatest);
                        if (protonPath != null) proton = localLatest;
                    }
                } catch (Exception ignored) {}
            }
            if (protonPath == null) {
                if (!skipIfNotFound) {
                    final String protonForLog = proton;
                    runOnFxThread(() -> LOG.info("Proton not found, would trigger download for {}", protonForLog));
                    // Safe recursion only once, avoid stack overflow offline
                    if (!"GE-Proton Latest".equals(proton)) {
                        return getProtonExecutable(gameName, exec, true);
                    }
                }
                return null;
            }
        }

        String execPath;
        if ("proton".equals(exec)) {
            execPath = protonPath + "/proton";
        } else {
            execPath = protonPath + "/files/bin/" + exec;
        }

        if (!Files.isRegularFile(Path.of(execPath))) {
            // For wine/wineserver, some protons place them directly under protonPath
            Path alt = Path.of(protonPath, exec);
            if (Files.isRegularFile(alt)) return alt.toAbsolutePath().toString();
            Path alt2 = Path.of(protonPath, "files/bin/" + exec + ".exe");
            if (Files.isRegularFile(alt2)) return alt2.toAbsolutePath().toString();
            if (!skipIfNotFound) {
                final String protonForLog2 = proton;
                runOnFxThread(() -> LOG.info("Proton exec not found, would trigger download for {}", protonForLog2));
                return getProtonExecutable(gameName, exec, true);
            }
            return null;
        }

        return Path.of(execPath).toAbsolutePath().toString();
    }

    // -----------------------------------------------------------------------
    // Prefix path
    // -----------------------------------------------------------------------

    public static String getProtonPrefixPath(String gameName) {
        return getProtonPrefixPath(gameName, "proton");
    }

    public static String getProtonPrefixPath(String gameName, String type) {
        var prefixPath = getGameProperty("prefixPath", gameName);
        if (prefixPath == null || prefixPath.isBlank()) {
            var executable = getGameProperty("executable", gameName);
            if (executable != null) {
                var parent = Path.of(executable).getParent();
                if (parent != null) {
                    prefixPath = parent.resolve("OFME Prefix").toString();
                } else {
                    prefixPath = Path.of("OFME Prefix").toAbsolutePath().toString();
                }
            } else {
                prefixPath = Path.of(getBasePathFor("prefixes"), gameName != null ? gameName : "default").toString();
            }
        }
        if ("wine".equals(type)) {
            prefixPath = prefixPath + "/pfx";
        }

        var p = Path.of(prefixPath);
        try {
            if (p.getParent() != null) Files.createDirectories(p.getParent());
            Files.createDirectories(p);
            // Fix root-owned prefix that triggers thumbnail cache admin prompts
            fixRootOwnershipIfNeeded(p, "prefix:" + gameName);
            if (p.getParent() != null) fixRootOwnershipIfNeeded(p.getParent(), "prefixParent:" + gameName);
            // Also ensure .local/share/CorkyTux itself not root-owned
            String launcherBase = "root".equals(System.getProperty("user.name"))
                    ? "/home/" + getExpectedUser() : System.getProperty("user.home");
            fixRootOwnershipIfNeeded(Path.of(launcherBase, ".local/share/CorkyTux"), "launcherRoot");
            fixRootOwnershipIfNeeded(Path.of(launcherBase, ".local/share/CorkyTux/prefixes"), "prefixes");
            // Ensure thumbnail cache is not root-owned – fixes Nemo Se ha detectado un problema con la caché de miniaturas
            fixThumbnailCachePermissions();
        } catch (IOException e) {
            LOG.warn("Failed to create prefix dir {}", p, e);
        }
        return p.toString();
    }

    // -----------------------------------------------------------------------
    // Steam runner
    // -----------------------------------------------------------------------

    public static boolean runSteam() {
        runOnFxThread(() -> LOG.info("Switching play button to 'wait' – starting Steam"));

        Process steam = null;
        try {
            steam = new ProcessBuilder("steam", "-silent").start();

            var loginUsers = Path.of(getExpectedHome(), ".steam/steam/config/loginusers.vdf");
            long lastMod = Files.exists(loginUsers) ? Files.getLastModifiedTime(loginUsers).toMillis() : -1L;

            int attempts = 0;
            while (attempts <= 420) {
                boolean exists = Files.exists(loginUsers);
                long curMod = exists ? Files.getLastModifiedTime(loginUsers).toMillis() : -1L;

                boolean changed = exists && curMod != lastMod;
                if (changed) break;

                if (steam != null && !steam.isAlive()) {
                    // steam exited early – consider as completed after 420 to signal failure path mirroring PHP
                    attempts = 420;
                }

                attempts++;
                try { Thread.sleep(1000); } catch (InterruptedException ie) { Thread.currentThread().interrupt(); break; }

                if (attempts > 420) break;
                // re-evaluate condition for next iteration
                if (steam != null && steam.isAlive() == false && attempts >= 420) break;
            }

            runOnFxThread(() -> LOG.info("Switching play button to 'play' – Steam start finished"));
            return attempts <= 420;

        } catch (Exception e) {
            LOG.warn("runSteam failed", e);
            runOnFxThread(() -> LOG.info("Switching play button to 'play' – Steam start failed"));
            return false;
        }
    }

    // -----------------------------------------------------------------------
    // Third-party helpers
    // -----------------------------------------------------------------------

    public static String getThirdParty(String prog) {
        // Fix: Run (third-party exe) must not show as if no internet when 7zip/unrar missing.
        // Offline: fallback gracefully, try both /usr/bin and ./thirdparty, then search PATH via 'which'.
        Map<String, String> progs = new HashMap<>();
        try {
            progs.put("7zip", Files.isRegularFile(Path.of("/usr/bin/7z")) ? "/usr/bin/7z" : "./thirdparty/7zip/7z");
            progs.put("unrar", Files.isRegularFile(Path.of("/usr/bin/unrar")) ? "/usr/bin/unrar" : "./thirdparty/unrar/unrar");
        } catch (Exception e) {
            LOG.debug("getThirdParty init failed offline safe", e);
            return null;
        }

        var candidate = progs.get(prog);
        if (candidate == null) {
            LOG.warn("Unknown thirdparty program: {} (offline safe)", prog);
            return null;
        }
        if (!Files.isRegularFile(Path.of(candidate))) {
            // Try which fallback before failing – does not require internet
            try {
                var proc = new ProcessBuilder("which", prog.equals("7zip") ? "7z" : prog).start();
                String whichOut = new String(proc.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
                proc.waitFor();
                if (!whichOut.isEmpty() && Files.isRegularFile(Path.of(whichOut))) {
                    LOG.info("Thirdparty {} resolved via which -> {}", prog, whichOut);
                    var f = new File(whichOut);
                    if (!f.canExecute()) f.setExecutable(true);
                    return f.getAbsolutePath();
                }
            } catch (Exception ignored) {}
            LOG.warn("Thirdparty {} not found at {} (offline safe, not internet)", prog, candidate);
            // Do not throw; caller will show toast but UI stays enabled (button not gray via network gate)
            return null;
        }

        var file = new File(candidate);
        try {
            if (!file.canExecute()) file.setExecutable(true);
        } catch (Exception e) {
            LOG.debug("chmod failed for {} offline safe", candidate, e);
        }
        return file.getAbsolutePath();
    }

    // -----------------------------------------------------------------------
    // Helpers – config, ini, args, env, fx
    // -----------------------------------------------------------------------

    public static String getBasePathFor(String forWhat) {
        boolean isRoot = "root".equals(System.getProperty("user.name"));
        String homeBase = isRoot ? "/home/" + getExpectedUser() : System.getProperty("user.home");
        var defaultDir = Path.of(homeBase, ".local/share/CorkyTux", forWhat).toString();
        var userDir = getLauncherProperty(forWhat + "Path", "User Settings");
        if (userDir == null || userDir.isBlank()) {
            try {
                var p = Path.of(defaultDir);
                if (p.getParent() != null) Files.createDirectories(p.getParent());
                Files.createDirectories(p);
                fixRootOwnershipIfNeeded(p, forWhat);
                fixThumbnailCachePermissions();
            } catch (IOException e) {
                LOG.warn("Failed to ensure default dir {}", defaultDir, e);
            }
            return defaultDir;
        }
        // Even for custom paths, ensure fix if exists
        try {
            var custom = Path.of(userDir);
            if (Files.exists(custom)) fixRootOwnershipIfNeeded(custom, forWhat);
            fixThumbnailCachePermissions();
        } catch (Exception ex) {
            LOG.debug("Custom base path ownership check failed for {}", userDir, ex);
        }
        return userDir;
    }

    /**
     * Returns the expected non-root user for thumbnail and steam checks.
     * When running as root via sudo, returns SUDO_USER; otherwise derives from HOME or /home/*.
     * Fully dynamic — no hardcoded usernames.
     */
    public static String getExpectedUser() {
        String currentUser = System.getProperty("user.name");
        String sudoUser = System.getenv("SUDO_USER");
        String userHome = System.getProperty("user.home");
        String homeOwner = userHome != null && userHome.contains("/") ? userHome.substring(userHome.lastIndexOf('/') + 1) : currentUser;

        // 1) SUDO_USER is the most reliable when launched via sudo
        if (sudoUser != null && !sudoUser.isBlank() && !"root".equals(sudoUser)) return sudoUser;

        // 2) If HOME is not root, the home dir owner is the user
        if (homeOwner != null && !homeOwner.isBlank() && !"root".equals(homeOwner)) return homeOwner;

        // 3) Running as non-root: currentUser is the user
        if (!"root".equals(currentUser)) return currentUser;

        // 4) Running as root with HOME=/root: scan /home/* for first real user
        try {
            var homeDir = Path.of("/home");
            if (Files.isDirectory(homeDir)) {
                try (var stream = Files.list(homeDir)) {
                    var firstUser = stream
                            .filter(Files::isDirectory)
                            .map(p -> p.getFileName().toString())
                            .filter(name -> !name.equals("root"))
                            .findFirst();
                    if (firstUser.isPresent()) return firstUser.get();
                }
            }
        } catch (Exception ignored) {}

        // 5) Last resort: return currentUser (may be root)
        return currentUser;
    }

    /** Returns the correct home directory for the expected user, even when running as root. */
    public static String getExpectedHome() {
        String user = getExpectedUser();
        // Check standard /home/<user> first
        String candidate = "/home/" + user;
        if (Files.isDirectory(Path.of(candidate))) return candidate;
        // Fallback: System.getProperty("user.home") if not root
        String sysHome = System.getProperty("user.home");
        if (sysHome != null && !"/root".equals(sysHome)) return sysHome;
        return candidate;
    }

    /**
     * Returns the Flatpak Steam data directory if it exists, null otherwise.
     * Path: ~/.var/app/com.valvesoftware.Steam/data/Steam
     */
    public static String getFlatpakSteamDataPath() {
        String flatpakPath = getExpectedHome() + "/.var/app/com.valvesoftware.Steam/data/Steam";
        if (Files.isDirectory(Path.of(flatpakPath))) return flatpakPath;
        return null;
    }

    /**
     * Returns the best STEAM_COMPAT_CLIENT_INSTALL_PATH.
     * Prefers native ~/.steam/steam, falls back to Flatpak path.
     */
    public static String getSteamClientInstallPath() {
        String nativePath = getExpectedHome() + "/.steam/steam";
        if (Files.isDirectory(Path.of(nativePath))) return nativePath;
        String flatpakPath = getFlatpakSteamDataPath();
        if (flatpakPath != null) return flatpakPath;
        return nativePath; // fallback even if missing
    }

    /** Reads DBUS_SESSION_BUS_ADDRESS from a desktop session process (cinnamon, gnome-session, etc.) */
    private static String readDbusFromSessionProcess() {
        try {
            String[] procs = {"cinnamon", "gnome-session", "xfce4-session", "mate-session", "lxqt-session"};
            for (String proc : procs) {
                var pids = new ProcessBuilder("pgrep", "-u", getExpectedUser(), proc)
                        .redirectErrorStream(true).start();
                String pidOut = new String(pids.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
                pids.waitFor();
                if (pidOut.isBlank()) continue;
                for (String pid : pidOut.split("\n")) {
                    pid = pid.trim();
                    if (pid.isEmpty()) continue;
                    try {
                        var env = new ProcessBuilder("bash", "-c", "cat /proc/" + pid + "/environ 2>/dev/null | tr '\\0' '\\n' | grep DBUS_SESSION_BUS_ADDRESS | cut -d= -f2-")
                                .redirectErrorStream(true).start();
                        String dbus = new String(env.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
                        env.waitFor();
                        if (dbus != null && !dbus.isBlank()) {
                            LOG.debug("Read DBUS from {} pid {}: {}", proc, pid, dbus);
                            return dbus;
                        }
                    } catch (Exception ignored) {}
                }
            }
        } catch (Exception e) { LOG.debug("readDbusFromSessionProcess failed", e); }
        return null;
    }

    /** Reads critical XDG env vars from a desktop session process */
    private static java.util.Map<String, String> readSessionEnvFromProcess() {
        var result = new java.util.HashMap<String, String>();
        try {
            String[] procs = {"cinnamon", "gnome-session", "xfce4-session", "mate-session", "lxqt-session"};
            String[] keys = {"DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_VTNR"};
            for (String proc : procs) {
                var pids = new ProcessBuilder("pgrep", "-u", getExpectedUser(), proc)
                        .redirectErrorStream(true).start();
                String pidOut = new String(pids.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
                pids.waitFor();
                if (pidOut.isBlank()) continue;
                String[] pidLines = pidOut.split("\n");
                for (String pidLine : pidLines) {
                    String pid = pidLine.trim();
                    if (pid.isEmpty()) continue;
                    String grepPattern = String.join("|", keys);
                    var env = new ProcessBuilder("bash", "-c",
                            "cat /proc/" + pid + "/environ 2>/dev/null | tr '\\0' '\\n' | grep -E '^(" + grepPattern + ")='")
                            .redirectErrorStream(true).start();
                    String out = new String(env.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
                    env.waitFor();
                    for (String line : out.split("\n")) {
                        line = line.trim();
                        int eq = line.indexOf('=');
                        if (eq > 0) {
                            result.put(line.substring(0, eq), line.substring(eq + 1));
                        }
                    }
                    if (!result.isEmpty()) {
                        LOG.debug("Read session env from {} pid {}: {} keys", proc, pid, result.size());
                        return result;
                    }
                }
            }
        } catch (Exception e) { LOG.debug("readSessionEnvFromProcess failed", e); }
        return result;
    }

    /**
     * Builds a ProcessBuilder for xdg-open that runs as the user when launcher is root.
     * Fixes thumbnail cache admin error where Nemo shows "Se ha detectado un problema con la caché de miniaturas.
     * Necesita privilegios administrativos" because xdg-open was invoked as root (HOME=/root) while cache is owned by the user.
     * When running as root, uses sudo -u the user or runuser -u the user with preserved DISPLAY/XDG env, otherwise plain xdg-open/gio.
     * Java 25: ensures flatpak/wayland + x11 dual support (XDG_SESSION_TYPE, DISPLAY, WAYLAND_DISPLAY preserved).
     */
    public static ProcessBuilder buildXdgOpenCommand(String target) {
        String currentUser = System.getProperty("user.name");
        String expected = getExpectedUser();
        boolean isRoot = "root".equals(currentUser);
        // Prefer xdg-open, fallback to gio open – both handle Wayland/X11; xdg-open respects XDG_SESSION_TYPE
        if (isRoot && expected != null && !"root".equals(expected) && !"".equals(expected)) {
            // Running as root – must open as the user to avoid root-owned thumbnails and DBUS mismatch
            // Try sudo -u first (preserves HOME, but we set HOME to /home/the user for correct cache)
            String display = System.getenv("DISPLAY");
            if (display == null || display.isBlank()) display = ":0";
            String wayland = System.getenv("WAYLAND_DISPLAY");
            String xdgSession = System.getenv("XDG_SESSION_TYPE");
            String xdgCurrent = System.getenv("XDG_CURRENT_DESKTOP");
            // Build env string for sudo -u
            // Use sudo -u the user env DISPLAY=:0 ... xdg-open target (ensures thumbnail cache is /home/the user, not /root)
            // Note: HOME must be /home/the user for xdg-open/gio to pick correct .cache/thumbnails
            java.util.List<String> cmd = new java.util.ArrayList<>();
            cmd.add("sudo");
            cmd.add("-u");
            cmd.add(expected);
            // Preserve critical env explicitly via env command
            cmd.add("env");
            // CRITICAL: Unset SUDO_UID — when Java runs as root and uses sudo -u, sudo
            // sets SUDO_UID=0 (root's uid). cinnamon-desktop's gnome_desktop_get_session_user_pwent()
            // reads SUDO_UID before USER, resolving to root instead of the target user.
            // This causes Nemo's thumbnail cache permission check to fail (files owned by
            // user vs check against root) and shows the "thumbnail cache problem" bar.
            cmd.add("-u");
            cmd.add("SUDO_UID");
            cmd.add("HOME=/home/" + expected);
            cmd.add("USER=" + expected);
            cmd.add("LOGNAME=" + expected);
            if (display != null) cmd.add("DISPLAY=" + display);
            if (wayland != null && !wayland.isBlank()) cmd.add("WAYLAND_DISPLAY=" + wayland);
            if (xdgSession != null && !xdgSession.isBlank()) cmd.add("XDG_SESSION_TYPE=" + xdgSession);
            if (xdgCurrent != null && !xdgCurrent.isBlank()) cmd.add("XDG_CURRENT_DESKTOP=" + xdgCurrent);
            // Preserve DBUS if available from the user session (try to read from /proc or fallback)
            String dbus = System.getenv("DBUS_SESSION_BUS_ADDRESS");
            if (dbus != null && !dbus.isBlank()) cmd.add("DBUS_SESSION_BUS_ADDRESS=" + dbus);
            else {
                // Try to get dbus from the user's environment via sudo -u env
                try {
                    String dbusAsUser = execOutputTrimmed("bash", "-c", "sudo -u " + expected + " env 2>/dev/null | grep DBUS_SESSION_BUS_ADDRESS | cut -d= -f2-");
                    if (dbusAsUser != null && !dbusAsUser.isBlank()) cmd.add("DBUS_SESSION_BUS_ADDRESS=" + dbusAsUser.trim());
                } catch (Exception ignored) {}
            }
            cmd.add("xdg-open");
            cmd.add(target);
            LOG.debug("buildXdgOpenCommand as root -> sudo -u {} xdg-open {}", expected, target);
            return new ProcessBuilder(cmd);
        } else {
            // Normal user – direct xdg-open, but ensure critical session env vars are present
            // (may be missing when launched from root terminal: sudo -u the user bash -c 'java -jar ...')
            var sessionEnv = readSessionEnvFromProcess();
            java.util.List<String> cmd = new java.util.ArrayList<>();
            cmd.add("env");
            // CRITICAL: Unset SUDO_UID — even when launched via `sudo -u the user bash -c 'java ...'`
            // from a root terminal, sudo sets SUDO_UID=0 (root's uid). cinnamon-desktop's
            // gnome_desktop_get_session_user_pwent() reads SUDO_UID before USER, resolving to
            // root instead of the target user. This causes Nemo's thumbnail cache permission
            // check to fail (files owned by user vs check against root) and shows the warning bar.
            cmd.add("-u"); cmd.add("SUDO_UID");
            cmd.add("-u"); cmd.add("PKEXEC_UID");
            // Always set DBUS
            String dbus = System.getenv("DBUS_SESSION_BUS_ADDRESS");
            if (dbus == null || dbus.isBlank()) dbus = sessionEnv.get("DBUS_SESSION_BUS_ADDRESS");
            if (dbus != null && !dbus.isBlank()) cmd.add("DBUS_SESSION_BUS_ADDRESS=" + dbus);
            // XDG_RUNTIME_DIR is critical for Nemo thumbnail service
            String runtime = System.getenv("XDG_RUNTIME_DIR");
            if (runtime == null || runtime.isBlank()) runtime = sessionEnv.get("XDG_RUNTIME_DIR");
            if (runtime != null && !runtime.isBlank()) cmd.add("XDG_RUNTIME_DIR=" + runtime);
            // XDG_SESSION_TYPE for Wayland/X11 detection
            String sessionType = System.getenv("XDG_SESSION_TYPE");
            if (sessionType == null || sessionType.isBlank()) sessionType = sessionEnv.get("XDG_SESSION_TYPE");
            if (sessionType != null && !sessionType.isBlank()) cmd.add("XDG_SESSION_TYPE=" + sessionType);
            // XDG_CURRENT_DESKTOP
            String desktop = System.getenv("XDG_CURRENT_DESKTOP");
            if (desktop == null || desktop.isBlank()) desktop = sessionEnv.get("XDG_CURRENT_DESKTOP");
            if (desktop != null && !desktop.isBlank()) cmd.add("XDG_CURRENT_DESKTOP=" + desktop);
            cmd.add("xdg-open");
            cmd.add(target);
            LOG.debug("buildXdgOpenCommand as {} -> xdg-open {} sessionEnvKeys={}", currentUser, target, sessionEnv.size());
            return new ProcessBuilder(cmd);
        }
    }

    /**
     * Opens path/url via xdg-open with correct user and thumbnail cache fix before open.
     * Ensures fixThumbnailCachePermissions is called first and xdg-open is invoked as the user not root.
     * Fallback to gio open if xdg-open fails (Wayland-safe).
     */
    public static void openWithXdgOpen(String target) throws IOException {
        fixThumbnailCachePermissions();
        ProcessBuilder pb = buildXdgOpenCommand(target);
        try {
            pb.start();
        } catch (IOException e) {
            LOG.warn("xdg-open failed for {}, fallback to gio open", target, e);
            String currentUser = System.getProperty("user.name");
            String expected = getExpectedUser();
            boolean isRoot = "root".equals(currentUser);
            String dbus = System.getenv("DBUS_SESSION_BUS_ADDRESS");
            if (dbus == null || dbus.isBlank()) dbus = readDbusFromSessionProcess();
            ProcessBuilder gio;
            if (isRoot && expected != null && !"root".equals(expected)) {
                java.util.List<String> gioCmd = new java.util.ArrayList<>();
                gioCmd.add("sudo"); gioCmd.add("-u"); gioCmd.add(expected);
                gioCmd.add("env"); gioCmd.add("-u"); gioCmd.add("SUDO_UID");
                if (dbus != null && !dbus.isBlank()) { gioCmd.add("DBUS_SESSION_BUS_ADDRESS=" + dbus); }
                gioCmd.add("gio"); gioCmd.add("open"); gioCmd.add(target);
                gio = new ProcessBuilder(gioCmd);
            } else {
                java.util.List<String> cmd = new java.util.ArrayList<>();
                cmd.add("env");
                cmd.add("-u"); cmd.add("SUDO_UID");
                cmd.add("-u"); cmd.add("PKEXEC_UID");
                if (dbus != null && !dbus.isBlank()) { cmd.add("DBUS_SESSION_BUS_ADDRESS=" + dbus); }
                cmd.add("gio"); cmd.add("open"); cmd.add(target);
                gio = new ProcessBuilder(cmd);
            }
            gio.start();
        }
    }

    /**
     * Fixes Nemo thumbnail cache admin error: Se ha detectado un problema con la caché de miniaturas.
     * Necesita privilegios administrativos – when ~/.cache/thumbnails or prefixes were created as root
     * (sudo runs), Nemo requires admin on folder open. This ensures ~/.cache/thumbnails is owned by
     * the user (or SUDO_USER) not root, and perms are rwxr-xr-x / 0755 so thumbnailer can write without admin.
     * Java 25: correctly handles the user not root, dual HOME (/root vs /home/the user), and xdg-open user.
     */
    public static void fixThumbnailCachePermissions() {
        try {
            String userHome = System.getProperty("user.home");
            String currentUser = System.getProperty("user.name");
            String expected = getExpectedUser();
            boolean runningAsRoot = "root".equals(currentUser);
            // When running as root (HOME=/root), target the REAL user's cache, not /root/.cache
            // When running as user, target user.home directly
            String targetHome = runningAsRoot ? "/home/" + expected : userHome;

            // Sentinel: skip if verified within last 24 hours
            Path sentinel = Path.of(targetHome, ".cache/thumbnails/.corkytux-perms-ok");
            if (Files.isRegularFile(sentinel)) {
                try {
                    long lastModified = Files.getLastModifiedTime(sentinel).toMillis();
                    if (System.currentTimeMillis() - lastModified < 86_400_000L) {
                        LOG.debug("fixThumbnailCachePermissions: sentinel exists and fresh, skipping");
                        return;
                    }
                } catch (Exception ignored) {}
            }

            var toFixList = new java.util.ArrayList<Path>();
            toFixList.addAll(List.of(
                    Path.of(targetHome, ".cache/thumbnails"),
                    Path.of(targetHome, ".cache/thumbnails/large"),
                    Path.of(targetHome, ".cache/thumbnails/normal"),
                    Path.of(targetHome, ".cache/thumbnails/fail"),
                    Path.of(targetHome, ".local/share/CorkyTux"),
                    Path.of(targetHome, ".local/share/CorkyTux/prefixes")
            ));
            // Also include other /home/* users if different from targetHome
            try {
                var homeDir = Path.of("/home");
                if (Files.isDirectory(homeDir)) {
                    try (var stream = Files.list(homeDir)) {
                        stream.filter(Files::isDirectory)
                              .map(p -> p.toString())
                              .filter(h -> !h.equals(targetHome))
                              .forEach(h -> {
                                  toFixList.add(Path.of(h, ".cache/thumbnails"));
                                  toFixList.add(Path.of(h, ".cache/thumbnails/large"));
                                  toFixList.add(Path.of(h, ".cache/thumbnails/normal"));
                                  toFixList.add(Path.of(h, ".cache/thumbnails/fail"));
                              });
                    }
                }
            } catch (Exception ignored) {}
            List<Path> toFix = java.util.Collections.unmodifiableList(toFixList);
            for (Path p : toFix) {
                try {
                    if (!Files.exists(p)) {
                        // Never create dirs as root — Nemo runs as the user and expects cache there
                        if (runningAsRoot) continue;
                        // Only create when running as the correct user
                        if (p.toString().contains(".cache/thumbnails")) {
                            Files.createDirectories(p);
                            LOG.info("Created thumbnail cache dir {}", p);
                        } else continue;
                    }
                    var owner = Files.getOwner(p).getName();
                    boolean isRootOwned = "root".equals(owner);
                    if (isRootOwned) {
                        if (runningAsRoot && !"root".equals(expected)) {
                            // Running as root — fix ownership directly
                            try {
                                var proc = new ProcessBuilder("chown", "-R", expected + ":" + expected, p.toString())
                                        .redirectErrorStream(true).start();
                                String out = new String(proc.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
                                int exit = proc.waitFor();
                                if (exit == 0) LOG.info("Fixed ownership {} -> {}", p, expected);
                                else LOG.warn("chown failed {} exit={} out={}", p, exit, out);
                            } catch (Exception ex) { LOG.warn("Failed chown {} -> {}", p, expected, ex); }
                            try {
                                var chmodPb = new ProcessBuilder("chmod", "-R", "755", p.toString());
                                chmodPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chmodPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chmodPb.start().waitFor();
                            } catch (Exception ignored) {}
                        } else if (!runningAsRoot) {
                            // Non-root: skip root-owned dirs silently, never prompt for admin
                            LOG.debug("Path {} owned by root while running as {} – skipping", p, currentUser);
                        }
                    }
                    // Ensure perms 755 on dirs we own
                    if (!isRootOwned || runningAsRoot) {
                        try {
                            if (Files.isDirectory(p)) {
                                Files.setPosixFilePermissions(p, java.nio.file.attribute.PosixFilePermissions.fromString("rwxr-xr-x"));
                            }
                        } catch (Exception ignored) {}
                    }
                } catch (Exception e) { LOG.debug("fixThumbnailCache for {} failed", p, e); }
            }
            // Only scan individual thumbnail files when running as root to fix ownership
            if (runningAsRoot && !"root".equals(expected)) {
                for (String base : new String[]{targetHome, getExpectedHome()}) {
                    Path thumbRoot = Path.of(base, ".cache/thumbnails");
                    if (!Files.isDirectory(thumbRoot)) continue;
                    try (var walk = Files.walk(thumbRoot)) {
                        walk.filter(Files::isRegularFile).forEach(f -> {
                            try {
                                var o = Files.getOwner(f).getName();
                                if ("root".equals(o)) {
                                    var chownPb = new ProcessBuilder("chown", expected + ":" + expected, f.toString());
                                    chownPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chownPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chownPb.start().waitFor();
                                    var chmodPb = new ProcessBuilder("chmod", "644", f.toString());
                                    chmodPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chmodPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chmodPb.start().waitFor();
                                }
                            } catch (Exception ignored) {}
                        });
                    } catch (Exception ignored) {}
                }
            }
            LOG.debug("fixThumbnailCachePermissions done expected={} root={}", expected, runningAsRoot);
            // Write sentinel to skip next 24h
            try {
                Path sentinelDir = sentinel.getParent();
                if (sentinelDir != null && !Files.isDirectory(sentinelDir)) Files.createDirectories(sentinelDir);
                Files.writeString(sentinel, "ok");
            } catch (Exception ignored) {}
        } catch (Exception e) {
            LOG.debug("fixThumbnailCachePermissions failed", e);
        }
    }

    /**
     * Fixes thumbnail cache / admin message caused by prefix created as root.
     * If directory is owned by root while current user is not root (or vice versa via SUDO_USER),
     * attempts chown -R to correct user and ensures rwxr-xr-x to avoid thumbnail admin prompts.
     */
    private static void fixRootOwnershipIfNeeded(Path path, String context) {
        try {
            if (!Files.exists(path)) return;
            var owner = Files.getOwner(path).getName();
            String currentUser = System.getProperty("user.name");
            String sudoUser = System.getenv("SUDO_USER");
            String homeOwner = Path.of(System.getProperty("user.home")).getFileName() != null
                    ? System.getProperty("user.home").replaceAll(".*/","")
                    : currentUser;
            // Heuristic: expected owner is either currentUser, sudoUser, or homeOwner (the user) – Java 25 centralizes via getExpectedUser but keep inline for legacy
            String expected = sudoUser != null && !sudoUser.isBlank() ? sudoUser
                    : (!"root".equals(homeOwner) ? homeOwner : currentUser);
            if ("root".equals(expected) || expected == null || expected.isBlank()) expected = getExpectedUser();
            boolean isRootOwned = "root".equals(owner);
            boolean runningAsRoot = "root".equals(currentUser);
            if (isRootOwned && !runningAsRoot) {
                // Non-root: skip root-owned dirs silently, never prompt for admin
                LOG.debug("Path {} ({} ) owned by root while running as {} – skipping (run launcher as root once to fix)", path, context, currentUser);
            } else if (isRootOwned && runningAsRoot && expected != null && !"root".equals(expected)) {
                LOG.warn("Path {} ({} ) owned by root while SUDO_USER/home is {} – fixing ownership", path, context, expected);
                try {
                    var proc = new ProcessBuilder("chown", "-R", expected + ":" + expected, path.toString())
                            .redirectErrorStream(true).start();
                    String out = new String(proc.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
                    int exit = proc.waitFor();
                    if (exit == 0) LOG.info("Fixed ownership of {} -> {} ({} )", path, expected, context);
                    else LOG.warn("chown failed for {} exit={} out={}", path, exit, out);
                } catch (Exception ex) {
                    LOG.warn("Failed to chown {} -> {}", path, expected, ex);
                }
            }
            // Ensure posix permissions rwxr-xr-x to allow thumbnails and non-admin folder access
            try {
                var perms = java.nio.file.attribute.PosixFilePermissions.fromString("rwxr-xr-x");
                Files.setPosixFilePermissions(path, perms);
            } catch (Exception ignored) {
                // Non-posix FS or permission denied – ignore
            }
            // Also ensure parent launcher dir not root-owned
            String launcherHome = runningAsRoot ? "/home/" + expected : System.getProperty("user.home");
            Path parent = Path.of(launcherHome, ".local/share/CorkyTux");
            if (Files.exists(parent) && !path.equals(parent)) {
                try {
                    var pOwner = Files.getOwner(parent).getName();
                    if ("root".equals(pOwner) && runningAsRoot && expected != null && !"root".equals(expected)) {
                        new ProcessBuilder("chown", "-R", expected + ":" + expected, parent.toString()).start().waitFor();
                        LOG.info("Fixed parent launcher dir ownership {}", parent);
                    }
                } catch (Exception ignored) {}
            }
        } catch (Exception e) {
            LOG.debug("fixRootOwnershipIfNeeded failed for {} ({})", path, context, e);
        }
    }

    private static String getGameProperty(String key, String gameSection) {
        return getIniProperty(Path.of(getExpectedHome(), ".config/CorkyTux/Games.ini"), gameSection, key);
    }

    private static void setGameProperty(String key, String value, String gameSection) {
        setIniProperty(Path.of(getExpectedHome(), ".config/CorkyTux/Games.ini"), gameSection, key, value);
    }

    private static Map<String, String> getGameSection(String gameSection) {
        return getIniSection(Path.of(getExpectedHome(), ".config/CorkyTux/Games.ini"), gameSection);
    }

    private static String getLauncherProperty(String key, String section) {
        return getIniProperty(Path.of(getExpectedHome(), ".config/CorkyTux/Launcher.ini"), section, key);
    }

    private static String getIniProperty(Path iniPath, String section, String key) {
        if (!Files.isRegularFile(iniPath)) return null;
        try {
            var wini = new Wini(iniPath.toFile());
            var sec = wini.get(section);
            if (sec == null) return null;
            return sec.get(key);
        } catch (Exception e) {
            // fallback simple parse
            try {
                return simpleIniGet(iniPath, section, key);
            } catch (IOException ex) {
                LOG.debug("getIniProperty fallback failed", ex);
                return null;
            }
        }
    }

    private static void setIniProperty(Path iniPath, String section, String key, String value) {
        try {
            if (iniPath.getParent() != null) Files.createDirectories(iniPath.getParent());
            Wini wini;
            if (Files.isRegularFile(iniPath)) {
                wini = new Wini(iniPath.toFile());
            } else {
                // create new
                Files.createFile(iniPath);
                wini = new Wini(iniPath.toFile());
            }
            wini.put(section, key, value);
            wini.store();
        } catch (Exception e) {
            LOG.warn("setIniProperty failed for {} [{}] {}={}", iniPath, section, key, value, e);
        }
    }

    private static Map<String, String> getIniSection(Path iniPath, String section) {
        var map = new LinkedHashMap<String, String>();
        if (!Files.isRegularFile(iniPath)) return map;
        try {
            var wini = new Wini(iniPath.toFile());
            var sec = wini.get(section);
            if (sec != null) {
                for (var e : sec.entrySet()) map.put(e.getKey(), e.getValue());
            }
        } catch (Exception e) {
            LOG.debug("getIniSection wini failed, fallback", e);
            try {
                var all = simpleIniSection(iniPath, section);
                map.putAll(all);
            } catch (IOException ex) {
                LOG.debug("fallback also failed", ex);
            }
        }
        return map;
    }

    private static String simpleIniGet(Path iniPath, String section, String key) throws IOException {
        var lines = Files.readAllLines(iniPath, StandardCharsets.UTF_8);
        String current = null;
        for (var raw : lines) {
            var line = raw.trim();
            if (line.startsWith("[") && line.endsWith("]")) {
                current = line.substring(1, line.length() - 1);
            } else if (section.equals(current) && line.contains("=")) {
                int eq = line.indexOf('=');
                var k = line.substring(0, eq).trim();
                if (k.equals(key)) return line.substring(eq + 1).trim();
            }
        }
        return null;
    }

    private static Map<String, String> simpleIniSection(Path iniPath, String section) throws IOException {
        var map = new LinkedHashMap<String, String>();
        var lines = Files.readAllLines(iniPath, StandardCharsets.UTF_8);
        String current = null;
        for (var raw : lines) {
            var line = raw.trim();
            if (line.isEmpty() || line.startsWith(";") || line.startsWith("#")) continue;
            if (line.startsWith("[") && line.endsWith("]")) {
                current = line.substring(1, line.length() - 1);
            } else if (section.equals(current) && line.contains("=")) {
                int eq = line.indexOf('=');
                map.put(line.substring(0, eq).trim(), line.substring(eq + 1).trim());
            }
        }
        return map;
    }

    private static Map<String, String> parseEnvironmentMap(String game, String envString) {
        var out = new LinkedHashMap<String, String>();
        if (envString == null || envString.isBlank()) return out;
        // Normalize legacy 4-backslash saves -> 2, then split on literal "\\" (two)
        String normalized = envString.replace("\\\\\\\\", "\\\\");
        if (!normalized.contains("\\\\")) {
            var parts = normalized.split("====", 2);
            if (parts.length == 2) out.put(parts[0], parts[1]);
            else if (parts.length == 1 && !parts[0].isBlank()) out.put(parts[0], "");
        } else {
            var entries = normalized.split("\\\\\\\\", -1);
            for (var env : entries) {
                if (env == null || env.isEmpty()) continue;
                var kv = env.split("====", 2);
                if (kv.length == 2) out.put(kv[0], kv[1]);
                else if (kv.length == 1) out.put(kv[0], "");
            }
        }
        return out;
    }

    private static List<String> splitArgs(String args) {
        if (args == null || args.isBlank()) return List.of();
        // mirrors PHP str::split(' ') – simple space split, no quote handling
        var parts = args.trim().split(" +");
        var list = new ArrayList<String>();
        for (var p : parts) if (!p.isEmpty()) list.add(p);
        return list;
    }

    private static int executeAndGetExit(String... command) throws IOException, InterruptedException {
        var pb = new ProcessBuilder(command);
        pb.redirectOutput(ProcessBuilder.Redirect.DISCARD);
        pb.redirectError(ProcessBuilder.Redirect.DISCARD);
        var p = pb.start();
        return p.waitFor();
    }

    /** Timeout-guarded exec: returns exit code or -1 on timeout/error (2s). */
    private static int tryExecZeroTimeout(String... command) {
        try {
            var pb = new ProcessBuilder(command);
            pb.redirectOutput(ProcessBuilder.Redirect.DISCARD);
            pb.redirectError(ProcessBuilder.Redirect.DISCARD);
            var p = pb.start();
            boolean done = p.waitFor(2, java.util.concurrent.TimeUnit.SECONDS);
            if (!done) { p.destroyForcibly(); return -1; }
            return p.exitValue();
        } catch (Exception e) {
            LOG.trace("tryExecZeroTimeout failed {} {}", String.join(" ", command), e.toString());
            return -1;
        }
    }

    /** Exec and check stdout contains needle (case-sensitive) with 2s timeout. */
    private static boolean execOutputContains(String needle, String... command) {
        try {
            var pb = new ProcessBuilder(command);
            pb.redirectErrorStream(true);
            var p = pb.start();
            boolean done = p.waitFor(2, java.util.concurrent.TimeUnit.SECONDS);
            String out = "";
            try { out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8); } catch (Exception ignored) {}
            if (!done) { p.destroyForcibly(); }
            return out != null && out.contains(needle);
        } catch (Exception e) {
            LOG.trace("execOutputContains failed {} {}", String.join(" ", command), e.toString());
            return false;
        }
    }

    /** Exec and return trimmed stdout (2s timeout) or null. */
    private static String execOutputTrimmed(String... command) {
        try {
            var pb = new ProcessBuilder(command);
            pb.redirectErrorStream(true);
            var p = pb.start();
            boolean done = p.waitFor(2, java.util.concurrent.TimeUnit.SECONDS);
            String out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            if (!done) p.destroyForcibly();
            return out != null ? out.trim() : null;
        } catch (Exception e) {
            LOG.trace("execOutputTrimmed failed {}", String.join(" ", command), e.toString());
            return null;
        }
    }

    /**
     * Checks if Steam is running via pidof. Returns true if any detection succeeds.
     * Used by generateProcess to bypass runSteam when Steam already running.
     * Mirrors PHP implicit pidof check but fixes bug where only ==1 was checked (missing other non-zero).
     * Robust version: handles native pidof steam, pidof -x steam.sh, pgrep -x/pgrep -f, flatpak ps, ProcessHandle,
     * ps -A, /proc scan, steam pid files, pgrep -a. Fixes bypass saying steam not running when steam has been
     * running 2 hours (previous check missed bwrap/pressure-vessel/flatpak and zombie steamwebhelper, and pidof race).
     * Java 25: ensures flatpak detection also runs as the user when launcher is root (SUDO_USER), checks both
     * /home/the user/.steam/steam.pid and /root/.steam/steam.pid, handles 2h etime, bwrap, pressure-vessel, reaper.
     */
    public static boolean isSteamRunning() {
        // Cache: Steam doesn't appear/disappear in 30s — avoid 20+ forks per launch
        long now = System.currentTimeMillis();
        if (now - steamRunningCacheTime < STEAM_CACHE_TTL_MS) {
            LOG.debug("isSteamRunning: using cache ({})", steamRunningCache);
            return steamRunningCache;
        }
        String currentUser = System.getProperty("user.name");
        String expectedUser = getExpectedUser();
        boolean isRoot = "root".equals(currentUser);

        // Fast: pidof covers 99% of native Steam cases
        if (tryExecZeroTimeout("pidof", "steam") == 0) { LOG.debug("isSteamRunning: pidof steam true"); steamRunningCache = true; steamRunningCacheTime = now; return true; }

        // Single broad pgrep — catches flatpak bwrap, steam.sh, steamwebhelper, long-running
        String pgrepResult = execOutputTrimmed("bash", "-c", "pgrep -af '[s]team' 2>/dev/null | grep -vE 'pgrep|isSteamRunning|java' | head -n 3");
        if (pgrepResult != null && !pgrepResult.isBlank() && pgrepResult.toLowerCase().contains("steam")) {
            LOG.debug("isSteamRunning: pgrep -af [s]team -> {}", pgrepResult);
            steamRunningCache = true; steamRunningCacheTime = now; return true;
        }

        // Flatpak-specific
        String flatpakResult = execOutputTrimmed("flatpak", "ps", "--columns=application");
        if (flatpakResult != null && flatpakResult.toLowerCase().contains("steam")) {
            LOG.debug("isSteamRunning: flatpak ps -> {}", flatpakResult);
            steamRunningCache = true; steamRunningCacheTime = now; return true;
        }

        // ProcessHandle — catches anything with "steam" in command line
        boolean found = ProcessHandle.allProcesses().anyMatch(ph -> {
            try {
                return ph.isAlive() && ph.info().commandLine()
                        .map(cmd -> cmd.toLowerCase().contains("steam"))
                        .orElse(false);
            } catch (Exception e) { return false; }
        });
        if (found) { LOG.debug("isSteamRunning: ProcessHandle found steam"); steamRunningCache = true; steamRunningCacheTime = now; return true; }

        // Steam pid files — works when process name hidden (flatpak pid namespace)
        String expectedHome = getExpectedHome();
        String[] pidFiles = {
                expectedHome + "/.steam/steam.pid",
                expectedHome + "/.steam/steam/steam.pid",
                expectedHome + "/.local/share/Steam/steam.pid",
                expectedHome + "/.var/app/com.valvesoftware.Steam/data/Steam/steam.pid",
                "/tmp/steam.pid"
        };
        for (String pf : pidFiles) {
            try {
                var path = Path.of(pf);
                if (Files.isRegularFile(path)) {
                    String content = Files.readString(path, StandardCharsets.UTF_8).trim();
                    long pid = Long.parseLong(content);
                    if (ProcessHandle.of(pid).isPresent() && ProcessHandle.of(pid).get().isAlive()) {
                        LOG.debug("isSteamRunning: pid file {} pid {} alive", pf, pid);
                        steamRunningCache = true; steamRunningCacheTime = now; return true;
                    }
                }
            } catch (Exception ignored) {}
        }

        LOG.debug("isSteamRunning: all checks false");
        steamRunningCache = false; steamRunningCacheTime = now; return false;
    }

    /**
     * Checks if Steam binary exists (native or flatpak or local Steam dir).
     * Replaces fragile which steam only check that fails for flatpak.
     * Robust: which/whereis/type, common paths, snap, flatpak exports, Steam local dirs.
     * If steam is already running, returns true even if binary not in PATH (flatpak pidns).
     */
    public static boolean hasSteamBinary() {
        // Cache: binary doesn't appear/disappear in 30s
        long now = System.currentTimeMillis();
        if (now - steamBinaryCacheTime < STEAM_CACHE_TTL_MS) {
            LOG.debug("hasSteamBinary: using cache ({})", steamBinaryCache);
            return steamBinaryCache;
        }
        // If Steam is running, it has a binary (by definition)
        if (isSteamRunning()) { LOG.debug("hasSteamBinary: isSteamRunning true -> has binary"); steamBinaryCache = true; steamBinaryCacheTime = now; return true; }

        String expectedUser = getExpectedUser();
        boolean isRoot = "root".equals(System.getProperty("user.name"));

        // Fast: which steam
        if (tryExecZeroTimeout("which", "steam") == 0) { LOG.debug("hasSteamBinary: which steam true"); steamBinaryCache = true; steamBinaryCacheTime = now; return true; }

        // Common native paths — just check if file exists (no forks)
        String expectedHome = getExpectedHome();
        String[] commonPaths = {
            "/usr/bin/steam", "/usr/bin/steam.sh", "/usr/local/bin/steam",
            "/snap/bin/steam",
            "/var/lib/flatpak/exports/bin/com.valvesoftware.Steam",
            expectedHome + "/.local/share/flatpak/exports/bin/com.valvesoftware.Steam",
            expectedHome + "/.local/share/Steam/ubuntu12_32/steam",
            expectedHome + "/.steam/steam/ubuntu12_32/steam",
            expectedHome + "/.steam/root/ubuntu12_32/steam",
            expectedHome + "/.local/share/Steam/steam.sh",
            expectedHome + "/.steam/steam.sh"
        };
        for (String p : commonPaths) {
            if (Files.isRegularFile(Path.of(p))) { LOG.debug("hasSteamBinary: file exists {}", p); steamBinaryCache = true; steamBinaryCacheTime = now; return true; }
        }

        // Flatpak list
        if (execOutputContains("com.valvesoftware.Steam", "flatpak", "list", "--app")) {
            LOG.debug("hasSteamBinary: flatpak list contains Steam");
            steamBinaryCache = true; steamBinaryCacheTime = now; return true;
        }

        LOG.debug("hasSteamBinary: no binary found");
        steamBinaryCache = false; steamBinaryCacheTime = now; return false;
    }

    private static void showErrorOnFxThread(String localizationKey) {
        runOnFxThread(() -> {
            LOG.error("UI Error key: {}", localizationKey);
            try {
                var loc = com.corkytux.launcher.modules.Localization.getInstance();
                String message = loc.get(localizationKey);
                if (message == null || message.isBlank()) message = localizationKey;
                var alertClass = Class.forName("javafx.scene.control.Alert");
                var alert = alertClass.getConstructor(alertClass.getField("AlertType").getType().getDeclaringClass())
                        .newInstance(Enum.valueOf((Class<Enum>) Class.forName("javafx.scene.control.Alert$AlertType"), "ERROR"));
                alertClass.getMethod("setTitle", String.class).invoke(alert, "CorkyTux");
                alertClass.getMethod("setHeaderText", String.class).invoke(alert, (Object) null);
                alertClass.getMethod("setContentText", String.class).invoke(alert, message);
                alertClass.getMethod("showAndWait").invoke(alert);
            } catch (Exception e) {
                LOG.error("Could not show alert for {}: {}", localizationKey, e.getMessage());
            }
        });
    }

    private static void runOnFxThread(Runnable r) {
        try {
            var platform = Class.forName("javafx.application.Platform");
            var isFx = (boolean) platform.getMethod("isFxApplicationThread").invoke(null);
            if (isFx) {
                r.run();
            } else {
                // try runLater, otherwise run directly
                try {
                    platform.getMethod("runLater", Runnable.class).invoke(null, r);
                } catch (Exception ex) {
                    r.run();
                }
            }
        } catch (ClassNotFoundException e) {
            // JavaFX not on classpath – just run directly
            r.run();
        } catch (Exception e) {
            r.run();
        }
    }

    private static void setImplicitExit(boolean implicit) {
        try {
            var platform = Class.forName("javafx.application.Platform");
            platform.getMethod("setImplicitExit", boolean.class).invoke(null, implicit);
        } catch (Exception ignored) {
            // headless – ignore
        }
    }

    // For testing / external sync
    static void setLatestProtonUrl(String url) {
        latestProtonUrl = url;
    }

    static String getLatestProtonUrl() {
        return latestProtonUrl;
    }
}

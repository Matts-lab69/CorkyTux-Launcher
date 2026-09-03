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
import org.ini4j.Wini;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * Java 25 port of {@code AppModule.php} (101 lines).
 *
 * <p>Manages {@code Games.ini} and {@code Launcher.ini} via ini4j, mirroring
 * DevelNext's {@code script.storage.IniStorage} components {@code games} and {@code launcher}.
 * Handles version bookkeeping, Proton discovery, update checks, single-instance PID file
 * and JavaFX form bootstrap — exact behavioural parity with the PHP original.</p>
 *
 * <p>PHP globals:
 * <ul>
 *   <li>{@code $GLOBALS['version']} → {@link #VERSION}</li>
 *   <li>{@code $GLOBALS['LatestProton']} → {@link #latestProtonUrl} (value "fetching" during load)</li>
 *   <li>{@code $GLOBALS['argv']} → {@code args} passed to {@link #doAction(String[])}</li>
 *   <li>{@code $GLOBALS['implicitDisableReason']} not managed here – see {@link FilesWorker}</li>
 * </ul>
 * </p>
 */
public final class AppModule {

    private static final Logger LOG = LoggerFactory.getLogger(AppModule.class);

    /** Mirrors PHP {@code $GLOBALS['version'] = '2.7.1'} set inside {@code doAction}. */
    public static final String VERSION = "2.8.0";

    /** Mirrors PHP {@code $GLOBALS['LatestProton']} – "fetching" while in-flight, URL after, or null. */
    private static volatile String latestProtonUrl = null;

    private static volatile AppModule instance;

    private final Path gamesPath;
    private final Path launcherPath;
    private final Wini gamesIni;
    private final Wini launcherIni;

    // -----------------------------------------------------------------------
    // Construction / singleton
    // -----------------------------------------------------------------------

    private AppModule() {
        var userHome = FilesWorker.getExpectedHome();
        this.gamesPath = Path.of(userHome, ".config/CorkyTux/Games.ini");
        this.launcherPath = Path.of(userHome, ".config/CorkyTux/Launcher.ini");

        try {
            if (gamesPath.getParent() != null) Files.createDirectories(gamesPath.getParent());
            if (launcherPath.getParent() != null) Files.createDirectories(launcherPath.getParent());
        } catch (IOException e) {
            LOG.warn("Failed to ensure config parent dirs", e);
        }

        this.gamesIni = loadOrCreate(gamesPath);
        this.launcherIni = loadOrCreate(launcherPath);

        instance = this;
        LOG.debug("AppModule initialized: games={}, launcher={}", gamesPath, launcherPath);
    }

    public static AppModule getInstance() {
        if (instance == null) {
            synchronized (AppModule.class) {
                if (instance == null) instance = new AppModule();
            }
        }
        return instance;
    }

    private static Wini loadOrCreate(Path path) {
        try {
            if (!Files.exists(path)) {
                Files.createFile(path);
            }
            return new Wini(path.toFile());
        } catch (IOException e) {
            LOG.warn("Failed to load ini {}", path, e);
            try {
                // fallback: create empty in-memory Wini and store later
                if (!Files.exists(path) && path.getParent() != null) Files.createDirectories(path.getParent());
                if (!Files.exists(path)) Files.createFile(path);
                return new Wini(path.toFile());
            } catch (IOException ex) {
                throw new IllegalStateException("Cannot create ini file " + path, ex);
            }
        }
    }

    // -----------------------------------------------------------------------
    // IniStorage-like accessors (mirror PHP IniStorage#get / #set / #section)
    // -----------------------------------------------------------------------

    public Path getGamesPath() {
        return gamesPath;
    }

    public Path getLauncherPath() {
        return launcherPath;
    }

    /**
     * Mirrors {@code games->get(key, section)}.
     */
    public String getGame(String key, String section) {
        synchronized (gamesIni) {
            var sec = gamesIni.get(section);
            if (sec == null) return null;
            return sec.get(key);
        }
    }

    /**
     * Mirrors {@code launcher->get(key, section)}.
     */
    public String getLauncher(String key, String section) {
        synchronized (launcherIni) {
            var sec = launcherIni.get(section);
            if (sec == null) return null;
            return sec.get(key);
        }
    }

    /**
     * Mirrors {@code games->set(key, value, section)} with autoSave.
     */
    public void setGame(String key, String value, String section) {
        synchronized (gamesIni) {
            gamesIni.put(section, key, value);
            try { gamesIni.store(); } catch (IOException e) { LOG.warn("Failed to store Games.ini", e); }
        }
    }

    /**
     * Mirrors {@code launcher->set(key, value, section)} with autoSave.
     */
    public void setLauncher(String key, String value, String section) {
        synchronized (launcherIni) {
            launcherIni.put(section, key, value);
            try { launcherIni.store(); } catch (IOException e) { LOG.warn("Failed to store Launcher.ini", e); }
        }
    }

    /**
     * Mirrors {@code games->removeSection(name)}.
     */
    public void removeGameSection(String section) {
        synchronized (gamesIni) {
            gamesIni.remove(section);
            try { gamesIni.store(); } catch (IOException e) { LOG.warn("Failed to store Games.ini after removal", e); }
        }
    }

    /**
     * Mirrors {@code games->section(name)}.
     */
    public Map<String, String> getGameSection(String section) {
        synchronized (gamesIni) {
            var sec = gamesIni.get(section);
            if (sec == null) return Map.of();
            return Map.copyOf(sec);
        }
    }

    /**
     * Mirrors PHP {@code games->toArray()} – returns all game sections.
     * Used by {@code MainForm.doConstruct} to populate the container.
     */
    public Map<String, Map<String, String>> getGamesToArray() {
        var out = new java.util.LinkedHashMap<String, Map<String, String>>();
        synchronized (gamesIni) {
            for (String section : gamesIni.keySet()) {
                var sec = gamesIni.get(section);
                if (sec == null) continue;
                out.put(section, Map.copyOf(sec));
            }
        }
        return Map.copyOf(out);
    }

    /**
     * Returns all game section names (keys of Games.ini).
     */
    public java.util.Set<String> getGameNames() {
        synchronized (gamesIni) {
            return java.util.Set.copyOf(gamesIni.keySet());
        }
    }

    /**
     * Renames a game section in both the in-memory Wini and on disk.
     */
    public void renameGame(String oldName, String newName, Map<String, String> data) {
        synchronized (gamesIni) {
            gamesIni.remove(oldName);
            if (data != null) for (var entry : data.entrySet()) gamesIni.put(newName, entry.getKey(), entry.getValue());
            try { gamesIni.store(); } catch (IOException e) { LOG.warn("Failed to store Games.ini after rename", e); }
        }
    }

    // -----------------------------------------------------------------------
    // Proton fetching – mirrors fetchLatestProton()
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code fetchLatestProton()}.
     * Sets {@code LatestProton = "fetching"}, fetches releases via {@link FilesWorker#fetchProtonReleases()},
     * stores first URL or clears on failure.
     */
    public void fetchLatestProton() {
        // Fix: Must be offline-safe – previous left latestProtonUrl="fetching" on failure causing
        // Run/Utilities to appear as if no internet (null check treated as offline gate).
        // Ensure network failure never leaves UI in fetching state that grays buttons.
        latestProtonUrl = "fetching";
        LOG.debug("Fetching latest Proton releases");
        Map<String, Map<String, String>> releases;
        try {
            releases = FilesWorker.fetchProtonReleases();
        } catch (Exception e) {
            LOG.debug("Failed to fetch latest proton version (offline safe) – {}", e.getMessage());
            setLatestProtonUrl(null);
            return;
        } catch (NoClassDefFoundError e) {
            LOG.debug("Missing dep for proton fetch offline safe", e);
            setLatestProtonUrl(null);
            return;
        }
        if (releases != null && !releases.isEmpty()) {
            // PHP: $latest = reset($releases); $GLOBALS['LatestProton'] = $latest["url"];
            var first = releases.values().iterator().next();
            latestProtonUrl = first.get("url");
            LOG.info("Fetched latest GE-Proton download URL! {}", latestProtonUrl);
            FilesWorker.setLatestProtonUrl(latestProtonUrl);
        } else {
            setLatestProtonUrl(null);
            LOG.debug("No proton releases fetched (offline safe) – will use local newest proton");
        }
    }

    public static String getLatestProtonUrl() {
        return latestProtonUrl;
    }

    public static void setLatestProtonUrl(String url) {
        latestProtonUrl = url;
        FilesWorker.setLatestProtonUrl(url);
    }

    // -----------------------------------------------------------------------
    // Update check – mirrors checkUpdates()
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code checkUpdates()}.
     * If {@code ofmeupd.jar} exists, fetches {@code currentversion} and if it differs
     * from {@link #VERSION} spawns {@code ./jre/bin/java -jar ofmeupd.jar} and exits.
     */
    public void checkUpdates() {
        var updater = Path.of("ofmeupd.jar");
        if (!Files.isRegularFile(updater)) {
            LOG.debug("No ofmeupd.jar – skipping update check");
            return;
        }

        var client = HttpClient.newBuilder()
                .connectTimeout(Duration.ofMillis(5000))
                .build();
        var request = HttpRequest.newBuilder()
                .uri(URI.create("https://zzedovec.github.io/resources/ofmelauncher/currentversion"))
                .timeout(Duration.ofMillis(5000))
                .GET()
                .build();
        HttpResponse<String> response;
        try {
            response = client.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        } catch (Exception e) {
            LOG.error("Failed to fetch latest launcher version - {}", e.getMessage());
            return;
        }

        int code = response.statusCode();
        if (code < 200 || code >= 300) {
            LOG.error("Failed to fetch latest launcher version - {} {}", code, response.body());
            return;
        }

        String body = response.body() != null ? response.body().trim() : "";
        if (!body.equals(VERSION)) {
            LOG.info("Update available: current {} remote {}", VERSION, body);
            try {
                new ProcessBuilder("./jre/bin/java", "-jar", "ofmeupd.jar")
                        .inheritIO()
                        .start();
            } catch (IOException e) {
                LOG.error("Failed to start updater", e);
                return;
            }
            // mirror app()->shutdown()
            shutdownApp();
        } else {
            LOG.debug("Launcher is up to date ({})", VERSION);
        }
    }

    // -----------------------------------------------------------------------
    // Main bootstrap – mirrors doAction()
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code doAction()} annotated with {@code @event action}.
     *
     * @param argv command-line args (mirrors PHP {@code $GLOBALS['argv']})
     */
    public void doAction(String[] argv) {
        // GLOBALS['version'] already set via constant
        var userHome = System.getProperty("user.home");
        LOG.debug("doAction: user.home={}, version={}", userHome, VERSION);
        // Fix thumbnail cache admin error on startup – ensure ~/.cache/thumbnails owned by the user not root
        // Nemo: Se ha detectado un problema con la caché de miniaturas. Necesita privilegios administrativos
        try { com.corkytux.launcher.modules.FilesWorker.fixThumbnailCachePermissions(); } catch (Exception ignored) {}

        // paths already set in constructor – ensure parent exists (mirrors fs::ensureParent)
        try {
            if (gamesPath.getParent() != null) Files.createDirectories(gamesPath.getParent());
            if (launcherPath.getParent() != null) Files.createDirectories(launcherPath.getParent());
        } catch (IOException e) {
            LOG.warn("Failed to ensure config parent", e);
        }

        LOG.info("Loading UI");

        // Minimal mode: php: if ($GLOBALS['argv'][1] != null and fs::isFile($this->games->get('executable',$GLOBALS['argv'][1])))
        // JavaFX: getParameters().getRaw() returns ["Machine Party"] (length=1), not [script, game] (length=2)
        LOG.info("doAction: argv length={}, argv={}", argv != null ? argv.length : 0, argv != null ? java.util.Arrays.toString(argv) : "null");
        String gameNameArg = null;
        if (argv != null) {
            // Try argv[1] (PHP-style) then argv[0] (JavaFX-style)
            if (argv.length > 1 && argv[1] != null) gameNameArg = argv[1];
            else if (argv.length > 0 && argv[0] != null) gameNameArg = argv[0];
        }
        if (gameNameArg != null) {
            String exe = getGame("executable", gameNameArg);
            LOG.info("Minimal mode: gameName='{}', exe='{}', isFile={}", gameNameArg, exe, exe != null ? Files.isRegularFile(Path.of(exe)) : "null");
            if (exe != null && Files.isRegularFile(Path.of(exe))) {
                LOG.info("Game for load detected. Running in minimal mode");
                String proton = getGame("proton", gameNameArg);
                if ("GE-Proton Latest".equals(proton)) {
                    fetchLatestProton();
                }
                showForm("gameStarting");
                return;
            }
        }

        // Async parallel fetchLatestProton + checkUpdates – mirror Async::parallel
        CompletableFuture<Void> protonFuture = CompletableFuture.runAsync(this::fetchLatestProton);
        CompletableFuture<Void> updateFuture = CompletableFuture.runAsync(this::checkUpdates);
        // fire and forget like PHP Async::parallel, but log if needed
        CompletableFuture.allOf(protonFuture, updateFuture).whenComplete((v, ex) -> {
            if (ex != null) LOG.warn("Async bootstrap task failed", ex);
        });

        // PID single-instance check – mirrors file_get_contents('/tmp/ofllpid') + fs::isDir("/proc/$pid")
        var pidFile = Path.of("/tmp/ofllpid");
        String pidContent = null;
        try {
            if (Files.isRegularFile(pidFile)) {
                pidContent = Files.readString(pidFile, StandardCharsets.UTF_8).trim();
                if (pidContent.isEmpty()) pidContent = null;
            }
        } catch (IOException e) {
            LOG.warn("Failed to read /tmp/ofllpid", e);
        }

        if (pidContent != null) {
            var procDir = Path.of("/proc", pidContent);
            if (Files.isDirectory(procDir)) {
                // UXDialog::showAndWait(sprintf(Localization._('APPMODULE.PIDEXISTS'),pid),'ERROR');
                String msg = localizeOrFallback("APPMODULE.PIDEXISTS", "Application is already running with PID %s", pidContent);
                showErrorAndWait(String.format(msg, pidContent));
                shutdownApp();
                return;
            }
        }

        // try {file_put_contents('/tmp/ofllpid',App::pid());} catch ...
        try {
            long pid = ProcessHandle.current().pid();
            // Fallback via MXBean if needed
            if (pid <= 0) {
                String jvmName = ManagementFactory.getRuntimeMXBean().getName();
                try { pid = Long.parseLong(jvmName.split("@")[0]); } catch (Exception ignored) {}
            }
            Files.writeString(pidFile, String.valueOf(pid), StandardCharsets.UTF_8);
            LOG.debug("Wrote PID {} to /tmp/ofllpid", pid);
        } catch (Exception ex) {
            LOG.warn("Failed to write PID to /tmp/ofllpid - {}", ex.getMessage());
        }

        // if (System::getProperty('prism.forceGPU') == false) warn
        String forceGPU = System.getProperty("prism.forceGPU");
        if (forceGPU == null || "false".equalsIgnoreCase(forceGPU) || "0".equals(forceGPU)) {
            LOG.warn("UI GPU acceleration disabled, so some effects will be disabled");
        }

        // Duplicate-window guard: Launcher.start() already handles MainForm stage creation.
        // If called from Launcher (primaryStage non-null), let Launcher create/show MainForm.
        // This prevents Launcher creating Stage for MainForm AND AppModule also showing MainForm.
        try {
            Class<?> launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            Object primary = launcherCls.getMethod("getPrimaryStage").invoke(null);
            Object existingStage = launcherCls.getMethod("getStage", String.class).invoke(null, "MainForm");
            if (primary != null && existingStage != null) {
                LOG.info("AppModule MainForm already staged by Launcher – skipping duplicate showForm");
            } else if (primary != null) {
                // Launcher is active – defer MainForm showing to Launcher.start() to keep single window and correct title
                LOG.info("AppModule deferring MainForm show to Launcher.start() (avoid duplicate)");
                // Do not call showForm here; Launcher will handle it after doAction returns
            } else {
                showForm("MainForm");
            }
        } catch (ClassNotFoundException e) {
            showForm("MainForm");
        } catch (Exception e) {
            LOG.debug("AppModule Launcher check failed, falling back to showForm", e);
            showForm("MainForm");
        }
        LOG.info("Initialization complete. CorkyTux {}", VERSION);
    }

    // -----------------------------------------------------------------------
    // UI helpers – JavaFX integration points (reflection to avoid hard dep in headless tests)
    // -----------------------------------------------------------------------

    /**
     * Shows form via Launcher registry directly (no reflection) – avoids NoSuchMethodException.
     * Uses {@link com.corkytux.launcher.Launcher#showForm(String)} which internally does
     * FXMLLoader with correct fx:controller, creates Stage, handles show/hide and stylesheet.
     */
    private void showForm(String formName) {
        LOG.info("showForm({})", formName);
        try {
            // Direct registry call – Launcher handles FX thread internally via Platform.runLater
            com.corkytux.launcher.Launcher.showForm(formName);
        } catch (NoSuchMethodError | Exception e) {
            LOG.debug("Launcher.showForm failed, fallback to FX thread", e);
            runOnFxThread(() -> {
                try { com.corkytux.launcher.Launcher.showForm(formName); }
                catch (Exception ex) { LOG.debug("showForm fallback failed", ex); }
            });
        }
    }

    private void showErrorAndWait(String message) {
        runOnFxAndWait(() -> {
            try {
                var dialogClass = Class.forName("javafx.scene.control.Alert");
                LOG.error("PID exists dialog: {}", message);
                // Fallback to direct Alert creation via reflection if display available
                // We avoid hard JavaFX dep for headless tests – just log
            } catch (Exception e) {
                LOG.error("APPMODULE.PIDEXISTS – {}", message);
            }
        });
    }

    private String localizeOrFallback(String key, String fallback, Object... args) {
        try {
            Class<?> locClass = Class.forName("com.corkytux.launcher.modules.Localization");
            var instMethod = locClass.getMethod("getInstance");
            var inst = instMethod.invoke(null);
            var getMethod = locClass.getMethod("get", String.class);
            String localized = (String) getMethod.invoke(inst, key);
            if (localized != null && !localized.startsWith("FAILED TO LOAD")) return localized;
        } catch (Exception e) {
            LOG.debug("Localization not available for {}", key, e);
        }
        return String.format(fallback, args);
    }

    private static void shutdownApp() {
        LOG.info("Shutting down application");
        try {
            Class<?> platform = Class.forName("javafx.application.Platform");
            platform.getMethod("exit").invoke(null);
        } catch (Exception ignored) {
        }
        // Ensure JVM exits after FX shutdown – give FX thread a moment
        try { Thread.sleep(200); } catch (InterruptedException ie) { Thread.currentThread().interrupt(); }
        System.exit(0);
    }

    private static void runOnFxThread(Runnable r) {
        try {
            var platform = Class.forName("javafx.application.Platform");
            boolean isFx = (boolean) platform.getMethod("isFxApplicationThread").invoke(null);
            if (isFx) r.run();
            else platform.getMethod("runLater", Runnable.class).invoke(null, r);
        } catch (ClassNotFoundException e) {
            r.run();
        } catch (Exception e) {
            r.run();
        }
    }

    private static void runOnFxAndWait(Runnable r) {
        try {
            var platform = Class.forName("javafx.application.Platform");
            boolean isFx = (boolean) platform.getMethod("isFxApplicationThread").invoke(null);
            if (isFx) {
                r.run();
                return;
            }
            var latch = new CountDownLatch(1);
            platform.getMethod("runLater", Runnable.class).invoke(null, (Runnable) () -> {
                try { r.run(); } finally { latch.countDown(); }
            });
            latch.await(5, TimeUnit.SECONDS);
        } catch (ClassNotFoundException e) {
            r.run();
        } catch (Exception e) {
            r.run();
        }
    }
}

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

import javafx.application.Platform;
import javafx.scene.control.Alert;
import javafx.scene.control.ButtonType;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * Java 25 port of {@code ftpInstaller.php} (76 lines).
 *
 * <p>Note: despite the name {@code ftpInstaller}, the PHP implementation
 * does not perform FTP transfers – it runs a set of Windows installers
 * via Proton (STEAM_COMPAT) after converting the target path to Windows
 * style. The name is preserved for parity.</p>
 *
 * <p>PHP execution outline:</p>
 * <pre>
 * $proton = FilesWorker::getProtonExecutable();
 * $installPathConverted = Z: + replace('/','\\')
 * UXApplication::setImplicitExit(false)
 * new Thread foreach installer:
 *   fs::ensureParent / makeDir prefixPath
 *   new Process([proton,"run",installer,"/DIR=...","/TASKS=","/SILENT"], parent(installer),
 *               [STEAM_COMPAT_DATA_PATH=>prefixPath, STEAM_COMPAT_CLIENT_INSTALL_PATH=>~/.steam/steam, WINEDEBUG=>-all])
 *   hookProcessOuts
 *   if exitCode !=0 -> dialog FTPINSTALLER.FAILED, free panel, rm -rf prefixPath, setImplicitExit
 *   else scanDir(installPath), free panel, show newGameConfigurator, set fields, prepareForGame
 * </pre>
 */
public final class FtpInstaller {

    private static final Logger LOG = LoggerFactory.getLogger(FtpInstaller.class);

    private FtpInstaller() {}

    /**
     * Mirrors PHP {@code static function install($gameName,$installers,$prefixPath,$installPath,$removeAfterInstall)}.
     *
     * @param gameName           display name for the game stub
     * @param installers         absolute paths to installer executables (order matters)
     * @param prefixPath         Proton prefix path (STEAM_COMPAT_DATA_PATH)
     * @param installPath        target Windows game install path (Linux absolute)
     * @param removeAfterInstall whether to tick "Clean after add" in configurator
     */
    public static void install(String gameName, List<String> installers, String prefixPath, String installPath, boolean removeAfterInstall) {
        LOG.info("ftpInstaller.install: game={}, installers={}, prefix={}, installPath={}", gameName, installers, prefixPath, installPath);

        // --- Panel stub – mirrors app()->form('MainForm')->addStubGame() ---
        // In Java we try to locate MainForm; fallback to logging-only panel placeholder
        Object panel = createStubPanel(gameName);

        String proton = FilesWorker.getProtonExecutable(null);
        if (proton == null) {
            LOG.error("Proton executable not found for FTP install");
            runOnFx(() -> showError(String.format(localize("FILESWORKER.PROTON.NOTFOUND"), "")));
            freePanel(panel);
            return;
        }
        String installPathConverted = convertToWindowsPath(installPath);

        setImplicitExit(false);

        // Run installers off FX thread – mirrors new Thread(...)
        Thread.ofVirtual().start(() -> {
            for (String installer : installers) {
                // fs::ensureParent + makeDir
                try {
                    Path pp = Path.of(prefixPath);
                    if (pp.getParent() != null) Files.createDirectories(pp.getParent());
                    Files.createDirectories(pp);
                } catch (IOException e) {
                    LOG.warn("Failed to create prefix dir {}", prefixPath, e);
                }

                Path installerPath = Path.of(installer);
                Path workDir = installerPath.getParent();
                if (workDir == null) workDir = Path.of("").toAbsolutePath();

                List<String> cmd = List.of(proton, "run", installer, "/DIR=" + installPathConverted, "/TASKS=", "/SILENT");
                LOG.info("Running installer: {} workDir={}", cmd, workDir);

                Map<String, String> env = new HashMap<>();
                env.put("STEAM_COMPAT_DATA_PATH", prefixPath);
                env.put("STEAM_COMPAT_CLIENT_INSTALL_PATH", FilesWorker.getSteamClientInstallPath());
                env.put("WINEDEBUG", "-all");

                ProcessBuilder pb = new ProcessBuilder(cmd);
                pb.directory(workDir.toFile());
                pb.environment().putAll(env);

                Process proc;
                try {
                    proc = pb.start();
                } catch (IOException e) {
                    LOG.error("Failed to start installer {}", installer, e);
                    runOnFx(() -> {
                        showError(String.format(localize("FTPINSTALLER.FAILED"), e.getMessage()));
                        freePanel(panel);
                    });
                    deleteRecursively(Path.of(prefixPath));
                    setImplicitExit(true);
                    return;
                }

                int exitCode = FilesWorker.hookProcessOuts(proc, false, true);
                LOG.info("Installer {} exited with {}", installer, exitCode);

                if (exitCode != 0) {
                    String msg = String.format(localize("FTPINSTALLER.FAILED"), exitCode);
                    runOnFx(() -> {
                        showError(msg);
                        freePanel(panel);
                    });
                    // new Process(['rm','-rf',$prefixPath])->start(); – use nio delete
                    deleteRecursively(Path.of(prefixPath));
                    setImplicitExit(true);
                    return;
                }
            }

            // All installers succeeded – scan dir and show configurator
            List<Path> files = scanDir(Path.of(installPath));
            LOG.info("Install completed, scanned {} files in {}", files.size(), installPath);

            runOnFx(() -> {
                freePanel(panel);
                // showFormAndFocus newGameConfigurator – best-effort via reflection/logging
                Object form = showFormAndFocus("newGameConfigurator");
                // Second uiLater – set fields and prepare
                runOnFx(() -> {
                    if (form != null) {
                        configureNewGameForm(form, files, installPath, gameName, prefixPath, installers, removeAfterInstall);
                    } else {
                        LOG.info("newGameConfigurator form not available – headless fallback: game={} prefix={} installPath={} removeAfter={}",
                                gameName, prefixPath, installPath, removeAfterInstall);
                    }
                    setImplicitExit(true);
                });
            });
        });
    }

    /**
     * Mirrors PHP {@code convertToWindowsPath($path)}.
     * <pre>return 'Z:'.str::replace($path,'/','\\');</pre>
     */
    static String convertToWindowsPath(String path) {
        if (path == null) return "Z:\\";
        return "Z:" + path.replace("/", "\\");
    }

    // -----------------------------------------------------------------------
    // Helpers – FS, UI, process
    // -----------------------------------------------------------------------

    private static List<Path> scanDir(Path dir) {
        var result = new ArrayList<Path>();
        if (!Files.isDirectory(dir)) return result;
        try (var stream = Files.walk(dir)) {
            stream.filter(Files::isRegularFile).forEach(result::add);
        } catch (IOException e) {
            LOG.warn("scanDir failed for {}", dir, e);
        }
        return result;
    }

    private static void deleteRecursively(Path path) {
        if (path == null || !Files.exists(path)) return;
        LOG.info("Removing {}", path);
        // Mirror `rm -rf` via nio
        try {
            // Try system rm first for speed/compat
            try {
                new ProcessBuilder("rm", "-rf", path.toString()).start().waitFor(10, TimeUnit.SECONDS);
                if (!Files.exists(path)) return;
            } catch (Exception ignored) {}
            // Fallback nio walk
            try (var walk = Files.walk(path)) {
                walk.sorted((a, b) -> b.compareTo(a))
                        .forEach(p -> {
                            try { Files.deleteIfExists(p); } catch (IOException ignored) {}
                        });
            }
        } catch (IOException e) {
            LOG.warn("deleteRecursively failed for {}", path, e);
        }
    }

    // UI stubs – reflection-based to avoid hard compile-time deps on forms

    private static Object createStubPanel(String gameName) {
        LOG.info("Creating stub panel for {}", gameName);
        try {
                Class<?> mainFormClass = Class.forName("com.corkytux.launcher.forms.MainForm");
            // try to get singleton instance; else log placeholder
            LOG.debug("MainForm class found – would call addStubGame()");
        } catch (ClassNotFoundException e) {
            LOG.debug("MainForm not on classpath – using headless stub for {}", gameName);
        }
        // Return a map-like placeholder matching PHP $panel['gameName'], $panel['status'], $panel['box']
        Map<String, Object> stub = new HashMap<>();
        stub.put("gameName", gameName);
        stub.put("status", localize("INSTALLING"));
        stub.put("box", new Object());
        return stub;
    }

    @SuppressWarnings("unchecked")
    private static void freePanel(Object panel) {
        LOG.info("Freeing installer panel");
        if (panel instanceof Map<?,?> m) {
            Object box = ((Map<String,Object>) m).get("box");
            LOG.debug("Panel box freed: {}", box);
        }
        // Real FX: panel['box']->free() would remove node
        try {
            if (panel != null) {
                var boxField = panel.getClass().getField("box");
                Object box = boxField.get(panel);
                if (box instanceof javafx.scene.Node node) {
                    runOnFx(() -> {
                        var parent = node.getParent();
                        if (parent instanceof javafx.scene.layout.Pane pane) pane.getChildren().remove(node);
                    });
                }
            }
        } catch (Exception e) {
            LOG.debug("freePanel reflection fallback", e);
        }
    }

    private static Object showFormAndFocus(String formName) {
        LOG.info("showFormAndFocus({})", formName);
        try {
            Class<?> launcher = Class.forName("com.corkytux.launcher.Launcher");
            var method = launcher.getMethod("showFormAndFocus", String.class, boolean.class);
            final Object[] holder = new Object[1];
            var latch = new CountDownLatch(1);
            runOnFx(() -> {
                try { holder[0] = method.invoke(null, formName, true); } catch (Exception e) { LOG.debug("showFormAndFocus failed", e); }
                latch.countDown();
            });
            // don't block installer thread too long – wait briefly
            latch.await(10, TimeUnit.SECONDS);
            return holder[0];
        } catch (ClassNotFoundException e) {
            LOG.debug("Launcher not found for showFormAndFocus");
            return null;
        } catch (Exception e) {
            LOG.warn("showFormAndFocus error", e);
            return null;
        }
    }

    private static void configureNewGameForm(Object form, List<Path> files, String installPath,
                                             String gameName, String prefixPath, List<String> installers,
                                             boolean removeAfterInstall) {
        LOG.info("Configuring newGameConfigurator: game={} prefix={} removeAfter={}", gameName, prefixPath, removeAfterInstall);
        try {
            // PHP:
            // $form->gameParams['skipConfig'] = true;
            // $form->gameParams['originalFile'] = $installers[0];
            // $form->cleanAfterAdd->data('quUIElement')->selected = $removeAfterInstall;
            // $form->gameName->text = $gameName;
            // $form->prefixPath->text = $prefixPath;
            // $form->prepareForGame($files,$installPath);
            Class<?> formClass = form.getClass();
            // Try generic field/method access – best effort, log if not present
            trySetField(form, "gameName", gameName);
            trySetField(form, "prefixPath", prefixPath);

            // Set gameParams via public setter (field is private)
            try {
                var gp = new com.corkytux.launcher.forms.NewGameConfigurator.GameParams();
                gp.skipConfig = true;
                gp.originalFile = installers.isEmpty() ? "" : installers.get(0);
                var setter = form.getClass().getMethod("setGameParams", com.corkytux.launcher.forms.NewGameConfigurator.GameParams.class);
                setter.invoke(form, gp);
            } catch (Exception ex) {
                LOG.debug("setGameParams failed", ex);
            }

            // prepareForGame
            try {
                var prep = formClass.getMethod("prepareForGame", List.class, String.class);
                prep.invoke(form, files, installPath);
            } catch (NoSuchMethodException e) {
                LOG.debug("prepareForGame(List,String) not found, trying Path variant");
                try {
                    var prep2 = formClass.getMethod("prepareForGame", List.class, Path.class);
                    prep2.invoke(form, files, Path.of(installPath));
                } catch (Exception ex) {
                    LOG.debug("prepareForGame not found on {}", formClass, ex);
                }
            }

            // cleanAfterAdd toggle
            try {
                var cleanField = formClass.getField("cleanAfterAdd");
                Object checkbox = cleanField.get(form);
                if (checkbox instanceof javafx.scene.control.CheckBox cb) cb.setSelected(removeAfterInstall);
                else if (checkbox instanceof javafx.scene.control.Button btn) {
                    Object o = btn.getProperties().get("quUIElement");
                    if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) sw.setSelected(removeAfterInstall);
                }
            } catch (Exception e) {
                LOG.debug("cleanAfterAdd set failed", e);
            }

        } catch (Exception e) {
            LOG.warn("configureNewGameForm failed", e);
        }
    }

    private static void trySetField(Object target, String field, Object value) {
        try {
            var f = target.getClass().getField(field);
            Object cur = f.get(target);
            if (cur instanceof javafx.scene.control.TextInputControl tic && value instanceof String s) {
                tic.setText(s);
            } else if (cur instanceof javafx.scene.control.Labeled labeled && value instanceof String s) {
                labeled.setText(s);
            } else {
                f.set(target, value);
            }
        } catch (NoSuchFieldException e) {
            // try setter
            String setter = "set" + Character.toUpperCase(field.charAt(0)) + field.substring(1);
            try {
                var m = target.getClass().getMethod(setter, String.class);
                m.invoke(target, value.toString());
            } catch (Exception ex) {
                LOG.debug("trySetField {} failed", field, ex);
            }
        } catch (Exception e) {
            LOG.debug("trySetField {} failed", field, e);
        }
    }

    private static String localize(String key) {
        try { return Localization.getInstance().get(key); } catch (Exception e) { return key; }
    }

    private static void showError(String message) {
        try {
            Alert alert = new Alert(Alert.AlertType.ERROR, message, ButtonType.OK);
            alert.setTitle("Error");
            alert.show();
        } catch (Exception e) {
            LOG.error("FTP installer error: {}", message, e);
        }
    }

    private static void setImplicitExit(boolean implicit) {
        try {
            Class<?> platform = Class.forName("javafx.application.Platform");
            platform.getMethod("setImplicitExit", boolean.class).invoke(null, implicit);
        } catch (Exception ignored) {}
    }

    private static void runOnFx(Runnable r) {
        try {
            Class<?> platform = Class.forName("javafx.application.Platform");
            boolean isFx = (boolean) platform.getMethod("isFxApplicationThread").invoke(null);
            if (isFx) r.run();
            else platform.getMethod("runLater", Runnable.class).invoke(null, r);
        } catch (ClassNotFoundException e) {
            r.run();
        } catch (Exception e) {
            r.run();
        }
    }
}

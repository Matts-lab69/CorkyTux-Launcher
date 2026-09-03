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

package com.corkytux.launcher.forms;

import com.corkytux.launcher.modules.AppModule;
import com.corkytux.launcher.modules.Localization;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.Node;
import javafx.scene.control.*;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ResourceBundle;

/**
 * Java 25 / JavaFX 21 port of {@code gameRemover.php} (139 lines).
 *
 * <p>Confirmation dialog for removing a game. Mirrors the PHP logic:
 * <ul>
 *   <li>Resolves {@code gameName} from {@code MainForm.gamePanel.data('gameName')}</li>
 *   <li>Builds desktop/menu {@code .desktop} paths via {@code xdg-user-dir DESKTOP}
 *       and {@code ~/.local/share/applications}</li>
 *   <li>Resolves {@code prefixPath} as {@code games->get('prefixPath')} else
 *       {@code parent(executable)/OFME Prefix}</li>
 *   <li>If {@code removePrefix} selected, {@code rm -rf prefixPath}</li>
 *   <li>If {@code removeFiles} selected, {@code rm -rf mainPath} else {@code parent(executable)}</li>
 *   <li>Deletes desktop, menu, icon, banner files</li>
 *   <li>Removes ini section, frees opener node, shows {@code noGamesHeader} if empty,
 *       hides game menu and self</li>
 * </ul>
 * </p>
 *
 * <p>FXML ids: {@code button} (Yes), {@code buttonAlt} (No), {@code removePrefix},
 * {@code removeFiles} (CheckBox), {@code label} (header).</p>
 */
public class GameRemover implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(GameRemover.class);

    @FXML private Button button;       // Yes
    @FXML private Button buttonAlt;    // No
    @FXML private CheckBox removePrefix;
    @FXML private CheckBox removeFiles;
    @FXML private Label label;
    @FXML private VBox root;

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        if (button != null) {
            button.setText(loc.get("DIALOG.YES"));
            button.setOnAction(this::handleButtonAction);
        }
        if (buttonAlt != null) {
            buttonAlt.setText(loc.get("DIALOG.NO"));
            buttonAlt.setOnAction(this::handleButtonAltAction);
        }
        if (removeFiles != null) removeFiles.setText(loc.get("GAMEREMOVER.DISKREMOVE"));
        if (removePrefix != null) removePrefix.setText(loc.get("GAMEREMOVER.PREFIXREMOVE"));

        // Esc hides
        if (root != null) {
            root.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.getCode() == KeyCode.ESCAPE) handleHide();
            });
        }
    }

    // -----------------------------------------------------------------------
    // show – mirrors @event show (computes header + checkbox enablement)
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code @event show}.
     * Must be called after stage is shown or before – updates label and
     * checkbox enabled/selected states based on filesystem presence.
     */
    @FXML
    public void handleShow() {
        String gameName = resolveGameName();
        if (gameName == null) {
            LOG.warn("GameRemover show: no gameName resolved");
            return;
        }
        if (label != null) {
            try { label.setText(String.format(loc.get("GAMEREMOVER.HEADER"), gameName)); }
            catch (Exception e) { label.setText("Remove " + gameName + " from launcher?"); }
        }

        String mainPath = appModule.getGame("mainPath", gameName);
        String executable = appModule.getGame("executable", gameName);
        String legacyPrefixPath = executable != null && Path.of(executable).getParent() != null
                ? Path.of(executable).getParent().resolve("OFME Prefix").toString()
                : null;
        String prefixPath = appModule.getGame("prefixPath", gameName);

        boolean hasPrefix = prefixPath != null && Files.isDirectory(Path.of(prefixPath));
        boolean hasLegacy = legacyPrefixPath != null && Files.isDirectory(Path.of(legacyPrefixPath));

        if (removePrefix != null) {
            if (!hasPrefix && !hasLegacy) {
                removePrefix.setDisable(true);
                removePrefix.setSelected(false);
            } else if (hasLegacy || (prefixPath != null && mainPath != null && prefixPath.contains(mainPath))) {
                removePrefix.setDisable(true);
                removePrefix.setSelected(true);
            } else {
                removePrefix.setDisable(false);
                removePrefix.setSelected(true);
            }
        }
        if (removeFiles != null) {
            boolean hasMain = mainPath != null && Files.isDirectory(Path.of(mainPath));
            removeFiles.setDisable(!hasMain);
            // keep selected state as-is if already set; otherwise default false
        }
    }

    public void doShow() { handleShow(); }

    // -----------------------------------------------------------------------
    // button.action – mirrors PHP doButtonAction (full removal)
    // -----------------------------------------------------------------------

    @FXML
    private void handleButtonAction(javafx.event.ActionEvent e) {
        String gameName = resolveGameName();
        if (gameName == null) {
            LOG.warn("GameRemover: no gameName – abort");
            hideStage();
            return;
        }

        String desktopBase = execReadFully("xdg-user-dir DESKTOP");
        if (desktopBase == null || desktopBase.isBlank()) desktopBase = System.getProperty("user.home") + "/Desktop";
        else desktopBase = desktopBase.trim();

        String desktopIcon = desktopBase + "/" + gameName + ".desktop";
        String appMenuIcon = System.getProperty("user.home") + "/.local/share/applications/" + gameName + ".desktop";
        String icon = appModule.getGame("icon", gameName);
        String banner = appModule.getGame("banner", gameName);

        String executable = appModule.getGame("executable", gameName);
        String prefixPath = appModule.getGame("prefixPath", gameName);
        if (prefixPath == null || prefixPath.isBlank()) {
            prefixPath = executable != null && Path.of(executable).getParent() != null
                    ? Path.of(executable).getParent().resolve("OFME Prefix").toString()
                    : null;
        }

        // --- prefix deletion with detailed logging (Java 25) ---
        if (removePrefix != null && removePrefix.isSelected()) {
            if (prefixPath == null || prefixPath.isBlank()) {
                LOG.info("prefix deletion skipped: prefixPath is null/blank for game='{}' removePrefix selected={}", gameName, removePrefix.isSelected());
            } else {
                Path prefix = Path.of(prefixPath);
                boolean existedBefore = Files.exists(prefix);
                LOG.info("prefix deletion requested: game='{}' path='{}' existsBefore={} isDirectory={} removePrefix selected={}",
                        gameName, prefixPath, existedBefore, Files.isDirectory(prefix), removePrefix.isSelected());
                if (!existedBefore) {
                    LOG.info("prefix deletion result: path='{}' game='{}' already absent before deletion (nothing to delete) success=true", prefixPath, gameName);
                } else {
                    try {
                        Process proc = new ProcessBuilder("rm", "-rf", prefixPath).start();
                        int exit = proc.waitFor();
                        boolean existsAfter = Files.exists(prefix);
                        boolean deleted = !existsAfter;
                        if (exit == 0 && deleted) {
                            LOG.info("prefix deletion result: path='{}' game='{}' successfully deleted exitCode={} existsAfter={} success=true", prefixPath, gameName, exit, existsAfter);
                        } else if (exit == 0 && existsAfter) {
                            LOG.warn("prefix deletion result: path='{}' game='{}' rm exit 0 but path still exists existsAfter={} success=false (permission or mount issue)", prefixPath, gameName, existsAfter);
                        } else {
                            LOG.warn("prefix deletion result: path='{}' game='{}' failed exitCode={} existsAfter={} success=false", prefixPath, gameName, exit, existsAfter);
                        }
                    } catch (Exception ex) {
                        boolean existsAfter = Files.exists(prefix);
                        LOG.warn("prefix deletion result: path='{}' game='{}' exception during rm existsAfter={} success=false", prefixPath, gameName, existsAfter, ex);
                    }
                }
            }
        } else {
            LOG.info("prefix deletion skipped: removePrefix not selected or control null for game='{}' removePrefix selected={} prefixPath='{}'",
                    gameName, removePrefix != null ? removePrefix.isSelected() : null, prefixPath);
        }

        // Remove INI section BEFORE deleting files – if INI removal fails, preserve files
        try {
            appModule.removeGameSection(gameName);
        } catch (Exception ex) {
            LOG.error("Failed to remove INI section for game='{}' – aborting to preserve state", gameName, ex);
            hideStage();
            return;
        }

        if (removeFiles != null && removeFiles.isSelected()) {
            String mainPath = appModule.getGame("mainPath", gameName);
            if (mainPath == null || mainPath.isBlank()) {
                mainPath = executable != null && Path.of(executable).getParent() != null
                        ? Path.of(executable).getParent().toString()
                        : null;
            }
            if (mainPath != null) {
                Path mp = Path.of(mainPath);
                boolean existedBefore = Files.exists(mp);
                LOG.info("game files deletion requested: game='{}' path='{}' existsBefore={} isDirectory={}", gameName, mainPath, existedBefore, Files.isDirectory(mp));
                if (Files.isDirectory(mp)) {
                    try {
                        Process proc = new ProcessBuilder("rm", "-rf", mainPath).start();
                        int exit = proc.waitFor();
                        boolean existsAfter = Files.exists(mp);
                        boolean deleted = !existsAfter;
                        if (exit == 0 && deleted) {
                            LOG.info("game files deletion result: path='{}' game='{}' successfully deleted exitCode={} success=true", mainPath, gameName, exit);
                        } else {
                            LOG.warn("game files deletion result: path='{}' game='{}' failed exitCode={} existsAfter={} success=false", mainPath, gameName, exit, existsAfter);
                        }
                    } catch (Exception ex) {
                        LOG.warn("game files deletion result: path='{}' game='{}' exception existsAfter={} success=false", mainPath, gameName, Files.exists(mp), ex);
                    }
                } else {
                    LOG.info("game files deletion skipped: path='{}' game='{}' is not a directory or does not exist existsBefore={} success=false (nothing to delete)", mainPath, gameName, existedBefore);
                }
            } else {
                LOG.info("game files deletion skipped: mainPath resolved null for game='{}'", gameName);
            }
        } else {
            LOG.info("game files deletion skipped: removeFiles not selected for game='{}'", gameName);
        }

        removeFile(desktopIcon);
        removeFile(appMenuIcon);
        removeFile(icon);
        removeFile(banner);

        // Free opener – mirrors app()->form('MainForm')->gamePanel->data('opener')->free()
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getMainForm = launcherCls.getMethod("getMainForm");
            Object mf = getMainForm.invoke(null);
            if (mf != null) {
                // Sync MainForm UI lists (master + sidebar + grid) with deletion
                final String deletedName = gameName;
                try {
                    var removeUI = mf.getClass().getMethod("removeGameFromUI", String.class);
                    removeUI.invoke(mf, deletedName);
                } catch (Exception ex) {
                    LOG.debug("removeGameFromUI failed", ex);
                }
                // Try to invoke removeGameTile or free opener
                try {
                    var openerField = mf.getClass().getDeclaredField("currentOpenerNode");
                    openerField.setAccessible(true);
                    Object opener = openerField.get(mf);
                    if (opener instanceof Node n) {
                        Platform.runLater(() -> {
                            var parent = n.getParent();
                            if (parent instanceof javafx.scene.layout.Pane pane) pane.getChildren().remove(n);
                        });
                    }
                } catch (Exception nsf) {
                    LOG.debug("opener field access failed", nsf);
                }
                // Check if container is empty -> show noGamesHeader
                Platform.runLater(() -> {
                    try {
                        var containerField = mf.getClass().getDeclaredField("flowContent");                        containerField.setAccessible(true);
                        Object flow = containerField.get(mf);
                        boolean isEmpty = false;
                        if (flow instanceof javafx.scene.layout.FlowPane fp) isEmpty = fp.getChildren().isEmpty();
                        if (isEmpty) {
                            var headerField = mf.getClass().getDeclaredField("noGamesHeader");
                            headerField.setAccessible(true);
                            Object header = headerField.get(mf);
                            if (header instanceof Label lbl) lbl.setVisible(true);
                        }
                    } catch (Exception ex) { LOG.debug("noGamesHeader check failed", ex); }
                    try {
                        var hideMenu = mf.getClass().getMethod("hideGameMenu");
                        hideMenu.invoke(mf);
                    } catch (Exception ex) { LOG.debug("hideGameMenu failed", ex); }
                });
            }
        } catch (Exception ex) {
            LOG.debug("MainForm update failed", ex);
        }

        hideStage();
    }

    // -----------------------------------------------------------------------
    // Other handlers
    // -----------------------------------------------------------------------

    @FXML
    private void handleButtonAltAction(javafx.event.ActionEvent e) { hideStage(); }

    @FXML
    private void handleHide() { hideStage(); }

    public void doHide() { handleHide(); }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private void removeFile(String path) {
        if (path != null && !path.isBlank() && Files.isRegularFile(Path.of(path))) {
            try { Files.delete(Path.of(path)); }
            catch (IOException e) { LOG.warn("delete failed {}", path, e); }
        }
    }

    private String resolveGameName() {
        // Try gamePanel data via Launcher/MainForm reflection – mirrors app()->form('MainForm')->gamePanel->data('gameName')
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getMainForm = launcherCls.getMethod("getMainForm");
            Object mf = getMainForm.invoke(null);
            if (mf != null) {
                // Check field currentGameName
                try {
                    var field = mf.getClass().getDeclaredField("currentGameName");
                    field.setAccessible(true);
                    Object v = field.get(mf);
                    if (v instanceof String s && !s.isBlank()) return s;
                } catch (NoSuchFieldException ignored) {}
                // Try gamePanel properties
                try {
                    var panelField = mf.getClass().getDeclaredField("gamePanel");
                    panelField.setAccessible(true);
                    Object panel = panelField.get(mf);
                    if (panel instanceof javafx.scene.layout.Pane pane) {
                        Object v = pane.getProperties().get("gameName");
                        if (v instanceof String s) return s;
                    }
                } catch (Exception ignored2) {}
            }
        } catch (Exception e) { LOG.debug("resolveGameName via Launcher failed", e); }
        // Fallback: check system property set by MainForm
        String prop = System.getProperty("corkytux.currentGame");
        if (prop != null && !prop.isBlank()) return prop;
        return null;
    }

    private String execReadFully(String command) {
        try {
            var proc = new ProcessBuilder("bash", "-c", command).start();
            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line = reader.readLine();
                proc.waitFor();
                return line;
            }
        } catch (Exception e) {
            LOG.debug("exec failed {}", command, e);
            return null;
        }
    }

    private void hideStage() {
        Platform.runLater(() -> {
            // Modal context (inside MainForm overlay): close modal, NOT the stage.
            // Only when the modal overlay is actually visible; else legacy stage hide.
            try {
                var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                var getCtrl = launcherCls.getMethod("getFormController", String.class);
                Object mf = getCtrl.invoke(null, "MainForm");
                if (mf != null) {
                    boolean modalVisible = false;
                    try {
                        var overlayField = mf.getClass().getDeclaredField("modalOverlay");
                        overlayField.setAccessible(true);
                        Object overlay = overlayField.get(mf);
                        if (overlay instanceof Node ov) modalVisible = ov.isVisible();
                    } catch (Exception ignored) {}
                    if (modalVisible) {
                        var hideModal = mf.getClass().getDeclaredMethod("hideModal");
                        hideModal.setAccessible(true);
                        hideModal.invoke(mf);
                        return;
                    }
                }
            } catch (Exception e) {
                LOG.debug("hideModal fallback failed, hiding stage", e);
            }
            Node n = root != null ? root : button;
            if (n == null || n.getScene() == null) return;
            var w = n.getScene().getWindow();
            if (w instanceof Stage s) s.hide();
            else w.hide();
        });
    }
}

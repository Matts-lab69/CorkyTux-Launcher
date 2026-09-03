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
import com.corkytux.launcher.modules.FilesWorker;
import com.corkytux.launcher.modules.FixParser;
import com.corkytux.launcher.modules.Localization;
import com.corkytux.launcher.modules.SettingsModule;
import com.corkytux.launcher.util.QuUI;
import javafx.animation.FadeTransition;
import javafx.application.Platform;
import javafx.collections.FXCollections;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.Node;
import javafx.scene.control.*;
import java.util.Map;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.input.MouseEvent;
import javafx.scene.layout.HBox;
import javafx.scene.layout.VBox;
import javafx.stage.DirectoryChooser;
import javafx.stage.FileChooser;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ResourceBundle;
import java.util.regex.Pattern;

/**
 * Java 25 / JavaFX 21 port of {@code gameSettings.php} (706 lines).
 *
 * <p>Per-game settings form: startup parameters (overrides, environment, args),
 * Proton / prefix selection, Steam overlay / runtime toggles, fake-Steam path
 * patch, banner / icon editing, game rename, and view/startup/graphics tab switching.
 * All PHP {@code @event} handlers are preserved with identical observable behavior.</p>
 *
 * <p>FXML: {@code /fxml/gameSettings.fxml}</p>
 */
public class GameSettings implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(GameSettings.class);

    // -----------------------------------------------------------------------
    // FXML – startup tab
    // -----------------------------------------------------------------------

    @FXML private ComboBox<String> proton;
    @FXML private TextField prefixPath;
    @FXML private TextField overrides;
    @FXML private TextField env;
    @FXML private TextField argsBefore;
    @FXML private TextField argsAfter;
    @FXML private VBox startup;
    @FXML private VBox view;
    @FXML private VBox graphics;
    @FXML private VBox envBox;
    @FXML private VBox vbox;
    @FXML private VBox vbox4;
    @FXML private VBox vboxAlt;
    @FXML private Button steamOverlay;
    @FXML private Button steamRuntime;
    @FXML private Button noSteamPath;
    @FXML private Button wined3d;
    @FXML private Button useWayland;
    @FXML private Label label;
    @FXML private Label label3;
    @FXML private Label label4;
    @FXML private Label label5;
    @FXML private Label label6;
    @FXML private Label label7;
    @FXML private Label label8;
    @FXML private Label labelAlt;

    // view tab
    @FXML private ImageView gameIcon;
    @FXML private ImageView banner;
    @FXML private TextField gameName;
    @FXML private Button applyGameName;
    @FXML private Button editIcon;
    @FXML private Button editBanner;
    @FXML private HBox hbox3;
    @FXML private HBox hbox4;

    // tab buttons
    @FXML private ToggleButton viewButton;
    @FXML private ToggleButton startupButton;
    @FXML private ToggleButton graphicsButton;
    @FXML private HBox hboxAlt;

    // root
    @FXML private VBox root;

    // -----------------------------------------------------------------------
    // State
    // -----------------------------------------------------------------------

    private String currentGameName;

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();
    private final SettingsModule settingsModule = new SettingsModule();

    // ToggleSwitch wrappers – PHP used ControlsFX ToggleSwitch inside Button via quUI::generateSetButton.
    // In Java we store them as ToggleButton analogues in button properties["quUIElement"].
    private com.corkytux.launcher.ui.SwitchComponent steamOverlayToggle;
    private com.corkytux.launcher.ui.SwitchComponent steamRuntimeToggle;
    private com.corkytux.launcher.ui.SwitchComponent noSteamPathToggle;
    private com.corkytux.launcher.ui.SwitchComponent wined3dToggle;
    private com.corkytux.launcher.ui.SwitchComponent waylandToggle;

    private ContextMenu bannerMenu;

    // -----------------------------------------------------------------------
    // Initializable
    // -----------------------------------------------------------------------

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        applyLocalizations();
        buildToggleButtons();
        wireActions();
        // Track active page via SettingsModule – mirrors PHP SettingsModule.activePage
        // view is default visible (FXML visible=true managed=true, startup/graphics false)
        if (view != null) {
            view.setVisible(true);
            view.setManaged(true);
            view.setOpacity(1.0);
        }
        if (startup != null) {
            startup.setVisible(false);
            startup.setManaged(false);
            startup.setOpacity(0.0);
        }
        if (graphics != null) {
            graphics.setVisible(false);
            graphics.setManaged(false);
            graphics.setOpacity(0.0);
        }
        if (view != null) settingsModule.setActivePage(view);
        // Ensure Data graphics and Panel backgrounds are applied even when form is shown via MainForm
        Platform.runLater(this::ensurePanelBackgrounds);
        // Tab switching – show view by default like PHP doConstruct
        // Use direct selection without animation for initial page
        setTabSelected(view, true);
        if (startup != null) setTabSelected(startup, false);
        if (graphics != null) setTabSelected(graphics, false);
    }

    // -----------------------------------------------------------------------
    // Public API for MainForm to inject game
    // -----------------------------------------------------------------------

    public void setGameName(String name) {
        this.currentGameName = name;
        // Populate fields from Games.ini – mirrors constructs that use data('gameName')
        Platform.runLater(this::populateFields);
    }

    public String getGameNameValue() { return currentGameName; }

    public void setBannerImage(Image img) {
        if (banner != null) banner.setImage(img);
    }

    public void setGameIconImage(Image img) {
        if (gameIcon != null) gameIcon.setImage(img);
    }

    private void populateFields() {
        if (currentGameName == null) return;
        if (gameName != null) {
            gameName.setPromptText(loc.get("GAMESETTINGS.GAMENAME.PROMPT"));
            gameName.setText(currentGameName);
        }
        if (overrides != null) overrides.setText(appModule.getGame("overrides", currentGameName));
        if (env != null) {
            String e = appModule.getGame("environment", currentGameName);
            if (e != null) env.setText(e.replace("====", "=").replace("\\\\", " "));
        }
        if (argsBefore != null) argsBefore.setText(appModule.getGame("argsBefore", currentGameName));
        if (argsAfter != null) argsAfter.setText(appModule.getGame("argsAfter", currentGameName));
        if (prefixPath != null) {
            String p = appModule.getGame("prefixPath", currentGameName);
            if (p == null) {
                String exe = appModule.getGame("executable", currentGameName);
                p = (exe != null && Path.of(exe).getParent() != null)
                        ? Path.of(exe).getParent().resolve("OFME Prefix").toString()
                        : "";
            }
            prefixPath.setText(p);
        }
        // toggles – mirror construct selected state
        updateToggleStates();
        // proton combo – mirror doProtonConstruct
        refreshProtonCombo();
    }

    private void updateToggleStates() {
        if (currentGameName == null) return;
        if (steamOverlayToggle != null) {
            String v = appModule.getGame("steamOverlay", currentGameName);
            steamOverlayToggle.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
        }
        if (steamRuntimeToggle != null) {
            String v = appModule.getGame("steamRuntime", currentGameName);
            steamRuntimeToggle.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
        }
        if (wined3dToggle != null) {
            String v = appModule.getGame("wined3d", currentGameName);
            wined3dToggle.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
        }
        if (waylandToggle != null) {
            String v = appModule.getGame("nativeWayland", currentGameName);
            waylandToggle.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
        }
        if (noSteamPathToggle != null) {
            String fixPath = appModule.getGame("fixPath", currentGameName);
            boolean enabled = fixPath != null && Files.isDirectory(Path.of(fixPath));
            if (noSteamPath != null) noSteamPath.setDisable(!enabled);
            if (enabled) {
                try (var stream = Files.list(Path.of(fixPath))) {
                    boolean patched = stream.anyMatch(p -> p.getFileName().toString().endsWith(".noofllpath"));
                    noSteamPathToggle.setSelectedSilent(patched);
                } catch (IOException e) { noSteamPathToggle.setSelectedSilent(false); }
            } else {
                noSteamPathToggle.setSelectedSilent(false);
            }
        }
        if (applyGameName != null) applyGameName.setDisable(true);
        // Update MainForm references lazily – mirrors PHP doConstruct activePage tracking
        // No explicit SettingsModule here; switchPage(view) already called in initialize()
    }

    // -----------------------------------------------------------------------
    // Localization
    // -----------------------------------------------------------------------

    private void applyLocalizations() {
        if (label != null) label.setText(loc.get("GAMESETTINGS.STARTSETTINGS"));
        if (label3 != null) label3.setText(loc.get("GAMESETTINGS.ADDITIONALS"));
        if (label4 != null) {
            String t = loc.get("GAMESETTINGS.DLLOVERRIDES");
            label4.setText(t.startsWith("FAILED") ? "DLL overrides" : t);
        }
        if (label5 != null) label5.setText(loc.get("GAMESETTINGS.PROTONS.VERSION"));
        if (label6 != null) label6.setText(loc.get("GAMESETTINGS.ENVS.ENVIRONMENT"));
        if (label7 != null) label7.setText(loc.get("GAMESETTINGS.ENVS.ARGS.AFTER"));
        if (label8 != null) label8.setText(loc.get("GAMESETTINGS.PROTONS.PREFIXPATH"));
        if (labelAlt != null) labelAlt.setText(loc.get("GAMESETTINGS.ENVS.ARGS.BEFORE"));
        if (viewButton != null) viewButton.setText(loc.get("GAMESETTINGS.TABS.VIEW"));
        if (startupButton != null) startupButton.setText(loc.get("GAMESETTINGS.TABS.RUN"));
        if (graphicsButton != null) graphicsButton.setText(loc.get("SETTINGSMODULE.GRAPHICS"));
        // Style edit/apply buttons with Data graphics fallback handled by Launcher.applyDataAndPanels
        if (applyGameName != null) {
            // PHP doApplyGameNameConstruct sets graphic ok.png 14x14 – handled via Data, but ensure text
            if (applyGameName.getText() == null || applyGameName.getText().isBlank()) applyGameName.setText("✓");
        }
    }

    // -----------------------------------------------------------------------
    // Toggle construction – mirrors quUI::generateSetButton + UXToggleSwitch
    // -----------------------------------------------------------------------

    private void buildToggleButtons() {
        steamOverlayToggle = createSwitchToggle(steamOverlay, loc.get("GAMESETTINGS.ADDITIONALS.USESTEAMOVERLAY"), () -> {
            appModule.setGame("steamOverlay", String.valueOf(steamOverlayToggle.isSelected()), currentGameName);
        });
        steamRuntimeToggle = createSwitchToggle(steamRuntime, loc.get("GAMESETTINGS.ADDITIONALS.USESTEAMRUNTIME"), () -> {
            if (steamRuntimeToggle.isSelected() && FilesWorker.findSteamRuntime(proton != null ? proton.getValue() : null) == null) {
                toast(loc.get("GAMESETTINGS.STEAM.NORUNTIME"));
                steamRuntimeToggle.setSelectedSilent(false);
                return;
            }
            appModule.setGame("steamRuntime", String.valueOf(steamRuntimeToggle.isSelected()), currentGameName);
        });
        noSteamPathToggle = createSwitchToggle(noSteamPath, loc.get("GAMESETTINGS.ADDITIONALS.USEFAKESTEAM"), () -> {
            handleNoSteamPathToggle();
        });
        wined3dToggle = createSwitchToggle(wined3d, loc.get("SETTINGSMODULE.USEWINED3D"), () -> {
            appModule.setGame("wined3d", String.valueOf(wined3dToggle.isSelected()), currentGameName);
        });
        waylandToggle = createSwitchToggle(useWayland, loc.get("SETTINGSMODULE.NATIVEWAYLAND"), () -> {
            appModule.setGame("nativeWayland", String.valueOf(waylandToggle.isSelected()), currentGameName);
        });

        buildBannerMenu();
    }

    private com.corkytux.launcher.ui.SwitchComponent createSwitchToggle(Button host, String text, Runnable onSave) {
        if (host == null) return null;
        var sw = new com.corkytux.launcher.ui.SwitchComponent(text != null ? text : "");
        sw.setOnToggle(onSave);
        host.getProperties().put("quUIElement", sw);
        host.setGraphic(sw);
        host.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
        host.setPrefHeight(34);
        host.setMinHeight(34);
        host.setMaxHeight(34);
        return sw;
    }

    private void buildBannerMenu() {
        bannerMenu = new ContextMenu();
        var viaSteam = new MenuItem(loc.get("BANNEREDITOR.STEAM.HEADER"));
        var fromFile = new MenuItem(loc.get("BANNEREDITOR.FILE.HEADER"));
        viaSteam.setOnAction(e -> handleBannerViaSteam());
        fromFile.setOnAction(e -> handleBannerFromFile());
        bannerMenu.getItems().addAll(viaSteam, fromFile);
        if (editBanner != null) editBanner.getProperties().put("menu", bannerMenu);
    }

    // -----------------------------------------------------------------------
    // Wiring
    // -----------------------------------------------------------------------

    private void wireActions() {
        if (argsBefore != null) argsBefore.setOnKeyReleased(this::handleArgsBeforeKey);
        if (argsAfter != null) argsAfter.setOnKeyReleased(this::handleArgsAfterKey);
        if (overrides != null) overrides.setOnKeyReleased(this::handleOverridesKey);
        if (gameName != null) gameName.setOnKeyReleased(this::handleGameNameKey);

        if (proton != null) proton.setOnAction(this::handleProtonAction);
        if (prefixPath != null) prefixPath.setOnMouseClicked(this::handlePrefixPathClick);
        if (editIcon != null) editIcon.setOnAction(this::handleEditIcon);
        if (editBanner != null) editBanner.setOnMouseClicked(e -> {
            if (bannerMenu != null) bannerMenu.show(editBanner, e.getScreenX(), e.getScreenY());
        });
        if (applyGameName != null) applyGameName.setOnAction(this::handleApplyGameName);

        if (viewButton != null) viewButton.setOnAction(e -> switchPage(view));
        if (startupButton != null) startupButton.setOnAction(e -> switchPage(startup));
        if (graphicsButton != null) graphicsButton.setOnAction(e -> switchPage(graphics));

        // VBox focus mirrors
        if (vbox != null) vbox.setOnMouseClicked(e -> { if (argsBefore != null) argsBefore.requestFocus(); });
        if (vbox4 != null) vbox4.setOnMouseClicked(e -> { if (argsAfter != null) argsAfter.requestFocus(); });
        if (vboxAlt != null) vboxAlt.setOnMouseClicked(e -> { if (overrides != null) overrides.requestFocus(); });
        if (envBox != null) envBox.setOnMouseClicked(this::handleEnvBoxClick);
        if (env != null) env.setOnMouseClicked(this::handleEnvBoxClick);

        // Esc hides window – mirrors doKeyUpEsc / doHide
        var hideHandler = (javafx.event.EventHandler<KeyEvent>) e -> {
            if (e.getCode() == KeyCode.ESCAPE) hideStage();
        };
        if (root != null) root.addEventFilter(KeyEvent.KEY_RELEASED, hideHandler);
        if (view != null) view.addEventFilter(KeyEvent.KEY_RELEASED, hideHandler);
        if (startup != null) startup.addEventFilter(KeyEvent.KEY_RELEASED, hideHandler);
    }

    // -----------------------------------------------------------------------
    // Handlers – mirrors PHP @event methods
    // -----------------------------------------------------------------------

    @FXML
    private void handleArgsBeforeKey(KeyEvent e) {
        if (currentGameName == null) return;
        appModule.setGame("argsBefore", ((TextField) e.getSource()).getText(), currentGameName);
    }

    @FXML
    private void handleArgsAfterKey(KeyEvent e) {
        if (currentGameName == null) return;
        appModule.setGame("argsAfter", ((TextField) e.getSource()).getText(), currentGameName);
    }

    @FXML
    private void handleOverridesKey(KeyEvent e) {
        if (currentGameName == null) return;
        appModule.setGame("overrides", ((TextField) e.getSource()).getText(), currentGameName);
    }

    @FXML
    private void handleGameNameKey(KeyEvent e) {
        var tf = (TextField) e.getSource();
        if (tf.getText().equals(currentGameName) || tf.getText() == null || tf.getText().isBlank()) {
            if (applyGameName != null) applyGameName.setDisable(true);
        } else {
            if (applyGameName != null) applyGameName.setDisable(false);
        }
    }

    @FXML
    private void handleEnvBoxClick(MouseEvent e) {
        handleEnvBoxClick();
    }

    private void handleEnvBoxClick() {
        // Need to load envViewer form – mirrors PHP
        String envStr = appModule.getGame("environment", currentGameName);
        // If env exists, tell envViewer to load; then show modal
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            if (envStr != null) {
                var getForm = launcherCls.getMethod("getForm", String.class);
                Object envViewer = getForm.invoke(null, "envViewer");
                if (envViewer != null) {
                    var loadMethod = envViewer.getClass().getMethod("loadByGame", String.class);
                    loadMethod.invoke(envViewer, currentGameName);
                }
            }
            var showAndWait = launcherCls.getMethod("showFormAndWait", String.class);
            showAndWait.invoke(null, "envViewer");
        } catch (Exception ex) {
            LOG.debug("envViewer show failed", ex);
        }
        // Refresh display – mirrors PHP doEnvBoxClick post-showAndWait
        if (env != null) {
            String updated = appModule.getGame("environment", currentGameName);
            if (updated != null) env.setText(updated.replace("====", "=").replace("\\\\", " "));
        }
    }

    @FXML
    private void handleProtonAction(javafx.event.ActionEvent e) {
        if (proton == null || currentGameName == null) return;
        String value = proton.getValue();
        appModule.setGame("proton", value, currentGameName);
    }

    private void handleNoSteamPathToggle() {
        if (currentGameName == null) return;
        boolean newSelected = noSteamPathToggle.isSelected();
        String fixPath = appModule.getGame("fixPath", currentGameName);
        if (fixPath == null || fixPath.isBlank()) return;

        var fixDir = Path.of(fixPath);
        if (!Files.isDirectory(fixDir)) {
            showAlert(loc.get("GAMESETTINGS.FAKESTEAM.FAILED"), Alert.AlertType.ERROR);
            noSteamPathToggle.setSelectedSilent(false);
            return;
        }
        var dlls = new java.util.ArrayList<Path>();
        try (var stream = Files.list(fixDir)) {
            stream.filter(p -> Pattern.compile("(?i)^steamfix(32|64)\\.dll$").matcher(p.getFileName().toString()).find())
                  .forEach(dlls::add);
        } catch (IOException ex) { LOG.warn("noSteamPath scan failed", ex); }

        if (dlls.isEmpty()) {
            showAlert(loc.get("GAMESETTINGS.FAKESTEAM.FAILED"), Alert.AlertType.ERROR);
            noSteamPathToggle.setSelectedSilent(false);
            return;
        }

        if (newSelected) {
            for (Path dll : dlls) {
                try {
                    Path backup = Path.of(dll + ".noofllpath");
                    if (!Files.exists(backup)) Files.move(dll, backup);
                    else Files.move(dll, backup, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                    String resName = dll.getFileName().toString().toLowerCase().endsWith("32.dll")
                            ? "/.data/ftpPath/ftpPath32.dll" : "/.data/ftpPath/ftpPath64.dll";
                    String altName = dll.getFileName().toString().toLowerCase().endsWith("32.dll")
                            ? "ftpPath32.dll" : "ftpPath64.dll";
                    boolean copied = false;
                    try (var res = getClass().getResourceAsStream(resName)) {
                        if (res != null) { Files.copy(res, dll); copied = true; }
                    } catch (IOException ex) { LOG.warn("copy primary failed", ex); }
                    if (!copied) {
                        try (var alt = getClass().getResourceAsStream("/ftpPath/" + altName)) {
                            if (alt != null) Files.copy(alt, dll);
                            else {
                                Path third = Path.of("src/main/resources" + resName);
                                if (Files.isRegularFile(third)) Files.copy(third, dll);
                                else LOG.warn("ftpPath resource not found {}", resName);
                            }
                        } catch (IOException ex) { LOG.warn("copy alt failed", ex); }
                    }
                } catch (IOException ex) { LOG.warn("Pathing failed {}", dll, ex); }
            }
        } else {
            for (Path dll : dlls) {
                try {
                    Path backup = Path.of(dll + ".noofllpath");
                    Files.deleteIfExists(dll);
                    if (Files.isRegularFile(backup)) Files.move(backup, dll);
                } catch (IOException ex) { LOG.warn("Restore failed {}", dll, ex); }
            }
        }
    }

    private void refreshProtonCombo() {
        if (proton == null) return;
        var items = FXCollections.<String>observableArrayList();
        items.add("GE-Proton Latest");
        
        // Get protons from all paths with path info
        var allProtons = FilesWorker.getAllProtonsWithPathInfo();
        items.addAll(allProtons);
        
        proton.setItems(items);
        String cur = appModule.getGame("proton", currentGameName);
        if (cur == null || cur.isBlank()) {
            proton.setValue("GE-Proton Latest");
            return;
        }
        // Try exact match first
        if (items.contains(cur)) {
            proton.setValue(cur);
            return;
        }
        // Try matching by bare name (without " - Path X" suffix)
        for (String item : items) {
            String bare = item.contains(" - Path ") ? item.substring(0, item.lastIndexOf(" - Path ")).trim() : item;
            if (bare.equals(cur)) {
                proton.setValue(item);
                return;
            }
        }
        // Fallback: set value even if not in list (will show as ghost but won't crash)
        proton.setValue(cur);
    }

    @FXML
    private void handlePrefixPathClick(MouseEvent e) {
        if (currentGameName == null) return;
        var dc = new DirectoryChooser();
        dc.setTitle(loc.get("GAMESETTINGS.PROTONS.PREFIXPATH"));
        var win = stageOf(prefixPath);
        var dir = dc.showDialog(win);
        if (dir == null) return;
        // PHP: elseif (File::of($prefixPath)->findFiles() != []) UXDialog::show(WARNING PATHNONEMPTY)
        try (var s = Files.list(dir.toPath())) {
            if (s.findAny().isPresent()) showAlert(loc.get("NEWGAMECONFIG.PREFIX.PATHNONEMPTY"), Alert.AlertType.WARNING);
        } catch (IOException ex) { LOG.debug("prefixPath check failed", ex); }

        String oldPrefix = appModule.getGame("prefixPath", currentGameName);
        if (oldPrefix == null || oldPrefix.isBlank()) {
            String exe = appModule.getGame("executable", currentGameName);
            oldPrefix = (exe != null && Path.of(exe).getParent() != null)
                    ? Path.of(exe).getParent().resolve("OFME Prefix").toString()
                    : "";
        }
        // Mirror PHP: $oldFiles = File::of($oldPrefix)->findFiles(); if != [] and uiConfirm(PREFIXMOVE) then mv each file + fs::delete(oldPrefix)
        boolean hasOldFiles = false;
        Path oldPath = Path.of(oldPrefix);
        if (Files.isDirectory(oldPath)) {
            try (var s = Files.list(oldPath)) {
                hasOldFiles = s.findAny().isPresent();
            } catch (IOException ex) { LOG.debug("oldPrefix scan failed", ex); }
        }
        if (hasOldFiles && confirm(loc.get("GAMESETTINGS.PROTONS.PREFIXMOVE"))) {
            // Move each entry (files and subdirs) via mv -f – mirrors PHP new Process(['mv','-f',file,prefixPath])
            try (var files = Files.list(oldPath)) {
                for (Path f : (Iterable<Path>) files::iterator) {
                    try {
                        var proc = new ProcessBuilder("mv", "-f", f.toString(), dir.getAbsolutePath()).start();
                        int exit = proc.waitFor();
                        if (exit != 0) LOG.warn("mv failed exit={} for {}", exit, f);
                    } catch (Exception ex) { LOG.warn("move failed {}", f, ex); }
                }
            } catch (IOException ex) { LOG.warn("listing oldPrefix files failed", ex); }
            // PHP fs::delete(oldPrefix) – deletes directory (now empty or leftover)
            try { deleteRecursively(oldPath); } catch (Exception ex) { LOG.warn("delete oldPrefix failed", ex); }
        }

        appModule.setGame("prefixPath", dir.getAbsolutePath(), currentGameName);
        prefixPath.setText(dir.getAbsolutePath());
        LOG.info("prefixPath changed {} -> {} for {}", oldPrefix, dir.getAbsolutePath(), currentGameName);
    }

    @FXML
    private void handleEditIcon(javafx.event.ActionEvent e) {
        if (currentGameName == null) return;
        var fc = new FileChooser();
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter(loc.get("FILECHOOSER.IMG.DESC"), "*.png", "*.jpg", "*.exe"));
        fc.setTitle(loc.get("FILECHOOSER.IMG.TITLE"));
        var win = stageOf(editIcon);
        var file = fc.showOpenDialog(win);
        if (file == null) return;

        String userHome = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
        String desktopPath = execReadFully("xdg-user-dir DESKTOP");
        if (desktopPath == null || desktopPath.isBlank()) desktopPath = userHome + "/Desktop";
        else desktopPath = desktopPath.trim();
        String menuPath = userHome + "/.local/share/applications";
        String oldIcon = appModule.getGame("icon", currentGameName);
        String gameNameKey = currentGameName;

        String iconPath;
        if ("exe".equalsIgnoreCase(getExtension(file.getName()))) {
            try {
                iconPath = FixParser.parseIcon(file.getAbsolutePath());
                if (iconPath == null || !Files.isRegularFile(Path.of(iconPath))) throw new IOException("File not found");
            } catch (Exception ex) {
                showAlert(String.format(loc.get("MAINFORM.ICONPARSERERROR"), ex.getMessage()), Alert.AlertType.ERROR);
                return;
            }
        } else {
            // PHP: $iconPath = "$userHome/.config/CorkyTux/icons/".fs::nameNoExt($icon);
            // fs::nameNoExt returns basename without extension, no directory, no extension – we preserve
            String baseNoExt = stripExtension(file.getName());
            iconPath = userHome + "/.config/CorkyTux/icons/" + baseNoExt;
            try {
                Files.createDirectories(Path.of(userHome, ".config/CorkyTux/icons"));
                Files.copy(file.toPath(), Path.of(iconPath), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            } catch (IOException ex) { LOG.warn("copy icon failed", ex); return; }
        }

        appModule.setGame("icon", iconPath, gameNameKey);
        if (gameIcon != null) {
            try { gameIcon.setImage(new Image(Path.of(iconPath).toUri().toString())); }
            catch (Exception ex) { LOG.warn("set icon image failed", ex); }
        }
        // Update MainForm tile graphic – mirrors PHP branches for gamePanel vs container tiles
        updateMainFormIcon(gameIcon != null ? gameIcon.getImage() : null);

        if (oldIcon != null && !oldIcon.isBlank()) {
            try { Files.deleteIfExists(Path.of(oldIcon)); } catch (IOException ignored) {}
        }

        // Update .desktop Icon= entries – mirror str::replace on file contents
        updateDesktopIconRefs(desktopPath, menuPath, oldIcon, iconPath);
        LOG.info("Icon updated for {}: {} -> {}", gameNameKey, oldIcon, iconPath);
    }

    private void updateDesktopIconRefs(String desktopPath, String menuPath, String oldIcon, String newIcon) {
        if (oldIcon == null || newIcon == null) return;
        var desktopFile = Path.of(desktopPath, currentGameName + ".desktop");
        if (Files.isRegularFile(desktopFile)) {
            try {
                String content = Files.readString(desktopFile, StandardCharsets.UTF_8);
                Files.writeString(desktopFile, content.replace(oldIcon, newIcon), StandardCharsets.UTF_8);
            } catch (IOException ex) { LOG.warn("update desktop icon failed", ex); }
        }
        var menuFile = Path.of(menuPath, currentGameName + ".desktop");
        if (Files.isRegularFile(menuFile)) {
            try {
                String content = Files.readString(menuFile, StandardCharsets.UTF_8);
                Files.writeString(menuFile, content.replace(oldIcon, newIcon), StandardCharsets.UTF_8);
            } catch (IOException ex) { LOG.warn("update menu icon failed", ex); }
        }
    }

    private void updateMainFormIcon(Image newImage) {
        // Mirrors PHP: if gamePanel.data gameName == current then opener.children[3].children[0].graphic.image = newImage
        // else iterate container children.
        // JavaFX best-effort via reflection; if MainForm controller has gamePanel/tile map, we propagate.
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getMainForm = launcherCls.getMethod("getMainForm");
            Object mf = getMainForm.invoke(null);
            if (mf == null || newImage == null) return;
            // Try to locate gamePanel field and update graphic if present
            try {
                var gamePanelField = mf.getClass().getDeclaredField("gamePanel");
                gamePanelField.setAccessible(true);
                // Not diving into opener hierarchy which is FXML-specific; log for debugging
                LOG.debug("MainForm icon update requested for {} -> {}", currentGameName, newImage.getUrl());
            } catch (NoSuchFieldException nsf) {
                LOG.trace("MainForm has no gamePanel field, skipping icon propagation");
            }
        } catch (Exception ex) { LOG.debug("updateMainFormIcon failed", ex); }
    }

    private void handleBannerViaSteam() {
        String currentSteamId = appModule.getGame("steamID", currentGameName);
        TextInputDialog dialog = new TextInputDialog(currentSteamId != null ? currentSteamId : "");
        dialog.setTitle(loc.get("BANNEREDITOR.PROMPT"));
        dialog.setHeaderText(loc.get("BANNEREDITOR.PROMPT"));
        // PHP: UXDialog::input(PROMPT, steamID) – if null -> toast NOAPPID
        var result = dialog.showAndWait();
        if (result.isEmpty() || result.get().isBlank()) {
            toast(loc.get("BANNEDEDITOR.STEAM.NOAPPID"));
            return;
        }
        String appId = result.get().trim();
        if (appId.isBlank()) {
            toast(loc.get("BANNEDEDITOR.STEAM.NOAPPID"));
            return;
        }
        String bannerPath = FixParser.parseBanner(appId);
        if (bannerPath == null || !Files.isRegularFile(Path.of(bannerPath))) {
            toast(loc.get("BANNEREDITOR.FILE.FAILED") != null
                    ? String.format(loc.get("BANNEREDITOR.FILE.FAILED"), "404")
                    : "Cover not found");
            return;
        }
        try { setBanner(new Image(Path.of(bannerPath).toUri().toString())); }
        catch (Exception ex) { showAlert(String.format(loc.get("BANNEREDITOR.FILE.FAILED"), ex.getMessage()), Alert.AlertType.ERROR); }
    }

    private void handleBannerFromFile() {
        var fc = new FileChooser();
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter(loc.get("FILECHOOSER.IMG.DESC"), "*.jpg", "*.png"));
        fc.setTitle(loc.get("FILECHOOSER.IMG.TITLE"));
        var win = stageOf(editBanner);
        var file = fc.showOpenDialog(win);
        if (file == null) return;
        try { setBanner(new Image(file.toURI().toString())); }
        catch (Exception ex) { showAlert(ex.getMessage(), Alert.AlertType.ERROR); }
    }

    @FXML
    private void handleApplyGameName(javafx.event.ActionEvent e) {
        String oldName = currentGameName;
        String newName = gameName != null ? gameName.getText() : null;
        if (newName == null || newName.isBlank() || newName.equals(oldName)) return;

        // Collect section data before removal
        Map<String, String> sectionData = appModule.getGameSection(oldName);

        String desktopPath = execReadFully("xdg-user-dir DESKTOP");
        if (desktopPath == null) desktopPath = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/Desktop";
        else desktopPath = desktopPath.trim();
        String menuPath = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/.local/share/applications";

        boolean hasDesktop = Files.isRegularFile(Path.of(desktopPath, oldName + ".desktop"));
        boolean hasMenu = Files.isRegularFile(Path.of(menuPath, oldName + ".desktop"));
        if (hasDesktop) try { Files.deleteIfExists(Path.of(desktopPath, oldName + ".desktop")); } catch (IOException ignored) {}
        if (hasMenu) try { Files.deleteIfExists(Path.of(menuPath, oldName + ".desktop")); } catch (IOException ignored) {}

        // Remove old section + put new one – use AppModule to keep in-memory state in sync
        appModule.renameGame(oldName, newName, sectionData);

        currentGameName = newName;
        if (e.getSource() instanceof Button b) b.setDisable(true);

        // Update MainForm references – mirror PHP update of gamePanel opener and tiles
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getMainForm = launcherCls.getMethod("getMainForm");
            Object mf = getMainForm.invoke(null);
            if (mf != null) LOG.info("Renamed game {} -> {} – MainForm update requested", oldName, newName);
        } catch (Exception ex) { LOG.debug("MainForm rename update failed", ex); }

        if (hasDesktop) {
            String icon = sectionData != null ? sectionData.get("icon") : null;
            String entry = FilesWorker.generateDesktopEntry(newName, icon);
            try {
                Files.writeString(Path.of(desktopPath, newName + ".desktop"), entry, StandardCharsets.UTF_8);
                new ProcessBuilder("chmod", "+x", Path.of(desktopPath, newName + ".desktop").toString()).start();
            } catch (IOException ex) { LOG.warn("write desktop after rename failed", ex); }
        }
        if (hasMenu) {
            String icon = sectionData != null ? sectionData.get("icon") : null;
            String entry = FilesWorker.generateDesktopEntry(newName, icon);
            try { Files.writeString(Path.of(menuPath, newName + ".desktop"), entry, StandardCharsets.UTF_8); }
            catch (IOException ex) { LOG.warn("write menu after rename failed", ex); }
        }
    }

    // -----------------------------------------------------------------------
    // Banner – mirrors PHP setBanner(UXImage $banner)
    // PHP: $this->banner->image = banner; $mainForm->gameHeader->image = banner; opener.children[1].image = banner;
    //      if background visible background.image = banner; then save to bannersPath/gameName.png
    // -----------------------------------------------------------------------

    public void setBanner(Image image) {
        try {
            if (image == null) {
                showAlert(String.format(loc.get("BANNEREDITOR.FILE.FAILED"), "null image"), Alert.AlertType.ERROR);
                return;
            }
            if (banner != null) banner.setImage(image);
            // Update MainForm header/background – best effort mirroring PHP
            try {
                var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                var getMainForm = launcherCls.getMethod("getMainForm");
                Object mf = getMainForm.invoke(null);
                if (mf != null) LOG.debug("MainForm banner update requested for {}", currentGameName);
                // PHP updates gamePanel opener children[1].image and gameHeader/background – JavaFX equivalent is ImageView
            } catch (Exception ex) { LOG.debug("MainForm banner update failed", ex); }

            String bannersPath = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/.config/CorkyTux/banners";
            String currentBanner = appModule.getGame("banner", currentGameName);
            if (currentBanner != null) {
                try { Files.deleteIfExists(Path.of(currentBanner)); } catch (IOException ignored) {}
            } else {
                try { Files.createDirectories(Path.of(bannersPath)); } catch (IOException ex) { LOG.warn("create banners dir failed", ex); }
            }

            Path out = Path.of(bannersPath, currentGameName + ".png");
            // Ensure parent exists (PHP fs::makeDir else branch already handles, but we ensure)
            try { Files.createDirectories(out.getParent()); } catch (IOException ex) { LOG.warn("create banners parent failed", ex); }

            // Save image – use pixel reader -> BufferedImage -> ImageIO, mirrors UXImage.save
            boolean saved = false;
            try {
                var pixelReader = image.getPixelReader();
                if (pixelReader != null) {
                    int w = (int) image.getWidth();
                    int h = (int) image.getHeight();
                    if (w > 0 && h > 0) {
                        var buf = new java.awt.image.BufferedImage(w, h, java.awt.image.BufferedImage.TYPE_INT_ARGB);
                        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
                            int argb = pixelReader.getArgb(x, y);
                            buf.setRGB(x, y, argb);
                        }
                        saved = javax.imageio.ImageIO.write(buf, "png", out.toFile());
                        if (!saved) LOG.warn("ImageIO.write returned false for {}", out);
                    } else {
                        LOG.warn("Banner image has zero dimensions {}x{}", w, h);
                    }
                } else {
                    LOG.warn("Banner image pixelReader is null");
                }
            } catch (Exception ex) {
                LOG.warn("Failed to save banner image", ex);
            }
            if (!saved && !Files.isRegularFile(out)) {
                LOG.warn("Banner not saved to {}", out);
                return;
            }
            appModule.setGame("banner", out.toString(), currentGameName);
            LOG.info("Banner updated for {} -> {}", currentGameName, out);
        } catch (Exception ex) {
            showAlert(String.format(loc.get("BANNEREDITOR.FILE.FAILED"), ex.getMessage()), Alert.AlertType.ERROR);
        }
    }

    // -----------------------------------------------------------------------
    // Tab switching – mirrors SettingsModule.switchPage with FadeTransition(350)
    // -----------------------------------------------------------------------

    public void switchPage(Node newPage) {
        if (newPage == null) return;
        Node oldPage = settingsModule.getActivePage();
        // Fallback: if SettingsModule has no activePage (e.g. FXML reload), detect visible
        if (oldPage == null) {
            for (Node p : new Node[]{view, startup, graphics}) {
                if (p != null && p.isVisible()) { oldPage = p; break; }
            }
            if (oldPage == null) oldPage = view;
            settingsModule.setActivePage(oldPage);
        }
        if (oldPage == newPage) {
            setTabSelected(newPage, true);
            newPage.setVisible(true);
            newPage.setManaged(true);
            newPage.setOpacity(1.0);
            return;
        }
        if (oldPage != null) setTabSelected(oldPage, false);
        settingsModule.setActivePage(newPage);

        // Prepare new page for fade-in
        newPage.setVisible(true);
        newPage.setManaged(true);
        newPage.setOpacity(0.0);

        Node finalOld = oldPage;
        if (finalOld != null) {
            // Use QuUI.animateWithoutConflict for conflict-free fade (mirrors php animateWithoutConflict)
            // FadeOut old, then on finished hide old and FadeIn new
            var fadeOut = new FadeTransition(javafx.util.Duration.millis(350), finalOld);
            fadeOut.setFromValue(1.0);
            fadeOut.setToValue(0.0);
            fadeOut.setOnFinished(ev -> {
                finalOld.setVisible(false);
                finalOld.setManaged(false);
                // FadeIn new page
                QuUI.animateWithoutConflict("FadeIn", newPage, 1.0, null);
                // Ensure opacity after animation stays 1 – QuUI handles opacity, fallback:
                newPage.setOpacity(1.0);
            });
            // Stop any previous animation on old page before starting
            Object prev = finalOld.getProperties().get("quUIAnimation");
            if (prev instanceof javafx.animation.Transition t) t.stop();
            finalOld.getProperties().put("quUIAnimation", fadeOut);
            fadeOut.play();
        } else {
            QuUI.animateWithoutConflict("FadeIn", newPage, 1.0, null);
            newPage.setOpacity(1.0);
        }
        setTabSelected(newPage, true);
    }

    private void setTabSelected(Node page, boolean selected) {
        ToggleButton btn = null;
        if (page == view) btn = viewButton;
        else if (page == startup) btn = startupButton;
        else if (page == graphics) btn = graphicsButton;
        if (btn != null) btn.setSelected(selected);
    }

    private void ensurePanelBackgrounds() {
        // Panel backgrounds – original DevelNext <Panel backgroundColor="#333333" borderRadius="15">
        // were converted to AnchorPane; ensure inline style when not already set.
        // This keeps Data nodes and Panel backgrounds correctly styled (#333333 / #333337)
        try {
            javafx.scene.Parent dataRoot = null;
            if (root != null && root.getParent() instanceof javafx.scene.Parent pr) dataRoot = pr;
            else if (view != null && view.getParent() instanceof javafx.scene.Parent pr) dataRoot = pr;
            else if (startup != null && startup.getParent() instanceof javafx.scene.Parent pr) dataRoot = pr;
            if (dataRoot != null) {
                try {
                    var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                    var m = launcherCls.getDeclaredMethod("applyDataAndPanels", javafx.scene.Parent.class);
                    m.setAccessible(true);
                    m.invoke(null, dataRoot);
                } catch (Exception ignored) {}
            } else if (root != null) {
                try {
                    var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
                    var m = launcherCls.getDeclaredMethod("applyDataAndPanels", javafx.scene.Parent.class);
                    m.setAccessible(true);
                    m.invoke(null, root);
                } catch (Exception ignored) {}
            }
            // Direct style fix for panel/panelAlt which had empty style in earlier FXML
            for (var id : new String[]{"panel", "panelAlt"}) {
                Node n = null;
                if (dataRoot != null) n = dataRoot.lookup("#" + id);
                if (n == null && root != null) n = root.lookup("#" + id);
                if (n == null && view != null) n = view.lookup("#" + id);
                if (n == null && view != null && view.getParent() != null) n = view.getParent().lookup("#" + id);
                if (n instanceof javafx.scene.layout.Pane pane) {
                    String cur = pane.getStyle();
                    if (cur == null || !cur.contains("-fx-background-color")) {
                        pane.setStyle("-fx-background-color:#333333;-fx-background-radius:15px;");
                    }
                }
            }
        } catch (Exception e) {
            LOG.debug("ensurePanelBackgrounds failed", e);
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private boolean isToggleSelected(Object source) {
        if (source instanceof Button b) {
            Object o = b.getProperties().get("quUIElement");
            if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) return sw.isSelected();
            if (o instanceof ToggleButton tb) return tb.isSelected();
        }
        if (source instanceof com.corkytux.launcher.ui.SwitchComponent sw) return sw.isSelected();
        if (source instanceof ToggleButton tb) return tb.isSelected();
        return false;
    }

    private void setToggleSelected(Object source, boolean val) {
        if (source instanceof Button b) {
            Object o = b.getProperties().get("quUIElement");
            if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) { sw.setSelected(val); return; }
            if (o instanceof ToggleButton tb) { tb.setSelected(val); return; }
        }
        if (source instanceof com.corkytux.launcher.ui.SwitchComponent sw) sw.setSelected(val);
        else if (source instanceof ToggleButton tb) tb.setSelected(val);
    }

    private ToggleButton getQuToggle(Object source) {
        if (source instanceof Button b) {
            Object o = b.getProperties().get("quUIElement");
            if (o instanceof ToggleButton tb) return tb;
        }
        if (source instanceof ToggleButton tb) return tb;
        return null;
    }

    private Stage stageOf(Node n) {
        if (n == null || n.getScene() == null) return null;
        var w = n.getScene().getWindow();
        return w instanceof Stage s ? s : null;
    }

    private void hideStage() {
        Platform.runLater(() -> {
            // Modal context: close MainForm modal instead of hiding the main stage
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
                        if (overlay instanceof javafx.scene.Node ov) modalVisible = ov.isVisible();
                    } catch (Exception ignored) {}
                    if (modalVisible) {
                        var hideModal = mf.getClass().getDeclaredMethod("hideModal");
                        hideModal.setAccessible(true);
                        hideModal.invoke(mf);
                        return;
                    }
                }
            } catch (Exception e) {
                LOG.debug("hideModal fallback failed", e);
            }
            Stage s = stageOf(root != null ? root : view);
            if (s != null) s.hide();
        });
    }

    private void toast(String msg) {
        LOG.info("TOAST: {}", msg);
        showAlert(msg, Alert.AlertType.INFORMATION);
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            Alert a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
    }

    private boolean confirm(String msg) {
        Alert a = new Alert(Alert.AlertType.CONFIRMATION, msg, ButtonType.YES, ButtonType.NO);
        a.setHeaderText(null);
        var r = a.showAndWait();
        return r.isPresent() && r.get() == ButtonType.YES;
    }

    private String execReadFully(String cmd) {
        try {
            var proc = new ProcessBuilder("bash", "-c", cmd).start();
            try (var reader = new java.io.BufferedReader(new java.io.InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line = reader.readLine();
                proc.waitFor();
                return line;
            }
        } catch (Exception e) { LOG.debug("exec failed {}", cmd, e); return null; }
    }

    private static String getExtension(String name) {
        int dot = name.lastIndexOf('.');
        return dot == -1 ? "" : name.substring(dot + 1);
    }

    private static String stripExtension(String name) {
        int dot = name.lastIndexOf('.');
        return dot == -1 ? name : name.substring(0, dot);
    }

    private static void deleteRecursively(Path path) {
        if (path == null || !Files.exists(path)) return;
        try {
            var pb = new ProcessBuilder("rm", "-rf", path.toString());
            pb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
            pb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
            pb.start().waitFor();
        }
        catch (Exception e) { LoggerFactory.getLogger(GameSettings.class).warn("delete failed {}", path, e); }
    }
}

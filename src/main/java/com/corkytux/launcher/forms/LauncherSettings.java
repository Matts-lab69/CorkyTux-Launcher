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

import com.corkytux.launcher.modules.AccentColorManager;
import com.corkytux.launcher.modules.AppModule;
import com.corkytux.launcher.modules.FilesWorker;
import com.corkytux.launcher.modules.Localization;
import com.corkytux.launcher.modules.SettingsModule;
import com.corkytux.launcher.util.QuUI;
import javafx.application.Platform;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.control.*;
import javafx.scene.effect.ColorAdjust;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.input.MouseEvent;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Pane;
import javafx.scene.layout.VBox;
import javafx.scene.text.Font;
import javafx.scene.text.FontWeight;
import javafx.stage.DirectoryChooser;
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
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.ResourceBundle;

/**
 * Java 25 / JavaFX 21 port of {@code launcherSettings.php} (578 lines).
 *
 * <p>Settings form with 5 navigation tabs ({@code paths}, {@code protons},
 * {@code launcher}, {@code graphics}, {@code about}) and a paged content area.
 * All PHP {@code @event} handlers are mirrored as {@code @FXML} methods or
 * programmatic listeners in {@link #initialize}. The critical paged protons
 * list cell factory with download/remove logic, default-proton combo, and path
 * pickers are all preserved with full fidelity.</p>
 *
 * <p>FXML: {@code /fxml/launcherSettings.fxml} – controller set via
 * {@code fx:controller="com.corkytux.launcher.forms.LauncherSettings"}.
 * Pane ids must match FXML: {@code paths}, {@code protons}, {@code launcher},
 * {@code graphics}, {@code about}, etc.</p>
 */
public class LauncherSettings implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(LauncherSettings.class);

    // -----------------------------------------------------------------------
    // FXML – path page
    // -----------------------------------------------------------------------

    @FXML private Pane paths;
    @FXML private Pane protons;
    @FXML private Pane launcher;
    @FXML private Pane graphics;
    @FXML private Pane about;
    @FXML private Pane visuals;

    @FXML private TextField downloadsPath;
    @FXML private TextField installsPath;
    @FXML private TextField prefixesPath;
    @FXML private TextField protonsPath;
    @FXML private TextField protonsPath2;
    @FXML private TextField protonsPath3;

    @FXML private VBox vbox;     // installsPath wrapper
    @FXML private VBox vboxAlt;  // downloadsPath wrapper
    @FXML private VBox vbox3;    // prefixesPath wrapper
    @FXML private VBox vbox4;    // protonsPath wrapper
    @FXML private VBox vboxProtons2;
    @FXML private VBox vboxProtons3;

    // -----------------------------------------------------------------------
    // FXML – protons page
    // -----------------------------------------------------------------------

    @FXML private ListView<ProtonItem> protonsList;
    @FXML private ComboBox<String> protonFilter;
    @FXML private ComboBox<String> defaultProton;
    @FXML private ComboBox<String> protonPathsCombo;

    // -----------------------------------------------------------------------
    // FXML – launcher page
    // -----------------------------------------------------------------------

    @FXML private Button fullscreenLauncher;
    @FXML private Button requestSteam;

    // -----------------------------------------------------------------------
    // FXML – graphics page
    // -----------------------------------------------------------------------

    @FXML private Button wined3d;
    @FXML private Button useWayland;

    // -----------------------------------------------------------------------
    // FXML – about page
    // -----------------------------------------------------------------------

    @FXML private Label version;

    // -----------------------------------------------------------------------
    // FXML – navigation
    // -----------------------------------------------------------------------

    @FXML private ToggleButton pathsButton;
    @FXML private ToggleButton protonsButton;
    @FXML private ToggleButton launcherButton;
    @FXML private ToggleButton aboutButton;
    @FXML private ToggleButton graphicsButton;
    @FXML private ToggleButton visualsButton;

    @FXML private Label label;       // installs
    @FXML private Label label4;      // downloads
    @FXML private Label label6;      // protons
    @FXML private Label labelAlt;    // prefixes
    @FXML private Label label5;      // paths header
    @FXML private Label label3;      // default proton label
    @FXML private Label label9;      // proton hint
    @FXML private Label labelAlt2;   // alias workaround if FXML duplicates
    @FXML private Label label10;     // graphics header

    // Github clickable node – may be Label or ImageView or Pane
    @FXML private Node github;

    @FXML private VBox root;

    // -----------------------------------------------------------------------
    // State
    // -----------------------------------------------------------------------

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();
    private final SettingsModule settingsModule = new SettingsModule();

    // Allow refresh flag stored in protonsList properties – mirrors data('allowRefresh')
    private static final String ALLOW_REFRESH = "allowRefresh";

    /**
     * Mirrors PHP list item shape {@code [name, url]} where url may be null (installed only)
     * or a GitHub asset URL (available download).
     */
    public record ProtonItem(String name, String url) {
        @Override public String toString() { return name; }
    }

    // -----------------------------------------------------------------------
    // Initializable – mirrors doConstruct + all construct handlers
    // -----------------------------------------------------------------------

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        // Active page tracking – mirrors doConstruct: SettingsModule.activePage = paths
        if (paths != null) settingsModule.setActivePage(paths);

        applyLocalizations();
        buildToggleButtons();
        wireActions();

        // Initialize displayed texts – mirrors doDownloadsPathConstruct etc.
        initPathLabels();
        initProtonsList();
        initDefaultProton();
        initVersionLabel();
        initVisualsPane();

        // protonsList initial items already via initProtonsList

        // Global key handler – Esc
        if (root != null) {
            root.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.getCode() == KeyCode.ESCAPE) handleKeyEsc();
            });
        }
    }

    private void applyLocalizations() {
        if (label3 != null) label3.setText(loc.get("LAUNCHERSETTINGS.PROTON.DEFAULT"));
        if (pathsButton != null) pathsButton.setText(loc.get("LAUNCHERSETTINGS.TABS.PATHS"));
        if (protonsButton != null) protonsButton.setText(loc.get("LAUNCHERSETTINGS.TABS.PROTONS"));
        if (launcherButton != null) launcherButton.setText(loc.get("LAUNCHERSETTINGS.TABS.LAUNCHER"));
        if (aboutButton != null) aboutButton.setText(loc.get("LAUNCHERSETTINGS.TABS.ABOUT"));
        if (visualsButton != null) visualsButton.setText(loc.get("LAUNCHERSETTINGS.TABS.VISUALS"));
        if (graphicsButton != null) graphicsButton.setText(loc.get("SETTINGSMODULE.GRAPHICS"));
        if (label5 != null) label5.setText(loc.get("LAUNCHERSETTINGS.PATHS.HEADER"));
        if (label4 != null) label4.setText(loc.get("LAUNCHERSETTINGS.PATHS.DOWNLOADS"));
        if (label != null) label.setText(loc.get("LAUNCHERSETTINGS.PATHS.INSTALLS"));
        if (label6 != null) label6.setText(loc.get("LAUNCHERSETTINGS.PATHS.PROTONS"));
        if (labelAlt != null) labelAlt.setText(loc.get("LAUNCHERSETTINGS.PATHS.PREFIXES"));
        if (label9 != null) label9.setText(loc.get("LAUNCHERSETTINGS.PROTON.HINT"));
        if (label10 != null) label10.setText(loc.get("LAUNCHERSETTINGS.GRAPHICS.HEADER"));
    }

    private void buildToggleButtons() {
        if (fullscreenLauncher != null) {
            var sw = new com.corkytux.launcher.ui.SwitchComponent(loc.get("LAUNCHERSETTINGS.LAUNCHER.FULLSCREEN"));
            String v = appModule.getLauncher("fullscreen", "User Settings");
            sw.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
            sw.setOnToggle(() -> {
                boolean val = sw.isSelected();
                appModule.setLauncher("fullscreen", String.valueOf(val), "User Settings");
            });
            fullscreenLauncher.getProperties().put("quUIElement", sw);
            fullscreenLauncher.setGraphic(sw);
            fullscreenLauncher.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
            fullscreenLauncher.setPrefHeight(34);
            fullscreenLauncher.setMinHeight(34);
            fullscreenLauncher.setMaxHeight(34);
        }
        if (requestSteam != null) {
            var sw = new com.corkytux.launcher.ui.SwitchComponent(loc.get("LAUNCHERSETTINGS.LAUNCHER.REQUESTSTEAM"));
            String v = appModule.getLauncher("noSteamRequest", "User Settings");
            sw.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
            sw.setOnToggle(() -> {
                boolean val = sw.isSelected();
                appModule.setLauncher("noSteamRequest", String.valueOf(val), "User Settings");
            });
            requestSteam.getProperties().put("quUIElement", sw);
            requestSteam.setGraphic(sw);
            requestSteam.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
            requestSteam.setPrefHeight(34);
            requestSteam.setMinHeight(34);
            requestSteam.setMaxHeight(34);
        }
        if (wined3d != null) {
            var sw = new com.corkytux.launcher.ui.SwitchComponent(loc.get("SETTINGSMODULE.USEWINED3D"));
            String v = appModule.getLauncher("gamesUsesWined3d", "User Settings");
            sw.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
            sw.setOnToggle(() -> {
                boolean val = sw.isSelected();
                appModule.setLauncher("gamesUsesWined3d", String.valueOf(val), "User Settings");
            });
            wined3d.getProperties().put("quUIElement", sw);
            wined3d.setGraphic(sw);
            wined3d.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
            wined3d.setPrefHeight(34);
            wined3d.setMinHeight(34);
            wined3d.setMaxHeight(34);
        }
        if (useWayland != null) {
            var sw = new com.corkytux.launcher.ui.SwitchComponent(loc.get("SETTINGSMODULE.NATIVEWAYLAND"));
            String v = appModule.getLauncher("gamesUsesWayland", "User Settings");
            sw.setSelectedSilent("true".equalsIgnoreCase(v) || "1".equals(v));
            sw.setOnToggle(() -> {
                boolean val = sw.isSelected();
                appModule.setLauncher("gamesUsesWayland", String.valueOf(val), "User Settings");
            });
            useWayland.getProperties().put("quUIElement", sw);
            useWayland.setGraphic(sw);
            useWayland.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
            useWayland.setPrefHeight(34);
            useWayland.setMinHeight(34);
            useWayland.setMaxHeight(34);
        }
    }

    private void wireActions() {
        // Path pickers
        if (downloadsPath != null) downloadsPath.setOnMouseClicked(this::handleDownloadsPathClick);
        if (vboxAlt != null) vboxAlt.setOnMouseClicked(this::handleVboxAltClick);
        if (installsPath != null) installsPath.setOnMouseClicked(this::handleInstallsPathClick);
        if (vbox != null) vbox.setOnMouseClicked(this::handleVboxClick);
        if (prefixesPath != null) prefixesPath.setOnMouseClicked(this::handlePrefixesPathClick);
        if (vbox3 != null) vbox3.setOnMouseClicked(this::handleVbox3Click);
        if (protonsPath != null) protonsPath.setOnMouseClicked(this::handleProtonsPathClick);
        if (vbox4 != null) vbox4.setOnMouseClicked(this::handleVbox4Click);
        if (protonsPath2 != null) protonsPath2.setOnMouseClicked(this::handleProtonsPath2Click);
        if (vboxProtons2 != null) vboxProtons2.setOnMouseClicked(this::handleVboxProtons2Click);
        if (protonsPath3 != null) protonsPath3.setOnMouseClicked(this::handleProtonsPath3Click);
        if (vboxProtons3 != null) vboxProtons3.setOnMouseClicked(this::handleVboxProtons3Click);

        // Tabs
        if (pathsButton != null) pathsButton.setOnAction(this::handlePathsButtonAction);
        if (protonsButton != null) protonsButton.setOnAction(this::handleProtonsButtonAction);
        if (launcherButton != null) launcherButton.setOnAction(this::handleLauncherButtonAction);
        if (aboutButton != null) aboutButton.setOnAction(this::handleAboutButtonAction);
        if (graphicsButton != null) graphicsButton.setOnAction(this::handleGraphicsButtonAction);
        if (visualsButton != null) visualsButton.setOnAction(this::handleVisualsButtonAction);

        // Combos / toggles
        if (defaultProton != null) defaultProton.setOnAction(this::handleDefaultProtonAction);

        // External links
        if (github != null) github.setOnMouseClicked(this::handleGithubClick);

        // protonsList selection reset – mirrors doProtonsListAction selectedIndex = -1
        if (protonsList != null) {
            protonsList.setOnMouseClicked(e -> protonsList.getSelectionModel().clearSelection());
        }
    }

    private void initPathLabels() {
        // doDownloadsPathConstruct
        if (downloadsPath != null) {
            String v = appModule.getLauncher("downloadsPath", "User Settings");
            if (v == null) v = execReadFully("xdg-user-dir DOWNLOAD");
            if (v != null) downloadsPath.setText(v.trim());
        }
        // doInstallsPathConstruct
        if (installsPath != null) {
            String v = appModule.getLauncher("installsPath", "User Settings");
            installsPath.setText(v != null ? v : loc.get("LAUNCHERSETTINGS.PATHS.NOPATH"));
        }
        // doPrefixesPathConstruct – uses getBasePathFor('prefixes')
        if (prefixesPath != null) {
            prefixesPath.setText(getBasePathFor("prefixes"));
        }
        // doProtonsPathConstruct
        if (protonsPath != null) {
            String v = appModule.getLauncher("protonsPath", "User Settings");
            if (v == null) v = Path.of("./protons").toAbsolutePath().toString();
            protonsPath.setText(v);
        }
        // protonsPath2 – optional
        if (protonsPath2 != null) {
            String v = appModule.getLauncher("protonsPath2", "User Settings");
            protonsPath2.setText(v != null ? v : "");
            protonsPath2.setTooltip(new javafx.scene.control.Tooltip("Click: select folder | Right-click: clear"));
        }
        // protonsPath3 – optional
        if (protonsPath3 != null) {
            String v = appModule.getLauncher("protonsPath3", "User Settings");
            protonsPath3.setText(v != null ? v : "");
            protonsPath3.setTooltip(new javafx.scene.control.Tooltip("Click: select folder | Right-click: clear"));
        }
    }

    private void initVersionLabel() {
        if (version != null) version.setText(AppModule.VERSION + " version");
    }

    private void initVisualsPane() {
        if (visuals == null) return;
        var acm = AccentColorManager.getInstance();
        var children = visuals.getChildren();

        // Header
        var header = new Label("Theme Colors");
        header.setStyle("-fx-font-size:15px; -fx-font-weight:bold; -fx-text-fill:#ffffff;");
        header.setPrefHeight(36);
        children.add(header);

        // Description
        var desc = new Label("Choose an accent color for buttons, switches, and highlights");
        desc.setStyle("-fx-text-fill:#b3b3b3; -fx-font-size:11px;");
        desc.setPrefHeight(20);
        children.add(desc);

        // Color grid
        var grid = new javafx.scene.layout.FlowPane(8, 8);
        grid.setPrefWidth(480);
        grid.setAlignment(Pos.CENTER);

        String currentId = acm.getCurrentId();

        for (var entry : AccentColorManager.getAllAccents().entrySet()) {
            String id = entry.getKey();
            String name = entry.getValue()[0];
            String hex = entry.getValue()[1];

            var swatch = new Button(name);
            swatch.setPrefSize(80, 32);
            swatch.setMinSize(80, 32);
            swatch.setMaxSize(80, 32);

            boolean selected = id.equals(currentId);
            applySwatchStyle(swatch, hex, selected);

            swatch.setOnAction(e -> {
                acm.setAccent(id);
                applyAccentToAllScenes();
                // Refresh all swatch styles
                refreshSwatchStyles(grid, acm);
            });

            grid.getChildren().add(swatch);
        }

        children.add(grid);
    }

    private void applySwatchStyle(Button btn, String hex, boolean selected) {
        if (selected) {
            btn.setStyle("-fx-background-color:" + hex + ";"
                    + "-fx-text-fill:#ffffff; -fx-font-weight:bold;"
                    + "-fx-background-radius:8; -fx-cursor:hand;"
                    + "-fx-border-color:#ffffff; -fx-border-width:2; -fx-border-radius:8;");
        } else {
            String hoverLight = lighten(hex, 0.15);
            btn.setStyle("-fx-background-color:" + hex + ";"
                    + "-fx-text-fill:#ffffff; -fx-font-weight:bold;"
                    + "-fx-background-radius:8; -fx-cursor:hand;");
            btn.setOnMouseEntered(e ->
                    btn.setStyle("-fx-background-color:" + hoverLight + ";"
                            + "-fx-text-fill:#ffffff; -fx-font-weight:bold;"
                            + "-fx-background-radius:8; -fx-cursor:hand;"
                            + "-fx-border-color:#666666; -fx-border-width:1; -fx-border-radius:8;"));
            btn.setOnMouseExited(e ->
                    btn.setStyle("-fx-background-color:" + hex + ";"
                            + "-fx-text-fill:#ffffff; -fx-font-weight:bold;"
                            + "-fx-background-radius:8; -fx-cursor:hand;"));
        }
    }

    private void refreshSwatchStyles(javafx.scene.layout.FlowPane grid, AccentColorManager acm) {
        String currentId = acm.getCurrentId();
        int i = 0;
        for (var entry : AccentColorManager.getAllAccents().entrySet()) {
            if (i >= grid.getChildren().size()) break;
            var node = grid.getChildren().get(i);
            if (node instanceof Button btn) {
                String id = entry.getKey();
                String hex = entry.getValue()[1];
                boolean selected = id.equals(currentId);
                applySwatchStyle(btn, hex, selected);
            }
            i++;
        }
    }

    private static void applyAccentToAllScenes() {
        var acm = AccentColorManager.getInstance();
        String css = acm.generateCss();
        // Write to temp file for JavaFX to load
        try {
            var tmpDir = Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".config", "CorkyTux");
            Files.createDirectories(tmpDir);
            var cssFile = tmpDir.resolve("accent-override.css");
            Files.writeString(cssFile, css);
            String url = cssFile.toUri().toString();
            // Apply to ALL open stages
            for (var window : javafx.stage.Window.getWindows()) {
                if (window instanceof Stage stage && stage.getScene() != null) {
                    var sheets = stage.getScene().getStylesheets();
                    sheets.removeIf(s -> s.contains("accent-override"));
                    sheets.add(url);
                }
            }
        } catch (Exception e) {
            LOG.warn("Failed to write accent CSS", e);
        }
    }

    private static String lighten(String hex, double amount) {
        int r = Integer.parseInt(hex.substring(1, 3), 16);
        int g = Integer.parseInt(hex.substring(3, 5), 16);
        int b = Integer.parseInt(hex.substring(5, 7), 16);
        r = (int) Math.min(255, r + (255 - r) * amount);
        g = (int) Math.min(255, g + (255 - g) * amount);
        b = (int) Math.min(255, b + (255 - b) * amount);
        return String.format("#%02X%02X%02X", r, g, b);
    }

    private void initDefaultProton() {
        if (defaultProton == null) return;
        ObservableList<String> items = FXCollections.observableArrayList();
        items.add("GE-Proton Latest");
        // Scan all configured proton paths so newly added paths appear immediately
        for (String p : getAllProtonPaths()) {
            if (p != null && !p.isBlank()) {
                items.addAll(FilesWorker.getInstalledProtonsForPath(p));
            }
        }
        defaultProton.setItems(items);
        String cur = appModule.getLauncher("defaultProton", "User Settings");
        defaultProton.setValue(cur != null ? cur : "GE-Proton Latest");
    }

    private void initProtonsList() {
        if (protonsList == null) return;

        // Update paths display
        updateProtonPathsDisplay();

        // Listen for path selection changes to refresh protons list
        if (protonPathsCombo != null) {
            protonPathsCombo.valueProperty().addListener((obs, oldVal, newVal) -> {
                if (newVal != null && !newVal.equals(oldVal)) {
                    applyProtonFilter();
                }
            });
        }

        // Cell factory – mirrors full doProtonsListConstruct closure
        protonsList.setCellFactory(lv -> new ListCell<>() {
            @Override
            protected void updateItem(ProtonItem item, boolean empty) {
                super.updateItem(item, empty);
                if (empty || item == null) {
                    setGraphic(null);
                    setText(null);
                    return;
                }
                String protonPath = getSelectedProtonPathInstance();
                boolean isInstalled = Files.isRegularFile(Path.of(protonPath, item.name(), "proton"));

                var label = new Label(item.name());
                label.setStyle("-fx-text-fill: white;");

                var dnBtn = new Button();
                dnBtn.setPrefSize(20, 20);
                dnBtn.setMaxSize(20, 20);
                dnBtn.setMinSize(20, 20);
                dnBtn.getStyleClass().add("jfx-button");
                dnBtn.setStyle("-fx-background-radius: 15px; -fx-cursor: hand;");
                dnBtn.setContentDisplay(ContentDisplay.GRAPHIC_ONLY);

                var hbox = new HBox(8);
                hbox.setAlignment(Pos.CENTER_LEFT);

                // refresh func – mirrors PHP $refreshFunc
                Runnable refreshFunc = () -> {
                    var items = protonsList.getItems();
                    for (int i = 0; i < items.size(); i++) {
                        var ci = items.get(i);
                        if (ci.name().equals(item.name())) {
                            String url = item.url();
                            items.remove(i);
                            if (url != null) items.add(i, new ProtonItem(item.name(), url));
                            break;
                        }
                    }
                };

                // download func
                Runnable dnFunc = () -> {
                    Object controller = com.corkytux.launcher.Launcher.getFormController("protonDownloader");
                    if (controller instanceof ProtonDownloader pd && pd.isDownloading()) {
                        LOG.info("Proton download already in progress – ignoring click for {}", item.name());
                        return;
                    }
                    dnBtn.setDisable(true);
                    try {
                        LOG.info("Proton download: showing protonDownloader form for {}", item.name());
                        com.corkytux.launcher.Launcher.showForm("protonDownloader");
                        controller = com.corkytux.launcher.Launcher.getFormController("protonDownloader");
                        LOG.info("Proton download: controller={} url={}", controller != null ? controller.getClass().getSimpleName() : "null", item.url());
                        if (controller instanceof ProtonDownloader downloader) {
                            downloader.startDownload(item.name(), item.url());
                        } else {
                            LOG.warn("ProtonDownloader controller not found after showForm, controller={}", controller);
                        }
                    } catch (Exception e) {
                        LOG.warn("proton download flow failed", e);
                    } finally {
                        dnBtn.setDisable(false);
                    }
                };

                // remove func – runs on FX thread, blocks briefly for rm -rf
                Runnable rmFunc = () -> {
                    Path target = Path.of(protonPath, item.name()).toAbsolutePath();
                    LOG.info("rmFunc: target={} exists={}", target, Files.exists(target));
                    if (!Files.exists(target)) {
                        LOG.warn("rmFunc: directory does not exist: {}", target);
                        refreshFunc.run();
                        return;
                    }
                    dnBtn.setDisable(true);
                    Thread.ofVirtual().start(() -> {
                        try {
                            ProcessBuilder pb = new ProcessBuilder("rm", "-rf", target.toString());
                            pb.redirectErrorStream(true);
                            var proc = pb.start();
                            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream()))) {
                                String line; while ((line = reader.readLine()) != null) LOG.debug("rm: {}", line);
                            }
                            int exit = proc.waitFor();
                            LOG.info("rmFunc: rm exit={} for {}", exit, target);
                        } catch (Exception e) {
                            LOG.warn("rm proton failed", e);
                        }
                        Platform.runLater(() -> {
                            dnBtn.setDisable(false);
                            refreshFunc.run();
                        });
                    });
                };

                ImageView graphic;
                if (isInstalled) {
                    // hover brightness – mirrors ColorAdjustEffectBehaviour brightness 0.15 when HOVER
                    var hoverAdjust = new ColorAdjust();
                    hoverAdjust.setBrightness(0.15);
                    dnBtn.hoverProperty().addListener((obs, was, isHover) -> {
                        if (isHover) dnBtn.setEffect(hoverAdjust);
                        else dnBtn.setEffect(null);
                    });
                    dnBtn.setStyle(dnBtn.getStyle() + "-fx-background-color: #fb2121;");
                    graphic = imageView("/img/remove.png", 15);
                    dnBtn.setOnAction(e -> rmFunc.run());
                    label.setFont(Font.font("System", FontWeight.BOLD, 12));
                } else {
                    graphic = imageView("/img/download.png", 15);
                    dnBtn.setOnAction(e -> dnFunc.run());
                    label.setFont(Font.font("System", 12));
                }
                if (graphic != null) {
                    graphic.setPreserveRatio(true);
                    dnBtn.setGraphic(graphic);
                }

                hbox.getChildren().addAll(dnBtn, label);
                setGraphic(hbox);
                setText(null);
            }
        });

        // Populate installed protons – mirrors foreach installedProtons add
        String selectedPath = getSelectedProtonPathInstance();
        var installed = FilesWorker.getInstalledProtonsForPath(selectedPath);
        ObservableList<ProtonItem> items = FXCollections.observableArrayList();
        for (String p : installed) items.add(new ProtonItem(p, null));
        protonsList.setItems(items);

        // Store installed protons for filtering
        protonsList.getProperties().put("installedProtons", installed);

        // Setup filter ComboBox
        if (protonFilter != null) {
            protonFilter.getItems().clear();
            protonFilter.getItems().addAll("All", "GE-Proton", "CachyOS");
            protonFilter.setValue("All");
        }

        // Background thread fetching releases – mirrors new Thread fetchProtonReleases
        Thread.ofVirtual().start(() -> {
            var geReleases = FilesWorker.fetchProtonReleases();
            var cachyosReleases = FilesWorker.fetchCachyOSProtonReleases();

            java.util.LinkedHashMap<String, Map<String, String>> allReleases = new java.util.LinkedHashMap<>();
            if (geReleases != null) allReleases.putAll(geReleases);
            if (cachyosReleases != null) allReleases.putAll(cachyosReleases);

            if (allReleases.isEmpty()) {
                Platform.runLater(() -> toast(loc.get("GAMESETTINGS.NOGITHUBAPI")));
                protonsList.getProperties().put(ALLOW_REFRESH, Boolean.TRUE);
                return;
            }

            for (Map.Entry<String, Map<String, String>> entry : allReleases.entrySet()) {
                String release = entry.getKey();
                String url = entry.getValue().get("url");
                if (installed.contains(release)) {
                    Platform.runLater(() -> {
                        var list = protonsList.getItems();
                        for (int i = 0; i < list.size(); i++) {
                            var it = list.get(i);
                            if (it.name().equals(release) && it.url() == null) {
                                list.remove(i);
                                list.add(i, new ProtonItem(release, url));
                                break;
                            }
                        }
                    });
                } else {
                    Platform.runLater(() -> protonsList.getItems().add(new ProtonItem(release, url)));
                }
            }

            // Store all releases for filtering
            protonsList.getProperties().put("allReleases", allReleases);
            protonsList.getProperties().put("installedProtons", installed);

            // Setup filter action
            if (protonFilter != null) {
                Platform.runLater(() -> protonFilter.setOnAction(e -> applyProtonFilter()));
            }
        });
    }

    public static String getSelectedProtonPath() {
        // Get the instance's protonPathsCombo value
        Object controller = com.corkytux.launcher.Launcher.getFormController("launcherSettings");
        if (controller instanceof LauncherSettings ls && ls.protonPathsCombo != null) {
            String val = ls.protonPathsCombo.getValue();
            if (val != null) {
                int idx = val.indexOf(": ");
                if (idx > 0) {
                    return val.substring(idx + 2);
                }
            }
        }
        return getBasePathFor("protons");
    }

    private String getSelectedProtonPathInstance() {
        if (protonPathsCombo == null) return getBasePathFor("protons");
        String val = protonPathsCombo.getValue();
        if (val == null) return getBasePathFor("protons");
        // Extract path from "Path 1: /home/user/..." format
        int idx = val.indexOf(": ");
        if (idx > 0) {
            return val.substring(idx + 2);
        }
        return getBasePathFor("protons");
    }

    @SuppressWarnings("unchecked")
    private void applyProtonFilter() {
        if (protonFilter == null || protonsList == null) return;
        String filter = protonFilter.getValue();
        var allReleases = (java.util.Map<String, Map<String, String>>) protonsList.getProperties().get("allReleases");
        
        // Refresh installed protons based on selected path
        String selectedPath = getSelectedProtonPathInstance();
        var installedProtons = FilesWorker.getInstalledProtonsForPath(selectedPath);
        protonsList.getProperties().put("installedProtons", installedProtons);
        
        if (allReleases == null) return;

        ObservableList<ProtonItem> filtered = FXCollections.observableArrayList();

        // Always show installed first
        for (String p : installedProtons) {
            String url = allReleases.containsKey(p) ? allReleases.get(p).get("url") : null;
            filtered.add(new ProtonItem(p, url));
        }

        // Add releases based on filter
        for (var entry : allReleases.entrySet()) {
            String name = entry.getKey();
            String url = entry.getValue().get("url");
            if (installedProtons.contains(name)) continue;

            boolean match = switch (filter) {
                case "GE-Proton" -> name.startsWith("GE-");
                case "CachyOS" -> name.startsWith("cachyos");
                default -> true;
            };
            if (match) filtered.add(new ProtonItem(name, url));
        }

        protonsList.setItems(filtered);
    }

    // -----------------------------------------------------------------------
    // Event handlers – mirrors PHP @event methods
    // -----------------------------------------------------------------------

    @FXML
    private void handleDownloadsPathClick(MouseEvent e) {
        boolean ok = SettingsModule.setWithDirChooser("downloadsPath", downloadsPath, windowOf(downloadsPath));
        if (ok && downloadsPath != null) LOG.debug("downloadsPath set to {}", downloadsPath.getText());
    }

    @FXML
    private void handleVboxAltClick(MouseEvent e) { handleDownloadsPathClick(e); }

    @FXML
    private void handleInstallsPathClick(MouseEvent e) {
        boolean ok = SettingsModule.setWithDirChooser("installsPath", installsPath, windowOf(installsPath));
        if (ok) LOG.debug("installsPath set");
    }

    @FXML
    private void handleVboxClick(MouseEvent e) { handleInstallsPathClick(e); }

    @FXML
    private void handlePrefixesPathClick(MouseEvent e) {
        boolean ok = SettingsModule.setWithDirChooser("prefixesPath", prefixesPath, windowOf(prefixesPath));
        if (ok) LOG.debug("prefixesPath set");
    }

    @FXML
    private void handleVbox3Click(MouseEvent e) { handlePrefixesPathClick(e); }

    @FXML
    private void handleProtonsPathClick(MouseEvent e) {
        boolean result = SettingsModule.setWithDirChooser("protonsPath", protonsPath, windowOf(protonsPath));
        if (!result) return;
        if (protonsList != null) protonsList.getItems().clear();
        if (defaultProton != null) defaultProton.getItems().clear();
        appModule.setLauncher("defaultProton", null, "User Settings");
        initDefaultProton();
        initProtonsList();
        updateProtonPathsDisplay();
    }

    @FXML
    private void handleVbox4Click(MouseEvent e) { handleProtonsPathClick(e); }

    @FXML
    private void handleProtonsPath2Click(MouseEvent e) {
        if (e.getButton() == javafx.scene.input.MouseButton.SECONDARY) {
            // Right-click: clear path
            protonsPath2.setText("");
            appModule.setLauncher("protonsPath2", "", "User Settings");
            updateProtonPathsDisplay();
            if (protonsList != null) protonsList.getItems().clear();
            if (defaultProton != null) defaultProton.getItems().clear();
            initDefaultProton();
            initProtonsList();
            return;
        }
        boolean result = SettingsModule.setWithDirChooser("protonsPath2", protonsPath2, windowOf(protonsPath2));
        if (!result) return;
        updateProtonPathsDisplay();
        if (protonsList != null) protonsList.getItems().clear();
        if (defaultProton != null) defaultProton.getItems().clear();
        initDefaultProton();
        initProtonsList();
    }

    @FXML
    private void handleVboxProtons2Click(MouseEvent e) { handleProtonsPath2Click(e); }

    @FXML
    private void handleProtonsPath3Click(MouseEvent e) {
        if (e.getButton() == javafx.scene.input.MouseButton.SECONDARY) {
            // Right-click: clear path
            protonsPath3.setText("");
            appModule.setLauncher("protonsPath3", "", "User Settings");
            updateProtonPathsDisplay();
            if (protonsList != null) protonsList.getItems().clear();
            if (defaultProton != null) defaultProton.getItems().clear();
            initDefaultProton();
            initProtonsList();
            return;
        }
        boolean result = SettingsModule.setWithDirChooser("protonsPath3", protonsPath3, windowOf(protonsPath3));
        if (!result) return;
        updateProtonPathsDisplay();
        if (protonsList != null) protonsList.getItems().clear();
        if (defaultProton != null) defaultProton.getItems().clear();
        initDefaultProton();
        initProtonsList();
    }

    @FXML
    private void handleVboxProtons3Click(MouseEvent e) { handleProtonsPath3Click(e); }

    @FXML
    private void handlePathsButtonAction(javafx.event.ActionEvent e) { switchPage(paths); }

    @FXML
    private void handleProtonsButtonAction(javafx.event.ActionEvent e) { switchPage(protons); }

    @FXML
    private void handleLauncherButtonAction(javafx.event.ActionEvent e) { switchPage(launcher); }

    @FXML
    private void handleAboutButtonAction(javafx.event.ActionEvent e) { switchPage(about); }

    @FXML
    private void handleGraphicsButtonAction(javafx.event.ActionEvent e) { switchPage(graphics); }

    @FXML
    private void handleVisualsButtonAction(javafx.event.ActionEvent e) { switchPage(visuals); }

    @FXML
    private void handleDefaultProtonAction(javafx.event.ActionEvent e) {
        if (defaultProton == null || defaultProton.getValue() == null) return;
        appModule.setLauncher("defaultProton", defaultProton.getValue(), "User Settings");
    }

    @FXML
    private void handleGithubClick(MouseEvent e) {
        openUrl("https://github.com/Matts-lab69/corkytux");
    }

    @FXML
    private void handleShow() {
        // Mirrors doShow: if allowRefresh then clear and reconstruct
        if (protonsList != null && Boolean.TRUE.equals(protonsList.getProperties().get(ALLOW_REFRESH))) {
            protonsList.getProperties().put(ALLOW_REFRESH, Boolean.FALSE);
            protonsList.getItems().clear();
            initProtonsList();
        }
    }

    /** Called from FXML show event or externally. */
    public void doShow() { handleShow(); }

    @FXML
    private void handleKeyEsc() {
        hideStage();
    }

    // -----------------------------------------------------------------------
    // Page switching – mirrors SettingsModule.switchPage with fade
    // -----------------------------------------------------------------------

    public void switchPage(Node newPage) {
        if (newPage == null) return;
        
        // Hide all pages immediately, show only the target
        for (Node p : new Node[]{paths, protons, launcher, graphics, visuals, about}) {
            if (p == null) continue;
            if (p == newPage) {
                p.setVisible(true);
                p.setManaged(true);
            } else {
                p.setVisible(false);
                p.setManaged(false);
            }
        }
        
        settingsModule.switchPage(newPage);
    }

    // -----------------------------------------------------------------------
    // Static helper – mirrors PHP static getBasePathFor
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code static function getBasePathFor($for)}.
     *
     * <pre>
     * $userHome = System::getProperty('user.home');
     * $defaultDir = "$userHome/.local/share/CorkyTux/$for";
     * $userDir = app()->appModule()->launcher->get("$for\Path",'User Settings');
     * if ($userDir == null){ fs::ensureParent($defaultDir); fs::makeDir($defaultDir); return $defaultDir; }
     * return $userDir;
     * </pre>
     */
    public static String getBasePathFor(String forWhat) {
        boolean isRoot = "root".equals(System.getProperty("user.name"));
        String userHome = isRoot ? com.corkytux.launcher.modules.FilesWorker.getExpectedHome() : System.getProperty("user.home");
        String defaultDir = Path.of(userHome, ".local/share/CorkyTux", forWhat).toString();
        String userDir = AppModule.getInstance().getLauncher(forWhat + "Path", "User Settings");
        if (userDir == null || userDir.isBlank()) {
            try {
                Path p = Path.of(defaultDir);
                if (p.getParent() != null) Files.createDirectories(p.getParent());
                Files.createDirectories(p);
            } catch (IOException e) {
                LOG.warn("ensure default dir failed {}", defaultDir, e);
            }
            return defaultDir;
        }
        return userDir;
    }

    /**
     * Returns all non-empty proton paths (primary + optional 2 & 3).
     * Used by ProtonDownloader to know where to install.
     */
    public static java.util.List<String> getAllProtonPaths() {
        var paths = new java.util.ArrayList<String>();
        String main = getBasePathFor("protons");
        if (main != null && !main.isBlank()) paths.add(main);
        String p2 = AppModule.getInstance().getLauncher("protonsPath2", "User Settings");
        if (p2 != null && !p2.isBlank()) paths.add(p2);
        String p3 = AppModule.getInstance().getLauncher("protonsPath3", "User Settings");
        if (p3 != null && !p3.isBlank()) paths.add(p3);
        return paths;
    }

    private void updateProtonPathsDisplay() {
        if (protonPathsCombo == null) return;
        var paths = getAllProtonPaths();
        protonPathsCombo.getItems().clear();
        if (paths.isEmpty()) {
            protonPathsCombo.getItems().add("No paths configured");
            protonPathsCombo.setValue("No paths configured");
        } else {
            for (int i = 0; i < paths.size(); i++) {
                String p = paths.get(i);
                String label = "Path " + (i + 1) + ": " + p;
                protonPathsCombo.getItems().add(label);
            }
            protonPathsCombo.setValue(protonPathsCombo.getItems().get(0));
        }
    }

    // -----------------------------------------------------------------------
    // Static refresh – called from ProtonDownloader after tar extraction
    // -----------------------------------------------------------------------

    public static void refreshProtonsList() {
        Object controller = com.corkytux.launcher.Launcher.getFormController("launcherSettings");
        if (controller instanceof LauncherSettings ls) {
            Platform.runLater(() -> {
                if (ls.protonsList != null) {
                    ls.protonsList.getItems().clear();
                    ls.initProtonsList();
                }
                if (ls.defaultProton != null) {
                    ls.defaultProton.getItems().clear();
                    ls.initDefaultProton();
                }
            });
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private javafx.stage.Window windowOf(Node n) {
        if (n == null || n.getScene() == null) return null;
        return n.getScene().getWindow();
    }

    private TextInputControl asTextInput(Label label) {
        if (label == null) return null;
        // Wrap Label as TextInputControl adapter – SettingsModule expects TextInputControl
        // We create a TextField proxy that syncs text bidirectionally
        var tf = new TextField(label.getText());
        tf.textProperty().addListener((obs, o, v) -> label.setText(v));
        label.textProperty().addListener((obs, o, v) -> tf.setText(v));
        return tf;
    }

    private ToggleButton getQuToggle(Button host) {
        if (host == null) return null;
        Object o = host.getProperties().get("quUIElement");
        if (o instanceof ToggleButton tb) return tb;
        return null;
    }

    private boolean isToggleSelected(Button host) {
        if (host == null) return false;
        Object o = host.getProperties().get("quUIElement");
        if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) return sw.isSelected();
        if (o instanceof ToggleButton tb) return tb.isSelected();
        return false;
    }

    private void setToggleSelected(Button host, boolean val) {
        if (host == null) return;
        Object o = host.getProperties().get("quUIElement");
        if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) sw.setSelected(val);
        else if (o instanceof ToggleButton tb) tb.setSelected(val);
    }

    private ImageView imageView(String resource, int size) {
        try (var is = getClass().getResourceAsStream(resource)) {
            if (is == null) {
                try (var alt = getClass().getResourceAsStream("/img/" + Path.of(resource).getFileName())) {
                    if (alt == null) return null;
                    var img = new Image(alt);
                    var iv = new ImageView(img);
                    iv.setFitWidth(size); iv.setFitHeight(size); iv.setPreserveRatio(true);
                    return iv;
                }
            }
            var img = new Image(is);
            var iv = new ImageView(img);
            iv.setFitWidth(size); iv.setFitHeight(size); iv.setPreserveRatio(true);
            return iv;
        } catch (Exception e) {
            LOG.debug("imageView failed {}", resource, e);
            return null;
        }
    }

    private String execReadFully(String command) {
        try {
            var proc = new ProcessBuilder("bash", "-c", command).start();
            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line = reader.readLine();
                proc.waitFor();
                return line != null ? line.trim() : null;
            }
        } catch (Exception e) {
            LOG.debug("exec failed {}", command, e);
            return null;
        }
    }

    private void openUrl(String url) {
        try { com.corkytux.launcher.modules.FilesWorker.openWithXdgOpen(url); }
        catch (IOException e) { LOG.warn("xdg-open failed {}", url, e); }
        catch (Exception e) { LOG.warn("xdg-open failed {}", url, e); }
    }

    private void hideStage() {
        Stage s = windowOf(root != null ? root : paths) instanceof Stage st ? st : null;
        if (s != null) s.hide();
        else if (root != null) root.setVisible(false);
    }

    private void toast(String msg) {
        LOG.info("TOAST: {}", msg);
        Platform.runLater(() -> {
            var alert = new Alert(Alert.AlertType.INFORMATION, msg, ButtonType.OK);
            alert.setHeaderText(null);
            alert.show();
        });
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            var a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
    }
}

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
 * {@code launcher}, {@code about}) and a paged content area.
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
    @FXML private Pane plugins;
    @FXML private Pane about;
    @FXML private Pane visuals;
    @FXML private VBox integrations;
    @FXML private javafx.scene.control.ScrollPane integrationsScroll;

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
    @FXML private ToggleButton pluginsButton;
    @FXML private ToggleButton aboutButton;
    @FXML private ToggleButton visualsButton;
    @FXML private ToggleButton integrationsButton;

    @FXML private Label label;       // installs
    @FXML private Label label4;      // downloads
    @FXML private Label label6;      // protons
    @FXML private Label labelAlt;    // prefixes
    @FXML private Label label5;      // paths header
    @FXML private Label label3;      // default proton label
    @FXML private Label label9;      // proton hint
    @FXML private Label labelAlt2;   // alias workaround if FXML duplicates
    @FXML private Label label10;     // graphics header
    @FXML private ImageView image;   // About section image

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
        if (paths != null) settingsModule.setActivePage(visuals);
        if (visuals != null) switchPage(visuals);

        applyLocalizations();
        buildToggleButtons();
        initTabIcons();
        wireActions();

        // Initialize displayed texts – mirrors doDownloadsPathConstruct etc.
        initPathLabels();
        initProtonsList();
        initDefaultProton();
        initVersionLabel();
        initVisualsPane();
        initIntegrationsPane();

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
        if (pluginsButton != null) pluginsButton.setOnAction(this::handlePluginsButtonAction);
        if (aboutButton != null) aboutButton.setOnAction(this::handleAboutButtonAction);
        if (visualsButton != null) visualsButton.setOnAction(this::handleVisualsButtonAction);
        if (integrationsButton != null) integrationsButton.setOnAction(this::handleIntegrationsButtonAction);

        // Combos / toggles
        if (defaultProton != null) defaultProton.setOnAction(this::handleDefaultProtonAction);

        // External links
        if (github != null) github.setOnMouseClicked(this::handleGithubClick);
        if (github instanceof ImageView giv) {
            com.corkytux.launcher.modules.ThemedIcons.applyTo(giv, "/.data/img/github.png");
        }

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

        // Theme mode header
        var themeHeader = new Label("Theme Mode");
        themeHeader.getStyleClass().add("label");
        themeHeader.setStyle("-fx-font-size:18px; -fx-font-weight:bold;");
        themeHeader.setPrefHeight(40);
        children.add(themeHeader);

        // Dark / Light toggle row
        var tm = com.corkytux.launcher.modules.ThemeManager.getInstance();
        var themeRow = new javafx.scene.layout.HBox(8);
        themeRow.setAlignment(Pos.CENTER_LEFT);
        var darkBtn = new Button("🌙 Dark");
        var lightBtn = new Button("☀ Light");
        for (var b : new Button[]{darkBtn, lightBtn}) {
            b.setPrefSize(110, 34);
            b.setCursor(javafx.scene.Cursor.HAND);
        }
        Runnable refreshThemeBtns = () -> {
            boolean isLight = tm.isLight();
            darkBtn.setStyle(themeBtnStyle(!isLight, false));
            lightBtn.setStyle(themeBtnStyle(isLight, true));
        };
        refreshThemeBtns.run();
        darkBtn.setOnAction(e -> {
            tm.setTheme(com.corkytux.launcher.modules.ThemeManager.DARK);
            com.corkytux.launcher.Launcher.reapplyThemeToAllScenes();
            refreshThemeBtns.run();
        });
        lightBtn.setOnAction(e -> {
            tm.setTheme(com.corkytux.launcher.modules.ThemeManager.LIGHT);
            com.corkytux.launcher.Launcher.reapplyThemeToAllScenes();
            refreshThemeBtns.run();
        });
        themeRow.getChildren().addAll(darkBtn, lightBtn);
        children.add(themeRow);

        var themeDesc = new Label("Dark is the default look. Light is easier on bright screens.");
        themeDesc.getStyleClass().add("label-secondary");
        themeDesc.setStyle("-fx-font-size:11px;");
        themeDesc.setPrefHeight(20);
        children.add(themeDesc);

        // Header
        var header = new Label("Theme Colors");
        header.getStyleClass().add("label");
        header.setStyle("-fx-font-size:18px; -fx-font-weight:bold;");
        header.setPrefHeight(40);
        children.add(header);

        // Description
        var desc = new Label("Choose an accent color for buttons, switches, and highlights");
        desc.getStyleClass().add("label-secondary");
        desc.setStyle("-fx-font-size:11px;");
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

    // ── Integrations pane ───────────────────────────────────────────────

    private void initIntegrationsPane() {
        if (integrations == null) return;
        var im = com.corkytux.launcher.modules.IntegrationsManager.getInstance();
        var children = integrations.getChildren();
        children.clear();

        var header = new Label("🌐 Integrations");
        header.getStyleClass().add("label");
        header.setStyle("-fx-font-size:18px; -fx-font-weight:bold;");
        children.add(header);

        var desc = new Label("Connect external platforms. Local scans need no API key.");
        desc.getStyleClass().add("label-secondary");
        desc.setStyle("-fx-font-size:11px;");
        desc.setWrapText(true);
        children.add(desc);

        // Steam native
        children.add(buildIntegrationRow("Steam",
                "Native library support – scan installed Steam games",
                "steam", false, null,
                "Scan & Import", () -> Thread.ofVirtual().start(() -> {
                    var games = im.scanSteamLibrary();
                    int imported = importSteamGames(games);
                    int total = games.size();
                    javafx.application.Platform.runLater(() ->
                        showInfoAlert("Steam scan", "Found " + total + " game(s), imported " + imported + " new."));
                })));
        // Lutris
        children.add(buildIntegrationRow("Lutris",
                "Largest Linux gaming platform – scan installed Lutris games",
                "lutris", false, null,
                "Scan & Import", () -> Thread.ofVirtual().start(() -> {
                    var games = im.scanLutrisLibrary();
                    int imported = importLutrisGames(games);
                    javafx.application.Platform.runLater(() ->
                        showInfoAlert("Lutris scan", games.isEmpty()
                            ? "No Lutris games found (is Lutris installed?)"
                            : "Found " + games.size() + " game(s), imported " + imported + " new."));
                })));
        // ProtonDB
        children.add(buildIntegrationRow("ProtonDB",
                "Compatibility ratings & community reviews (public API, no key)",
                "protondb", true, null, null, null));
        // Note: SteamGridDB works automatically via bundled key (no UI needed)
        // IGDB
        children.add(buildIntegrationRow("IGDB",
                "Game info & ratings – needs Twitch ClientID:ClientSecret",
                "igdb", true, "ClientID:Secret", null, null));
    }

    private javafx.scene.layout.VBox buildIntegrationRow(String title, String subtitle,
            String key, boolean withToggle, String keyHint,
            String actionLabel, Runnable action) {
        var im = com.corkytux.launcher.modules.IntegrationsManager.getInstance();
        var box = new javafx.scene.layout.VBox(4);
        box.getStyleClass().add("modern-input-box");
        box.setStyle("-fx-background-radius:8; -fx-padding:10 14;");

        var topRow = new javafx.scene.layout.HBox(8);
        topRow.setAlignment(Pos.CENTER_LEFT);
        var titleLabel = new Label(title);
        titleLabel.getStyleClass().add("label");
        titleLabel.setStyle("-fx-font-weight:bold; -fx-font-size:13;");
        titleLabel.setMaxWidth(Double.MAX_VALUE);
        javafx.scene.layout.HBox.setHgrow(titleLabel, javafx.scene.layout.Priority.ALWAYS);
        topRow.getChildren().add(titleLabel);

        if (withToggle) {
            var sw = new com.corkytux.launcher.ui.SwitchComponent("");
            sw.setSelectedSilent(im.isEnabled(key));
            sw.setOnToggle(() -> im.setEnabled(key, sw.isSelected()));
            topRow.getChildren().add(sw);
        }
        if (actionLabel != null && action != null) {
            var btn = new Button(actionLabel);
            btn.getStyleClass().add("btn-primary");
            btn.setStyle("-fx-font-size:11px; -fx-padding:4 12;");
            btn.setOnAction(e -> action.run());
            topRow.getChildren().add(btn);
        }
        box.getChildren().add(topRow);

        var sub = new Label(subtitle);
        sub.getStyleClass().add("label-secondary");
        sub.setStyle("-fx-font-size:11px;");
        sub.setWrapText(true);
        box.getChildren().add(sub);

        if (keyHint != null) {
            var field = new javafx.scene.control.TextField();
            field.setPromptText(keyHint);
            String saved = im.getKey(key);
            if (saved != null) field.setText(saved);
            field.getStyleClass().add("text-input");
            field.setStyle("-fx-font-size:11px; -fx-background-radius:6; -fx-prompt-text-fill:#6C757D;");
            field.focusedProperty().addListener((o, was, isNow) -> {
                if (!isNow) im.setKey(key, field.getText().trim());
            });
            field.setOnAction(e -> im.setKey(key, field.getText().trim()));
            box.getChildren().add(field);
        }
        return box;
    }

    /**
     * Dark/Light picker buttons: Dark = black bg + white text + white border,
     * Light = white bg + black text + black border. Selected gets thicker border.
     */
    private static String themeBtnStyle(boolean selected, boolean isLightButton) {
        String borderW = selected ? "3" : "1.5";
        if (isLightButton) {
            return "-fx-background-color:#ffffff; -fx-text-fill:#000000;"
                    + "-fx-font-weight:bold; -fx-background-radius:8;"
                    + "-fx-border-color:#000000; -fx-border-width:" + borderW + "; -fx-border-radius:8;";
        }
        return "-fx-background-color:#000000; -fx-text-fill:#ffffff;"
                + "-fx-font-weight:bold; -fx-background-radius:8;"
                + "-fx-border-color:#ffffff; -fx-border-width:" + borderW + "; -fx-border-radius:8;";
    }

    /** Style for Dark/Light selector buttons; selected uses accent color. */
    private static String themeBtnStyle(boolean selected) {
        String accent = AccentColorManager.getInstance().getPrimary();
        if (selected) {
            return "-fx-background-color:" + accent + "; -fx-text-fill:#ffffff;"
                    + "-fx-font-weight:bold; -fx-background-radius:8;";
        }
        return "-fx-background-color:#282828; -fx-text-fill:#ffffff;"
                + "-fx-background-radius:8;";
    }

    private void showInfoAlert(String title, String msg) {        try {
            var a = new javafx.scene.control.Alert(javafx.scene.control.Alert.AlertType.INFORMATION);
            a.setTitle(title);
            a.setHeaderText(null);
            a.setContentText(msg);
            a.showAndWait();
        } catch (Exception e) {
            LOG.debug("alert failed", e);
        }
    }

    private Object getMainForm() {
        try {
            return com.corkytux.launcher.Launcher.getFormController("MainForm");
        } catch (Exception e) {
            return null;
        }
    }

    /** Finds Lutris artwork (coverart/banners) across all known Lutris data dirs. */
    private static java.nio.file.Path findLutrisArtwork(String artDir, String slug) {
        for (var dir : com.corkytux.launcher.modules.IntegrationsManager.lutrisDataDirs()) {
            var candidate = dir.resolve(artDir + "/" + slug + ".jpg");
            if (java.nio.file.Files.isRegularFile(candidate)) return candidate;
        }
        // Fallback: unresolved path in default dir (caller checks isRegularFile)
        return com.corkytux.launcher.modules.IntegrationsManager.lutrisDataDir().resolve(artDir + "/" + slug + ".jpg");
    }

    /** Converts "Five Nights At Freddy's" → "five-nights-at-freddy-s" style slug. */    private static String toSlug(String name) {
        if (name == null) return "";
        return name.toLowerCase(java.util.Locale.ROOT)
                .replace("'", "-")
                .replaceAll("[^a-z0-9]+", "-")
                .replaceAll("^-|-$", "");
    }

    /**
     * Validates an extracted icon: exists, PNG magic, dimensions >= 16px.
     * Rejects raw .ico files. 16px real icons beat coverart for icon slot.
     */
    private static boolean isLoadablePng(String path) {
        try {
            var p = java.nio.file.Path.of(path);
            if (!java.nio.file.Files.isRegularFile(p)) return false;
            byte[] header = new byte[26];
            try (var in = java.nio.file.Files.newInputStream(p)) {
                if (in.read(header) < 26) return false;
            }
            // PNG: 89 50 4E 47 0D 0A 1A 0A
            if (!(header[0] == (byte) 0x89 && header[1] == 0x50
                    && header[2] == 0x4E && header[3] == 0x47)) return false;
            // IHDR width (bytes 16-19) x height (20-23), big-endian
            int w = ((header[16] & 0xFF) << 24) | ((header[17] & 0xFF) << 16)
                    | ((header[18] & 0xFF) << 8) | (header[19] & 0xFF);
            int h = ((header[20] & 0xFF) << 24) | ((header[21] & 0xFF) << 16)
                    | ((header[22] & 0xFF) << 8) | (header[23] & 0xFF);
            return w >= 16 && h >= 16;
        } catch (Exception ignored) {
            return false;
        }
    }

    /**
     * Ensures stored icon path has .png extension (copy if needed).
     * Eliminates extensionless-file loading quirks. Returns png path or original.
     */
    private static String ensurePngExtension(String path) {
        try {
            if (path == null || path.endsWith(".png")) return path;
            var src = java.nio.file.Path.of(path);
            if (!java.nio.file.Files.isRegularFile(src)) return path;
            var dest = java.nio.file.Path.of(path + ".png");
            if (!java.nio.file.Files.isRegularFile(dest)) {
                java.nio.file.Files.copy(src, dest,
                        java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            }
            return dest.toString();
        } catch (Exception ignored) {
            return path;
        }
    }

        /** Finds main .exe in dir: name-matching first, else largest .exe, else null. */

    /** Returns true if image at path is portrait (h > w*1.2) – i.e. a cover, not an icon. */
    private static boolean isPortraitImage(String path) {
        try {
            var p = java.nio.file.Path.of(path);
            if (!java.nio.file.Files.isRegularFile(p)) return false;
            byte[] header = new byte[26];
            try (var in = java.nio.file.Files.newInputStream(p)) {
                if (in.read(header) < 26) return false;
            }
            boolean isPng = header[0] == (byte) 0x89 && header[1] == 0x50
                    && header[2] == 0x4E && header[3] == 0x47;
            boolean isJpg = header[0] == (byte) 0xFF && header[1] == (byte) 0xD8;
            if (!isPng && !isJpg) return false;
            if (isPng) {
                int w = ((header[16] & 0xFF) << 24) | ((header[17] & 0xFF) << 16)
                        | ((header[18] & 0xFF) << 8) | (header[19] & 0xFF);
                int h = ((header[20] & 0xFF) << 24) | ((header[21] & 0xFF) << 16)
                        | ((header[22] & 0xFF) << 8) | (header[23] & 0xFF);
                return h > w * 1.2;
            }
            // JPEG: scan for SOF0/SOF2 marker to get dimensions
            try (var in = java.nio.file.Files.newInputStream(p)) {
                var data = in.readAllBytes();
                for (int i = 2; i < data.length - 9; i++) {
                    if ((data[i] & 0xFF) == 0xFF && ((data[i + 1] & 0xFF) == 0xC0 || (data[i + 1] & 0xFF) == 0xC2)) {
                        int h = ((data[i + 5] & 0xFF) << 8) | (data[i + 6] & 0xFF);
                        int w = ((data[i + 7] & 0xFF) << 8) | (data[i + 8] & 0xFF);
                        return h > w * 1.2;
                    }
                }
            }
            return false;
        } catch (Exception ignored) {
            return false;
        }
    }    private static String findMainExe(String dir, String gameName) {
        try {
            var dirPath = java.nio.file.Path.of(dir);
            if (!java.nio.file.Files.isDirectory(dirPath)) return null;
            try (var stream = java.nio.file.Files.list(dirPath)) {
                var exes = stream.filter(p -> p.toString().toLowerCase().endsWith(".exe"))
                        .toList();
                if (exes.isEmpty()) return null;
                // Prefer exe matching game name (slug-ish compare)
                String slug = toSlug(gameName).replace("-", "");
                for (var e : exes) {
                    String fn = e.getFileName().toString().toLowerCase().replaceAll("[^a-z0-9]", "");
                    if (!slug.isEmpty() && (fn.contains(slug) || slug.contains(fn.replace(".exe", "")))) {
                        return e.toString();
                    }
                }
                // Else largest .exe
                return exes.stream()
                        .max(java.util.Comparator.comparingLong(p -> {
                            try { return java.nio.file.Files.size(p); } catch (Exception ignored) { return 0; }
                        }))
                        .map(Object::toString).orElse(null);
            }
        } catch (Exception ignored) {
            return null;
        }
    }

    private int importSteamGames(java.util.List<com.corkytux.launcher.modules.IntegrationsManager.SteamGame> games) {
        int count = 0;
        Object mf = getMainForm();
        if (!(mf instanceof com.corkytux.launcher.forms.MainForm main)) return 0;
        var im = com.corkytux.launcher.modules.IntegrationsManager.getInstance();
        for (var g : games) {
            // Skip Steam tools/runtimes (not games): Proton, redistributables, SDKs...
            String lname = g.name() == null ? "" : g.name().toLowerCase(java.util.Locale.ROOT);
            if (lname.contains("proton") || lname.contains("steamworks")
                    || lname.contains("steam linux runtime") || lname.contains("redistributable")
                    || lname.contains(" runtime") || lname.endsWith("runtime")
                    || lname.contains(" sdk") || lname.endsWith("sdk")
                    || lname.contains("dedicated server")) {
                LOG.debug("Skipping Steam tool: {}", g.name());
                continue;
            }
            var data = new java.util.LinkedHashMap<String, String>();
            data.put("steamID", g.appId());
            if (!g.libraryPath().isBlank()) {
                data.put("mainPath", g.libraryPath());
                data.put("executable", g.libraryPath());
            }
            data.put("source", "steam");
            // Free artwork via Steam CDN (banner + icon)
            try {
                var art = im.fetchSteamArtwork(g.appId());
                if (art.containsKey("banner")) data.put("banner", art.get("banner"));
                if (art.containsKey("icon")) data.put("icon", art.get("icon"));
            } catch (Exception e) {
                LOG.debug("artwork failed for {}", g.name(), e);
            }
            try {
                if (main.importExternalGame(g.name(), data)) count++;
            } catch (Exception e) {
                LOG.debug("Steam import failed for {}", g.name(), e);
            }
        }
        return count;
    }

    private int importLutrisGames(java.util.List<com.corkytux.launcher.modules.IntegrationsManager.LutrisGame> games) {
        int count = 0;
        Object mf = getMainForm();
        if (!(mf instanceof com.corkytux.launcher.forms.MainForm main)) return 0;
        String home = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
        // Backfill real icons for lutris games using coverart as icon
        // Priority: Lutris hicolor 128px > exe-extracted
        try {
            var app = com.corkytux.launcher.modules.AppModule.getInstance();
            // slug lookup from pga.db
            var slugByName = new java.util.HashMap<String, String>();
            try {
                var db = com.corkytux.launcher.modules.IntegrationsManager.lutrisDataDir().resolve("pga.db");
                if (java.nio.file.Files.isRegularFile(db)) {
                    var pb2 = new ProcessBuilder("sqlite3", db.toString(),
                            "SELECT slug,name FROM games WHERE installed=1;");
                    pb2.redirectErrorStream(true);
                    var proc2 = pb2.start();
                    String out2 = new String(proc2.getInputStream().readAllBytes());
                    proc2.waitFor();
                    for (String line : out2.split("\n")) {
                        String[] cols = line.split("\\|", 2);
                        if (cols.length == 2 && !cols[1].isBlank()) slugByName.put(cols[1].trim(), cols[0].trim());
                    }
                }
            } catch (Exception e) {
                LOG.debug("backfill slug lookup failed", e);
            }
            for (var entry : app.getGamesToArray().entrySet()) {
                String n = entry.getKey();
                var vals = entry.getValue();
                if (!"lutris".equals(vals.get("source"))) continue;
                String icon = vals.get("icon");
                boolean iconIsCover = icon != null && (icon.contains("/coverart/") || icon.contains("/banners/")
                        || isPortraitImage(icon));
                if (icon == null || icon.isBlank() || iconIsCover) {
                    // 1) hicolor first
                    String slug = slugByName.getOrDefault(n, toSlug(n));
                    var hicolor = com.corkytux.launcher.modules.IntegrationsManager.hicolorAppsDir().resolve("lutris_" + slug + ".png");
                    if (java.nio.file.Files.isRegularFile(hicolor)) {
                        app.setGame("icon", hicolor.toString(), n);
                        continue;
                    }
                    // 2) exe-extracted
                    String exe = vals.get("executable");
                    if (exe == null || !exe.toLowerCase().endsWith(".exe")
                            || !java.nio.file.Files.isRegularFile(java.nio.file.Path.of(exe))) {
                        continue;
                    }
                    try {
                        String realIcon = com.corkytux.launcher.modules.FixParser.parseIcon(exe);
                        if (realIcon != null && isLoadablePng(realIcon)) {
                            app.setGame("icon", ensurePngExtension(realIcon), n);
                        } else if (iconIsCover) {
                            // Corrupt extraction left bad path? ensure coverart fallback
                            LOG.debug("backfill icon invalid for {}, keeping coverart", n);
                        }
                    } catch (Exception e) {
                        LOG.debug("backfill icon failed for {}", n, e);
                    }
                }
            }
        } catch (Exception e) {
            LOG.debug("exe icon backfill failed", e);
        }
        // Backfill banner artwork for lutris games missing it entirely
        // (slug looked up from pga.db by exact name match)
        try {
            var app = com.corkytux.launcher.modules.AppModule.getInstance();
            var slugByName = new java.util.HashMap<String, String>();
            try {
                var db = com.corkytux.launcher.modules.IntegrationsManager.lutrisDataDir().resolve("pga.db");
                if (java.nio.file.Files.isRegularFile(db)) {
                    var pb = new ProcessBuilder("sqlite3", db.toString(),
                            "SELECT slug,name FROM games WHERE installed=1;");
                    pb.redirectErrorStream(true);
                    var proc = pb.start();
                    String out = new String(proc.getInputStream().readAllBytes());
                    proc.waitFor();
                    for (String line : out.split("\n")) {
                        String[] cols = line.split("\\|", 2);
                        if (cols.length == 2 && !cols[1].isBlank()) slugByName.put(cols[1].trim(), cols[0].trim());
                    }
                }
            } catch (Exception e) {
                LOG.debug("slug lookup failed", e);
            }
            for (var entry : app.getGamesToArray().entrySet()) {
                String n = entry.getKey();
                var vals = entry.getValue();
                if (!"lutris".equals(vals.get("source"))) continue;
                if (vals.get("banner") != null && !vals.get("banner").isBlank()) continue;
                String slug = slugByName.getOrDefault(n, toSlug(n));
                for (String dir : new String[]{"coverart", "banners"}) {
                    var art = findLutrisArtwork(dir, slug);
                    if (java.nio.file.Files.isRegularFile(art)) {
                        app.setGame("banner", art.toString(), n);
                        app.setGame("icon", art.toString(), n);
                        break;
                    }
                }
            }
        } catch (Exception e) {
            LOG.debug("Lutris artwork backfill failed", e);
        }
        for (var g : games) {
            if (g.name() == null || g.name().isBlank() || g.name().matches("\\d+")) {
                LOG.debug("Skipping Lutris entry with invalid name: '{}'", g.name());
                continue;
            }
            var data = new java.util.LinkedHashMap<String, String>();
            if (!g.runner().isBlank()) data.put("lutrisRunner", g.runner());
            String dir = g.directory();
            String exe = g.executable();
            // No ~/Games guessing: paths come only from Lutris (pga.db + game YAML).
            // If Lutris has no path, user configures it manually in Game Settings.
            if (dir != null && !dir.isBlank()) data.put("mainPath", dir);
            if ((exe == null || exe.isBlank()) && dir != null && !dir.isBlank()) {
                exe = findMainExe(dir, g.name()); // search dir for main .exe
            }
            if (exe != null && !exe.isBlank()) data.put("executable", exe);
            if (g.prefix() != null && !g.prefix().isBlank()) data.put("prefixPath", g.prefix());
            // Playtime: Lutris hours → CorkyTux seconds
            if (g.playtimeHours() > 0) {
                data.put("timeSpent", String.valueOf((long) (g.playtimeHours() * 3600)));
            }
            // Icon priority: 1) Lutris hicolor 128px, 2) exe-extracted, 3) coverart fallback
            // 1) Lutris system icon (proper square game icon)
            try {
                if (g.slug() != null && !g.slug().isBlank()) {
                    var hicolor = com.corkytux.launcher.modules.IntegrationsManager.hicolorAppsDir().resolve("lutris_" + g.slug() + ".png");
                    if (java.nio.file.Files.isRegularFile(hicolor)) {
                        data.put("icon", hicolor.toString());
                    }
                }
            } catch (Exception e) {
                LOG.debug("hicolor icon failed for {}", g.name(), e);
            }
            // 2) Real game icon: extract from .exe for wine games (only if no hicolor)
            // Validated: must be loadable PNG >=16px, else keep coverart
            try {
                if (!data.containsKey("icon")) {
                    String exeForIcon = data.get("executable");
                    if (exeForIcon != null && exeForIcon.toLowerCase().endsWith(".exe")
                            && java.nio.file.Files.isRegularFile(java.nio.file.Path.of(exeForIcon))) {
                        String realIcon = com.corkytux.launcher.modules.FixParser.parseIcon(exeForIcon);
                        if (realIcon != null && isLoadablePng(realIcon)) {
                            data.put("icon", ensurePngExtension(realIcon));
                        } else {
                            LOG.debug("exe icon invalid for {}, keeping coverart", g.name());
                        }
                    }
                }
            } catch (Exception e) {
                LOG.debug("exe icon extract failed for {}", g.name(), e);
            }
            // Lutris local artwork: copy coverart/{slug}.jpg into CorkyTux dirs
            // (copy, not reference – survives Lutris uninstall)
            if (g.slug() != null && !g.slug().isBlank()) {
                for (String artDir : new String[]{"coverart", "banners"}) {
                    var src = findLutrisArtwork(artDir, g.slug());
                    if (java.nio.file.Files.isRegularFile(src)) {
                        try {
                            String safe = g.slug().replaceAll("[^a-z0-9]", "_") + ".jpg";
                            var destB = java.nio.file.Path.of(home, ".config", "CorkyTux", "banners", safe);
                            var destI = java.nio.file.Path.of(home, ".config", "CorkyTux", "icons", safe);
                            if (!data.containsKey("banner")) {
                                java.nio.file.Files.createDirectories(destB.getParent());
                                java.nio.file.Files.copy(src, destB,
                                        java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                                data.put("banner", destB.toString());
                            }
                            if (!data.containsKey("icon")) {
                                java.nio.file.Files.createDirectories(destI.getParent());
                                java.nio.file.Files.copy(src, destI,
                                        java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                                data.put("icon", destI.toString());
                            }
                        } catch (Exception e) {
                            LOG.debug("artwork copy failed for {}", g.slug(), e);
                        }
                        break;
                    }
                }
            }
            data.put("source", "lutris");
            try {
                if (main.importExternalGame(g.name(), data)) count++;
            } catch (Exception e) {
                LOG.debug("Lutris import failed for {}", g.name(), e);
            }
        }
        return count;
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
                label.getStyleClass().add("label"); // themed (white dark / #212529 light)

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
    private void handlePluginsButtonAction(javafx.event.ActionEvent e) { switchPage(plugins); }
    @FXML
    private void handleAboutButtonAction(javafx.event.ActionEvent e) { switchPage(about); }

    @FXML
    private void handleVisualsButtonAction(javafx.event.ActionEvent e) { switchPage(visuals); }

    @FXML
    private void handleIntegrationsButtonAction(javafx.event.ActionEvent e) {
        if (integrations != null) switchPage(integrations);
    }

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
        // (integrations lives inside integrationsScroll wrapper)
        Node effectivePage = (newPage == integrations && integrationsScroll != null) ? integrationsScroll : newPage;
        for (Node p : new Node[]{paths, protons, launcher, plugins, visuals, about, integrationsScroll}) {
            if (p == null) continue;
            if (p == effectivePage) {
                p.setVisible(true);
                p.setManaged(true);
            } else {
                p.setVisible(false);
                p.setManaged(false);
            }
        }
        // Sync tab buttons: exactly one selected (fixes double-highlight)
        syncTabButtons(effectivePage);

        settingsModule.switchPage(newPage);
    }

    /** Marks only the tab button matching the visible page as selected. */
    private void syncTabButtons(Node visiblePage) {
        setTabSelected(pathsButton, visiblePage == paths);
        setTabSelected(protonsButton, visiblePage == protons);
        setTabSelected(launcherButton, visiblePage == launcher);
        setTabSelected(pluginsButton, visiblePage == plugins);
        setTabSelected(visualsButton, visiblePage == visuals);
        setTabSelected(aboutButton, visiblePage == about);
        setTabSelected(integrationsButton, visiblePage == integrationsScroll);
    }

    private static void setTabSelected(ToggleButton btn, boolean selected) {
        if (btn == null) return;
        // setSelected doesn't fire onAction, safe for syncing visual state
        btn.setSelected(selected);
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

    private void initTabIcons() {
        setTabIcon(visualsButton, ".data/img/palette.png");
        setTabIcon(pathsButton, ".data/img/fileview.png");
        setTabIcon(protonsButton, ".data/img/proton17.png");
        setTabIcon(launcherButton, ".data/img/settings.png");
        setTabIcon(pluginsButton, ".data/img/plugins.png");
        setTabIcon(integrationsButton, ".data/img/openIn.png");
        setTabIcon(aboutButton, ".data/img/about.png");
        // About section image
        if (image != null) {
            try {
                var is = getClass().getResourceAsStream("/.data/img/corkytux-about.png");
                if (is != null) {
                    image.setImage(new Image(is));
                }
            } catch (Exception e) {
                LOG.debug("Failed to load about image", e);
            }
        }
    }

    private void setTabIcon(ToggleButton btn, String resource) {
        if (btn == null) return;
        try {
            String themed = com.corkytux.launcher.modules.ThemedIcons.pathFor(
                    resource.startsWith("/") ? resource : "/" + resource);
            var is = getClass().getResourceAsStream(themed);
            if (is == null) {
                is = getClass().getResourceAsStream("/img/" + java.nio.file.Path.of(resource).getFileName());
            }
            if (is != null) {
                var img = new Image(is);
                var iv = new ImageView(img);
                iv.setFitWidth(20);
                iv.setFitHeight(20);
                iv.setPreserveRatio(true);
                iv.getProperties().put("themedIconBase",
                        resource.startsWith("/") ? resource : "/" + resource);
                btn.setGraphic(iv);
                btn.setContentDisplay(javafx.scene.control.ContentDisplay.TOP);
            }
        } catch (Exception e) {
            LOG.debug("Failed to load tab icon: {}", resource);
        }
    }

    private ImageView imageView(String resource, int size) {
        // Themed variant first (dark icon on light theme), then legacy fallbacks
        Image themed = com.corkytux.launcher.modules.ThemedIcons.load(resource);
        if (themed != null) {
            var tiv = new ImageView(themed);
            tiv.setFitWidth(size); tiv.setFitHeight(size); tiv.setPreserveRatio(true);
            tiv.getProperties().put("themedIconBase", resource);
            return tiv;
        }
        try (var is = getClass().getResourceAsStream(resource)) {
            if (is == null) {
                try (var alt = getClass().getResourceAsStream("/img/" + Path.of(resource).getFileName())) {
                    if (alt == null) return null;
                    var img = new Image(alt);
                    var iv = new ImageView(img);
                    iv.setFitWidth(size); iv.setFitHeight(size); iv.setPreserveRatio(true);
                    iv.getProperties().put("themedIconBase", resource);
                    return iv;
                }
            }
            var img = new Image(is);
            var iv = new ImageView(img);
            iv.setFitWidth(size); iv.setFitHeight(size); iv.setPreserveRatio(true);
            iv.getProperties().put("themedIconBase", resource);
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

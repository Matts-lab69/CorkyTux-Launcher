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

import com.corkytux.launcher.Launcher;
import com.corkytux.launcher.modules.AppModule;
import com.corkytux.launcher.modules.FilesWorker;
import com.corkytux.launcher.modules.Localization;
import javafx.animation.FadeTransition;
import javafx.animation.ScaleTransition;
import javafx.application.Platform;
import javafx.collections.ObservableList;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.geometry.Insets;
import javafx.scene.Node;
import javafx.scene.control.*;
import javafx.scene.effect.DropShadow;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.input.MouseEvent;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.layout.*;
import javafx.scene.paint.Color;
import javafx.scene.shape.Rectangle;
import javafx.stage.DirectoryChooser;
import javafx.stage.FileChooser;
import javafx.stage.Stage;
import javafx.util.Duration;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.ResourceBundle;
import java.util.concurrent.CompletableFuture;
import java.util.regex.Pattern;

/**
 * Java 25 / JavaFX 21 port of {@code MainForm.php} (735 lines).
 *
 * <p>Main UI form: game list (FlowPane inside ScrollPane), side detail panel
 * ({@code gamePanel}), launch / stop controls, Proton / wineserver handling,
 * desktop-entry creation, utilities menu, external links, and per-game time tracking.</p>
 *
 * <p>FXML: {@code /fxml/MainForm.fxml} – controller set via {@code fx:controller}.
 * All {@code @event} handlers from PHP are mapped to {@code @FXML} methods or
 * programmatic listeners in {@link #initialize}.</p>
 */
public class MainForm implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(MainForm.class);

    // -----------------------------------------------------------------------
    // FXML injected nodes – ids must match MainForm.fxml
    // -----------------------------------------------------------------------

    @FXML private ScrollPane container;
    @FXML private VBox containerContent;
    @FXML private FlowPane flowContent;
    @FXML private VBox noGamesHeader;
    @FXML private Button aboutButton;
    @FXML private Button addGameButton;
    @FXML private Button btnAddGameTop;
    @FXML private Button addGameButtonCenter;
    @FXML private VBox gamePanel;
    @FXML private ImageView gameHeader;
    @FXML private Label gameTitle;
    @FXML private Button playButton;
    @FXML private Label timeLabel;
    @FXML private Button gameDebugButton;
    @FXML private Button gameSettingsButton;
    @FXML private Button gameDeleteButton;
    @FXML private Button gameMenuButton;
    @FXML private Label favoriteStarLabel;
    @FXML private Button utilitiesButton;
    @FXML private Button runInPrefixButton;
    @FXML private Button gameFolderButton;
    @FXML private Button protonDBButton;
    @FXML private Button steamButton;
    @FXML private Button steamDBButton;
    @FXML private Label gameSize;
    @FXML private Label gameInstallPath;
    @FXML private Label gamePrefixPath;
    @FXML private VBox gameList;
    @FXML private Button filterAll;
    @FXML private Button filterFavorites;
    @FXML private Button filterAZ;
    @FXML private Button filterMostPlayed;
    @FXML private Button filterRecent;
    private String activeFilter = "all";
    // Master list of all games in insertion order (never filtered, only appended)
    private final java.util.List<String> masterGameList = new java.util.ArrayList<>();
    private final java.util.Map<String, String> masterGameImage = new java.util.HashMap<>();
    private final java.util.Map<String, String> masterGameIcon = new java.util.HashMap<>();
    @FXML private VBox recentlyPlayedSection;
    @FXML private FlowPane recentlyPlayedFlow;
    @FXML private VBox allGamesSection;
    @FXML private TextField searchField; // removed from FXML, kept for compatibility
    @FXML private TextField librarySearch;
    @FXML private ImageView librarySearchIcon;
    
    // Modal overlay fields
    @FXML private BorderPane mainBorderPane;
    @FXML private VBox modalOverlay;
    @FXML private VBox modalContent;
    @FXML private HBox modalHeader;
    @FXML private Label modalTitle;
    @FXML private Button modalCloseButton;
    @FXML private StackPane modalBody;
    private Object currentModalController;

    // -----------------------------------------------------------------------
    // State
    // -----------------------------------------------------------------------

    private String currentGameName;
    private Node currentOpenerNode;
    private Image currentBannerImage;

    private Image playImage;
    private Image stopImage;
    private Image waitImage;

    private Image addImage;
    private ProgressIndicator loadingIndicator;

    private ContextMenu utilitiesMenu;
    private ContextMenu gameFolderMenu;

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();

    // -----------------------------------------------------------------------
    // Records
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code addStubGame()} return shape {@code ['box'=>Node, 'gameName'=>Label, 'status'=>Label]}.
     */
    public record StubGame(Pane box, Label gameNameLabel, Label statusLabel) {}

    // -----------------------------------------------------------------------
    // Initializable
    // -----------------------------------------------------------------------

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        LOG.debug("MainForm initialize");

        // --- container FlowPane setup – mirrors doContainerConstruct ---
        setupContainerPane();

        // --- fullscreen preference – mirrors doConstruct fullscreen ---
        applyFullscreenPreference();

        // --- graphics setup for play / add buttons – mirrors doPlayButtonConstruct / doAddGameConstruct ---
        initPlayButtonGraphics();
        initAddGameButtonGraphics();
        initStaticButtonGraphics();

        // --- localized texts – mirrors doNoGamesHeaderConstruct etc. ---
        applyLocalizations();

        // --- wire actions ---
        wireActions();

        // --- populate game list – mirrors doConstruct loop ---
        populateGameList();

        // --- keyboard: Esc hides panel – mirrors doKeyUpEsc ---
        installGlobalKeyHandlers();

        // Fix: Immediately ensure network-independent buttons are not gray after init.
        // Offline: Run/Utilities/Debug must be enabled (play yellow) even if proton fetch failed.
        Platform.runLater(() -> {
            if (playButton != null) {
                playButton.setDisable(false);
                playButton.setOpacity(1.0);
                ensurePlayButtonYellow(playButton);
            }
            if (gameDebugButton != null) {
                gameDebugButton.setDisable(false);
                gameDebugButton.setOpacity(1.0);
            }
            if (utilitiesButton != null) { utilitiesButton.setDisable(false); utilitiesButton.setOpacity(1.0); }
            if (runInPrefixButton != null) { runInPrefixButton.setDisable(false); runInPrefixButton.setOpacity(1.0); }
        });

        // request focus
        Platform.runLater(() -> {
            if (container != null) container.requestFocus();
            else if (container != null) container.requestFocus();
        });
    }

    // -----------------------------------------------------------------------
    // Construction helpers – mirror PHP @event construct handlers
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code doContainerConstruct}:
     * <pre>
     * $fPane = new UXFlowPane;
     * $fPane->hgap = $fPane->vgap = $fPane->paddingTop = ... =35;
     * $e->sender->content = $fPane;
     * </pre>
     */
    private void setupContainerPane() {
        if (container == null) {
            LOG.warn("ScrollPane container not injected – creating fallback");
            return;
        }
        FlowPane fp;
        // Prefer injected flowContent (FXML now provides fx:id flowContent) – ensures fx:id matches controller field
        if (flowContent != null) {
            fp = flowContent;
            LOG.debug("setupContainerPane: using injected flowContent via FXML fx:id");
        } else if (containerContent != null) {
            // containerContent holds flowContent as child – try to find it
            var found = containerContent.lookup("#flowContent");
            if (found instanceof FlowPane f) fp = f;
            else if (containerContent.getChildren().stream().filter(n -> n instanceof FlowPane).findFirst().orElse(null) instanceof FlowPane f2) fp = f2;
            else {
                fp = new FlowPane();
                containerContent.getChildren().add(fp);
            }
            flowContent = fp;
        } else if (container.getContent() instanceof FlowPane existing) {
            fp = existing;
            flowContent = fp;
        } else if (container.getContent() instanceof VBox ap) {
            containerContent = ap;
            // Search inside ap for FlowPane with id flowContent
            var inner = ap.lookup("#flowContent");
            if (inner instanceof FlowPane f) fp = f;
            else {
                fp = new FlowPane();
                ap.getChildren().add(fp);
                flowContent = fp;
            }
        } else {
            fp = new FlowPane();
            container.setContent(fp);
            flowContent = fp;
        }
        fp.setHgap(35);
        fp.setVgap(35);
        fp.setPadding(new Insets(35));
        fp.setId("flowContent");
        LOG.debug("setupContainerPane: FlowPane ensured hgap=vgap=padding=35 id=flowContent");
    }

    private void applyFullscreenPreference() {
        // Mirrors PHP {@code if ($this->appModule()->launcher->get('fullscreen','User Settings')) $this->fullScreen=true;}
        var val = appModule.getLauncher("fullscreen", "User Settings");
        boolean fullscreen = "true".equalsIgnoreCase(val) || "1".equals(val);
        if (fullscreen) {
            Platform.runLater(() -> {
                Stage stage = stageOf(container != null ? container : container);
                if (stage != null) stage.setFullScreen(true);
            });
        }
    }

    /**
     * Mirrors PHP {@code doPlayButtonConstruct}:
     * <pre>
     * $e->sender->graphic = new UXImageArea(new UXImage('res://.data/img/play.png'));
     * $stopGraphic = new UXImageArea(...stop.png); $waitGraphic = ...wait.png;
     * $e->sender->graphic->size = $stopGraphic->size = $waitGraphic->size = [20,20];
     * $e->sender->data('play',$e->sender->graphic);
     * $e->sender->data('stop',$stopGraphic); $e->sender->data('wait',$waitGraphic);
     * $e->sender->text = _('MAINFORM.PLAY');
     * </pre>
     * Data handling: graphic ImageViews stored via Node#getProperties() under keys "play","stop","wait"
     * (mirrors UXNode data()), with size 20x20 and theme textFill white.
     */
    private void initPlayButtonGraphics() {
        if (playButton == null) return;
        playImage = loadImage("/img/play.png", 20, 20);
        stopImage = loadImage("/img/stop.png", 20, 20);
        waitImage = loadImage("/img/wait.png", 20, 20);

        var playView = playImage != null ? createSizedImageView(playImage, 20) : null;
        var stopView = stopImage != null ? createSizedImageView(stopImage, 20) : null;
        var waitView = waitImage != null ? createSizedImageView(waitImage, 20) : null;

        playButton.setGraphic(playView);
        playButton.getProperties().put("play", playView);
        playButton.getProperties().put("stop", stopView);
        playButton.getProperties().put("wait", waitView);
        // keep Data userData Map for parity
        if (playView != null && stopView != null && waitView != null) {
            playButton.setUserData(Map.of("play", playView, "stop", stopView, "wait", waitView));
        }
        // Fix: Ensure play (Run) button is never left gray due to network/dependency errors.
        // Offline: play must stay neon yellow (#FFEB3B) when enabled, not #808080 gray.
        // Network checks must not disable play – only game running state does.
        playButton.setDisable(false);
        playButton.setOpacity(1.0);
        // Ensure style class retains jfx-button for neon yellow theme (not disabled gray)
        if (!playButton.getStyleClass().contains("jfx-button")) {
            playButton.getStyleClass().add("jfx-button");
        }
    }

    private ImageView createSizedImageView(Image img, int size) {
        var iv = new ImageView(img);
        iv.setFitWidth(size);
        iv.setFitHeight(size);
        iv.setPreserveRatio(true);
        iv.setSmooth(true);
        return iv;
    }

    /**
     * Mirrors PHP {@code doAddGameConstruct}:
     * <pre>
     * $loadingGraphic = new UXMaterialProgressIndicator; $loadingGraphic->size=[20,20];
     * $e->sender->graphic = new UXImageArea(new UXImage('res://.data/img/add.png')); size [20,20];
     * $e->sender->data('add',$e->sender->graphic); $e->sender->data('loading',$loadingGraphic);
     * $e->sender->text = _('MAINFORM.ADDGAME');
     * </pre>
     */
     private void initAddGameButtonGraphics() {
         if (addGameButton == null) return;
         addImage = loadImage("/img/add.png", 20, 20);
         loadingIndicator = new ProgressIndicator();
         loadingIndicator.setPrefSize(20, 20);
         loadingIndicator.setMaxSize(20, 20);
         // Use a text-based "+" label so the accent color CSS can tint it
         var addLabel = new Label("+");
         addLabel.getStyleClass().add("accent-text");
         addLabel.setStyle("-fx-font-size:16; -fx-font-weight:bold;");
         var addView = addLabel;
         addGameButton.setGraphic(addView);
         // data('add') must be same instance as graphic for equality check in doAddGameClick
         addGameButton.getProperties().put("add", addView);
         addGameButton.getProperties().put("loading", loadingIndicator);
         // keep userData Map for parity
         addGameButton.setUserData(Map.of("add", addView != null ? addView : new ImageView(), "loading", loadingIndicator));
     }

    private void initStaticButtonGraphics() {
        setButtonGraphic(aboutButton, "/img/settings-hires.png", 20);
        setButtonGraphic(gameDebugButton, "/img/debug.png", 15);
        setButtonGraphic(gameSettingsButton, "/img/settings.png", 15);
        setButtonGraphic(gameDeleteButton, "/img/remove.png", 15);
        setButtonGraphic(utilitiesButton, "/img/wine.png", 15);
        setButtonGraphic(protonDBButton, "/img/protondb.png", 15);
        setButtonGraphic(steamButton, "/img/steam.png", 15);
        setButtonGraphic(steamDBButton, "/img/db.png", 15);
        setButtonGraphic(gameFolderButton, "/img/folder.png", 15);
        setButtonGraphic(runInPrefixButton, "/img/run.png", 15);
        if (librarySearchIcon != null) {
            com.corkytux.launcher.modules.ThemedIcons.applyTo(librarySearchIcon, "/img/search.png");
        }
        // Fix: Ensure network-dependent buttons are never gray due to failed internet check.
        // Run (third-party exe) and Utilities must work offline – no internet required.
        // Debug must be enabled when game idle (play yellow). Force enable after graphics set.
        for (var btn : List.of(utilitiesButton, runInPrefixButton, gameDebugButton, gameSettingsButton, gameDeleteButton, gameFolderButton)) {
            if (btn != null) {
                btn.setDisable(false);
                btn.setOpacity(1.0);
                // Ensure jfx-menu-button gray (#808080) only when intentionally disabled; offline must not disable.
                // Keep style classes for dark theme; explicitly re-enable.
            }
        }
        if (gameDebugButton != null) {
            gameDebugButton.setDisable(false);
            // Must not be gray when play is in 'play' state – will be managed by switchPlayButton, but init as enabled.
        }
    }

    private void setButtonGraphic(Button btn, String resource, int size) {
        if (btn == null) return;
        var img = com.corkytux.launcher.modules.ThemedIcons.load(resource);
        if (img == null) img = loadImage(resource, size, size);
        if (img != null) {
            var iv = new ImageView(img);
            iv.setFitWidth(size);
            iv.setFitHeight(size);
            iv.getProperties().put("themedIconBase", resource);
            btn.setGraphic(iv);
        }
    }

    private Image loadImage(String resource, int w, int h) {
        try (var is = getClass().getResourceAsStream(resource)) {
            if (is == null) {
                // try alternate path
                try (var alt = getClass().getResourceAsStream("/img/" + Path.of(resource).getFileName())) {
                    if (alt == null) return null;
                    var img = new Image(alt);
                    return img;
                }
            }
            var img = new Image(is);
            return img;
        } catch (Exception e) {
            LOG.debug("loadImage failed {}", resource, e);
            return null;
        }
    }

    private void applyLocalizations() {
        if (playButton != null) playButton.setText(loc.get("MAINFORM.PLAY"));
        if (aboutButton != null) aboutButton.setText(loc.get("MAINFORM.SETTINGS"));
        if (gameDebugButton != null) {
            gameDebugButton.setText(loc.get("MAINFORM.MENU.RUNDEBUG"));
            gameDebugButton.setTooltip(new Tooltip(loc.get("MAINFORM.MENU.RUNDEBUG.TOOLTIP")));
        }
        if (gameSettingsButton != null) gameSettingsButton.setText(loc.get("MAINFORM.MENU.SETTINGS"));
        if (gameDeleteButton != null) gameDeleteButton.setText(loc.get("REMOVE"));
        if (utilitiesButton != null) utilitiesButton.setText(loc.get("MAINFORM.MENU.UTILITIES"));
        if (gameFolderButton != null) gameFolderButton.setText(loc.get("GAMESETTINGS.FOLDERS.BUTTON"));
        if (runInPrefixButton != null) {
            runInPrefixButton.setText(loc.get("MAINFORM.MENU.RUN"));
            runInPrefixButton.setTooltip(new Tooltip(loc.get("MAINFORM.MENU.RUN.TOOLTIP")));
        }
    }

    private void wireActions() {
        if (addGameButton != null) addGameButton.setOnMouseClicked(this::handleAddGameClick);
        if (btnAddGameTop != null) {
            btnAddGameTop.setText(loc.get("MAINFORM.ADDGAME"));
            btnAddGameTop.setOnMouseClicked(this::handleAddGameClick);
        }
        // Category filters – single selection
        if (filterAll != null) filterAll.setOnAction(e -> setActiveFilter("all"));
        if (filterFavorites != null) filterFavorites.setOnAction(e -> setActiveFilter("favorites"));
        if (filterAZ != null) filterAZ.setOnAction(e -> setActiveFilter("az"));
        if (filterMostPlayed != null) filterMostPlayed.setOnAction(e -> setActiveFilter("mostplayed"));
        if (filterRecent != null) filterRecent.setOnAction(e -> setActiveFilter("recent"));
        updateFilterStyles();
        // Library search – live filter by name
        if (librarySearch != null) {
            librarySearch.textProperty().addListener((obs, oldV, newV) -> applySearchFilter(newV));
        }
        if (addGameButtonCenter != null) addGameButtonCenter.setOnAction(this::handleAboutAction);
        if (playButton != null) playButton.setOnAction(this::handlePlayButtonAction);
        if (aboutButton != null) aboutButton.setOnAction(this::handleAboutAction);
        if (modalCloseButton != null) modalCloseButton.setOnAction(e -> hideModal());
        if (modalOverlay != null) {
            modalOverlay.setOnMouseClicked(e -> {
                if (e.getTarget() == modalOverlay) hideModal();
            });
        }
        if (gameDebugButton != null) gameDebugButton.setOnAction(this::handleGameDebugAction);
        if (gameMenuButton != null) gameMenuButton.setOnAction(e -> toggleFavorite());
        if (gameSettingsButton != null) gameSettingsButton.setOnAction(this::handleGameSettingsAction);
        if (gameDeleteButton != null) gameDeleteButton.setOnAction(this::handleGameDeleteAction);
        if (utilitiesButton != null) {
            buildUtilitiesMenu();
            utilitiesButton.setOnMouseClicked(e -> {
                if (utilitiesMenu != null) utilitiesMenu.show(utilitiesButton, e.getScreenX(), e.getScreenY());
            });
        }
        if (protonDBButton != null) protonDBButton.setOnAction(this::handleProtonDBAction);
        if (steamButton != null) steamButton.setOnAction(this::handleSteamAction);
        if (steamDBButton != null) steamDBButton.setOnAction(this::handleSteamDBAction);
        if (gameFolderButton != null) {
            buildGameFolderMenu();
            gameFolderButton.setOnMouseClicked(e -> {
                if (gameFolderMenu != null) gameFolderMenu.show(gameFolderButton, e.getScreenX(), e.getScreenY());
            });
        }
        if (runInPrefixButton != null) runInPrefixButton.setOnAction(this::handleRunInPrefixAction);

        // Close handler – mirrors doClose (delete /tmp/ofllpid)
        Platform.runLater(() -> {
            Stage stage = stageOf(container != null ? container : container);
            if (stage != null) {
                stage.setOnCloseRequest(e -> {
                    try { Files.deleteIfExists(Path.of("/tmp/ofllpid")); }
                    catch (IOException ex) { LOG.warn("Failed to delete /tmp/ofllpid", ex); }
                });
            }
        });
    }

    /**
     * Mirrors PHP {@code doConstruct}:
     * <pre>
     * if (launcher.get('fullscreen','User Settings')) this.fullScreen=true;
     * foreach (games->toArray() as $name=>$params) addGame(name, executable, overrides, banner, icon);
     * requestFocus();
     * </pre>
     * Reads Games.ini (~/.config/CorkyTux/Games.ini) via AppModule, populates FlowPane
     * with tiles whose covers are resolved from ~/.config/CorkyTux/banners/*.jpg
     * (via addGame -> resolveImage -> banners fallback). Handles noGamesHeader
     * visibility 1:1 with container children emptiness, and ensures newGameConfigurator
     * does NOT auto-open when games exist.
     */
    private void populateGameList() {
        doConstruct();
    }

    /**
     * Explicit doConstruct parity – called from initialize.
     */
    public void doConstruct() {
        var sections = collectGameSections();
        boolean hasGames = !sections.isEmpty();

        // noGamesHeader vs container logic 1:1 – mirror PHP: header visible iff container empty
        if (noGamesHeader != null) {
            // Defer to FX thread but set immediately if possible; Platform.runLater for safety
            boolean visible = !hasGames;
            if (javafx.application.Platform.isFxApplicationThread()) noGamesHeader.setVisible(visible);
            else Platform.runLater(() -> noGamesHeader.setVisible(visible));
        }

        for (var entry : sections.entrySet()) {
            var name = entry.getKey();
            var params = entry.getValue();
            String banner = params.get("banner");
            // If banner not set but file exists in banners dir, use that – mirrors covers from banners/*.jpg
            if ((banner == null || banner.isBlank() || !Files.isRegularFile(Path.of(banner)))) {
                String bannersDir = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/.config/CorkyTux/banners/";
                // Try by game name
                String bannerFromDir = bannersDir + name + ".jpg";
                if (Files.isRegularFile(Path.of(bannerFromDir))) banner = bannerFromDir;
                else {
                    String pngTry = bannersDir + name + ".png";
                    if (Files.isRegularFile(Path.of(pngTry))) banner = pngTry;
                }
                // Fallback: try by steamID (AppID) – banner files are saved as {appId}.jpg
                if (banner == null || !Files.isRegularFile(Path.of(banner))) {
                    String steamId = params.get("steamID");
                    if (steamId != null && !steamId.isBlank()) {
                        String byId = bannersDir + steamId + ".jpg";
                        if (Files.isRegularFile(Path.of(byId))) banner = byId;
                        else {
                            String byIdPng = bannersDir + steamId + ".png";
                            if (Files.isRegularFile(Path.of(byIdPng))) banner = byIdPng;
                        }
                    }
                }
            }
            addGame(name,
                    params.get("executable"),
                    params.get("overrides"),
                    banner,
                    params.get("icon"));
        }

        // Ensure newGameConfigurator does NOT auto-open when games exist – PHP doConstruct never did.
        // Previous buggy versions may have auto-opened configurator when empty; we keep empty => header shows, no auto-open.
        // Explicit: do NOT call showForm('newGameConfigurator') here.

        // Fix: initial panel appearing at startup when it shouldn't (MainForm shows gamePanel for a game on start,
        // should only show games list, panel should appear when clicking a game) – fix MainForm.doConstruct to not auto-select game.
        // Java 25 / JavaFX 21: Ensure gamePanel hidden initially, no auto-selection of first game, only games list visible.
        // Mirrors PHP doConstruct which never called showGameMenu – previous bug left FXML visible=true or auto-selected first entry.
        // Keep full detail: do NOT call showGameMenu for any game here; panel appears only on tile click -> showGameMenu(name, header, opener).
        Runnable hidePanelTask = () -> {
            if (gamePanel != null) {
                gamePanel.setVisible(false);
                gamePanel.setOpacity(0);
                gamePanel.toBack();
                gamePanel.getProperties().remove("gameName");
                gamePanel.getProperties().remove("opener");
            }
            currentGameName = null;
            currentOpenerNode = null;
            currentBannerImage = null;
            // Ensure container and buttons are fully visible and enabled at startup (list view, not detail)
            var flow = flowContent != null ? flowContent : findFlowPane();
            Node containerNode = flow != null ? flow : (container != null ? container : container);
            if (containerNode != null) {
                containerNode.setVisible(true);
                containerNode.setOpacity(1.0);
                containerNode.setDisable(false);
            }

            if (addGameButton != null) {
                addGameButton.setVisible(true);
                addGameButton.setOpacity(1.0);
                addGameButton.setDisable(false);
            }
        };
        if (javafx.application.Platform.isFxApplicationThread()) hidePanelTask.run();
        else Platform.runLater(hidePanelTask);

        // requestFocus mirrors PHP $this->requestFocus();
        Platform.runLater(() -> {
            if (container != null) container.requestFocus();
            else if (container != null) container.requestFocus();
        });
    }

    private Map<String, Map<String, String>> collectGameSections() {
        // Correctly reads Games.ini via AppModule – per task requirement
        try {
            return appModule.getGamesToArray();
        } catch (Exception e) {
            LOG.warn("collectGameSections via AppModule failed, fallback to direct parse", e);
            var path = Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".config/CorkyTux/Games.ini");
            var result = new java.util.LinkedHashMap<String, Map<String, String>>();
            if (!Files.isRegularFile(path)) return result;
            try {
                var wini = new org.ini4j.Wini(path.toFile());
                for (String section : wini.keySet()) {
                    var sec = wini.get(section);
                    if (sec == null) continue;
                    result.put(section, Map.copyOf(sec));
                }
            } catch (Exception ex) {
                LOG.warn("Failed to collect game sections fallback", ex);
            }
            return result;
        }
    }

    private void installGlobalKeyHandlers() {
        Node target = container != null ? container : container;
        if (target == null) return;
        target.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
            if (e.getCode() == KeyCode.ESCAPE) handleEscKey();
        });
        // mouseDown outside gamePanel hides it – mirrors on('mouseDown')
        target.addEventFilter(MouseEvent.MOUSE_PRESSED, e -> {
            if (gamePanel != null && gamePanel.isVisible() && e.getX() < gamePanel.getLayoutX()) {
                handleEscKey();
            }
        });
    }

    // -----------------------------------------------------------------------
    // Event handlers – direct PHP mirrors
    // -----------------------------------------------------------------------

    @FXML
    private void handleDesktopIconClick() {
        String desktop = execReadFully("xdg-user-dir DESKTOP");
        if (desktop == null || desktop.isBlank()) desktop = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/Desktop";
        boolean selected = createDesktopFile(desktop.trim());
    }

    @FXML
    private void handleMenuIconClick() {
        String appMenu = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/.local/share/applications";
        boolean selected = createDesktopFile(appMenu);
    }

    /**
     * Mirrors PHP {@code createDesktopFile($path)}.
     * Toggles {@code $gameName.desktop} – deletes if exists, otherwise creates via FilesWorker.
     */
    public boolean createDesktopFile(String directory) {
        if (currentGameName == null || currentGameName.isBlank()) {
            LOG.warn("createDesktopFile: no current game selected");
            return false;
        }
        var desktopPath = Path.of(directory, currentGameName + ".desktop");
        if (Files.isRegularFile(desktopPath)) {
            try { Files.delete(desktopPath); } catch (IOException ex) { LOG.warn("Failed to delete {}", desktopPath, ex); }
            return false;
        } else {
            String icon = appModule.getGame("icon", currentGameName);
            String content = FilesWorker.generateDesktopEntry(currentGameName, icon);
            try {
                if (desktopPath.getParent() != null) Files.createDirectories(desktopPath.getParent());
                Files.writeString(desktopPath, content, StandardCharsets.UTF_8);
                // chmod +x
                new ProcessBuilder("chmod", "+x", desktopPath.toString()).start();
            } catch (IOException ex) {
                LOG.error("Failed to create desktop entry {}", desktopPath, ex);
                return false;
            }
            return true;
        }
    }

    /**
     * Mirrors PHP {@code doPlayButtonAction}:
     * <pre>
     * if (sender.graphic == sender.data('stop')){
     *   wineserver=FilesWorker::getProtonExecutable(gameName,'wineserver',true);
     *   prefixDir=FilesWorker::getProtonPrefixPath(gameName,'wine');
     *   try{$kill=new Process([$wineserver,'-k'],null,['WINEPREFIX'=>prefixDir])->startAndWait();}
     *   catch(Throwable $ex){$kill=new Process(['pkill','-f',fs::name(executable)])->startAndWait();}
     *   if($kill->getExitValue()!=0) toast(KILLFAILED); else switchPlayButton('play');
     * } else runGame(gameName);
     * </pre>
     */
    @FXML
    private void handlePlayButtonAction(javafx.event.ActionEvent e) {
        if (isStopGraphicActive()) {
            String gameName = currentGameName != null ? currentGameName
                    : (gamePanel != null ? (String) gamePanel.getProperties().get("gameName") : null);
            if (gameName == null) {
                LOG.warn("doPlayButtonAction: no gameName");
                return;
            }
            String wineserver = FilesWorker.getProtonExecutable(gameName, "wineserver", true);
            String prefixDir = FilesWorker.getProtonPrefixPath(gameName, "wine");
            Process kill = null;
            int exit = -1;
            try {
                if (wineserver != null) {
                    var pb = new ProcessBuilder(wineserver, "-k");
                    pb.environment().put("WINEPREFIX", prefixDir);
                    kill = pb.start();
                    exit = kill.waitFor();
                } else throw new IOException("wineserver not found");
            } catch (Throwable ex) {
                LOG.debug("wineserver -k failed, fallback pkill", ex);
                String exe = appModule.getGame("executable", gameName);
                String name = exe != null ? Path.of(exe).getFileName().toString() : gameName;
                try { kill = new ProcessBuilder("pkill", "-f", name).start(); exit = kill.waitFor(); }
                catch (Exception ex2) { LOG.debug("pkill fallback failed", ex2); exit = -1; }
            }
            if (exit != 0) toast(loc.get("MAINFORM.KILLFAILED"));
            else switchPlayButton("play");
        } else {
            String gameName = currentGameName != null ? currentGameName
                    : (gamePanel != null ? (String) gamePanel.getProperties().get("gameName") : null);
            if (gameName != null) runGame(gameName);
            else LOG.warn("doPlayButtonAction: no game to run");
        }
    }

    /**
     * Mirrors PHP {@code $e->sender->graphic == $e->sender->data('stop')} reference equality.
     * Uses stored Node references via getProperties("stop") for exact parity.
     */
    private boolean isStopGraphicActive() {
        if (playButton == null || playButton.getGraphic() == null) return false;
        Object stop = playButton.getProperties().get("stop");
        if (stop instanceof Node n) return playButton.getGraphic() == n;
        // Fallback via userData map or image identity
        if (playButton.getGraphic().getUserData() != null) return "stop".equals(playButton.getGraphic().getUserData());
        if (playButton.getGraphic() instanceof ImageView iv && iv.getImage() == stopImage) return true;
        return false;
    }

    @FXML
    private void handleAboutAction(javafx.event.ActionEvent e) {
        showModal("launcherSettings");
    }

    @FXML
    private void handleGameDebugAction(javafx.event.ActionEvent e) {
        // Fix: Debug mode previously gated behind internet check (fetchLatestProton error
        // left gameDebugButton disabled gray). Debug is local WINEDEBUG – no network needed.
        // Ensure button is enabled when idle and launches correctly offline.
        if (currentGameName == null || currentGameName.isBlank()) {
            String panelName = gamePanel != null ? (String) gamePanel.getProperties().get("gameName") : null;
            if (panelName != null) currentGameName = panelName;
            else {
                LOG.warn("Debug: no current game selected");
                toast("No game selected");
                return;
            }
        }
        // Ensure debug button not gray before dialog – if play is yellow, debug must be enabled
        if (gameDebugButton != null) {
            // Do not allow debug to be invoked while game already running (stop state)
            Object waitGraphic = playButton != null ? playButton.getProperties().get("wait") : null;
            boolean isWait = playButton != null && waitGraphic instanceof Node n && playButton.getGraphic() == n;
            Object stopGraphic = playButton != null ? playButton.getProperties().get("stop") : null;
            boolean isStop = playButton != null && stopGraphic instanceof Node n2 && playButton.getGraphic() == n2;
            if (isStop || isWait) {
                LOG.warn("Debug requested while game running – ignored");
                return;
            }
            gameDebugButton.setDisable(false);
            gameDebugButton.setOpacity(1.0);
        }
        TextInputDialog dialog = new TextInputDialog("+err,+warn,+seh");
        dialog.setTitle("WINEDEBUG");
        dialog.setHeaderText("WINEDEBUG:");
        // Style dialog to stay dark – ensure not hidden offline
        var result = dialog.showAndWait();
        result.ifPresent(debug -> {
            if (debug == null || debug.isBlank()) {
                // Empty still means run with default +err,+warn,+seh – treat blank as default
                if (debug != null && debug.isBlank()) debug = "+err,+warn,+seh";
                else return;
            }
            // Launch with debug flag – offline safe, no internet check
            runGame(currentGameName, debug);
        });
        // After dialog, ensure debug button state follows switchPlayButton (stop vs play)
        // but never leave gray due to network error
        if (gameDebugButton != null) gameDebugButton.setOpacity(1.0);
    }

    @FXML
    private void handleGameSettingsAction(javafx.event.ActionEvent e) {
        // Open as modal overlay (same as launcherSettings)
        showModal("gameSettings");
        if (currentModalController instanceof GameSettings gs) {
            gs.setGameName(currentGameName);
            if (currentBannerImage != null) gs.setBannerImage(currentBannerImage);
            if (currentOpenerNode instanceof Pane p) {
                var iconImg = findIconImageInNode(p);
                if (iconImg != null) gs.setGameIconImage(iconImg);
            }
        } else {
            LOG.warn("gameSettings modal controller not available");
        }
    }

    private Image findIconImageInNode(Pane pane) {
        try {
            // shallow search for ImageView
            for (Node n : pane.getChildren()) {
                if (n instanceof ImageView iv && iv.getImage() != null) return iv.getImage();
                if (n instanceof Pane inner) {
                    var r = findIconImageInNode(inner);
                    if (r != null) return r;
                }
            }
        } catch (Exception ex) { LOG.debug("findIconImage failed", ex); }
        return null;
    }

    @FXML
    private void handleGameDeleteAction(javafx.event.ActionEvent e) {
        showModal("gameRemover");
        if (currentModalController instanceof com.corkytux.launcher.forms.GameRemover gr) {
            try { gr.handleShow(); } catch (Exception ex) { LOG.debug("GameRemover handleShow failed", ex); }
        }
    }

    @FXML
    private void handleProtonDBAction(javafx.event.ActionEvent e) {
        String steamId = appModule.getGame("steamID", currentGameName);
        if (steamId != null) openUrl("https://protondb.com/app/" + steamId);
    }

    @FXML
    private void handleSteamAction(javafx.event.ActionEvent e) {
        String steamId = appModule.getGame("steamID", currentGameName);
        if (steamId != null) openUrl("https://store.steampowered.com/app/" + steamId);
    }

    @FXML
    private void handleSteamDBAction(javafx.event.ActionEvent e) {
        String steamId = appModule.getGame("steamID", currentGameName);
        if (steamId != null) openUrl("https://steamdb.info/app/" + steamId);
    }

    @FXML
    private void handleRunInPrefixAction(javafx.event.ActionEvent e) {
        // Fix: Run (third-party exe) must NOT require internet. Previous buggy port gated it
        // behind network check (fetchLatestProton / noInternet.png) – offline showed as if no internet.
        // Correct: only requires local Proton + prefix, no HttpClient. Gracefully handle missing deps.
        String proton = null;
        String prefixDir = null;
        try {
            proton = FilesWorker.getProtonExecutable(currentGameName);
            prefixDir = FilesWorker.getProtonPrefixPath(currentGameName);
        } catch (Exception ex) {
            LOG.warn("Run: failed to resolve proton/prefix offline", ex);
            // Fallback: try skipping network-dependent Latest lookup, use local newest proton
            try {
                proton = FilesWorker.getProtonExecutable(currentGameName, "proton", true);
                prefixDir = FilesWorker.getProtonPrefixPath(currentGameName);
            } catch (Exception ignored) {}
        }
        // Ensure button never stays disabled due to exception – network failure must not gray it.
        if (runInPrefixButton != null) {
            runInPrefixButton.setDisable(false);
            runInPrefixButton.setOpacity(1.0);
        }
        if (proton == null || !Files.isRegularFile(Path.of(proton))) {
            toast(loc.get("FILESWORKER.PROTON.NOTFOUND"));
            LOG.warn("RunInPrefix: proton not found for {} (offline safe)", currentGameName);
            return;
        }
        FileChooser fc = new FileChooser();
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter(loc.get("FILECHOOSER.EXE.DESC"), "*.exe"));
        fc.setTitle(loc.get("FILECHOOSER.EXE.TITLE"));
        var file = fc.showOpenDialog(stageOf(playButton != null ? playButton : runInPrefixButton));
        if (file == null) return;
        var pb = new ProcessBuilder(proton, "run", file.getAbsolutePath());
        if (file.getParentFile() != null) pb.directory(file.getParentFile());
        pb.environment().put("STEAM_COMPAT_DATA_PATH", prefixDir);
        pb.environment().put("STEAM_COMPAT_CLIENT_INSTALL_PATH", com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/.steam/steam");
        try {
            var proc = pb.start();
            // Non-blocking hook – Run must work offline, no internet needed.
            FilesWorker.hookProcessOuts(proc, false, false);
            LOG.info("RunInPrefix started {} via {} prefix={}", file.getAbsolutePath(), proton, prefixDir);
        } catch (IOException ex) {
            LOG.error("runInPrefix failed (missing dep? {} )", ex.getMessage(), ex);
            toast("Failed to run: " + ex.getMessage());
            // Do not leave button disabled – re-enable for retry offline.
            if (runInPrefixButton != null) runInPrefixButton.setDisable(false);
        } finally {
            // Ensure never gray after attempt
            if (runInPrefixButton != null) Platform.runLater(() -> {
                runInPrefixButton.setDisable(false);
                runInPrefixButton.setOpacity(1.0);
            });
        }
    }

    /**
     * Mirrors PHP {@code doAddGameClick}:
     * <pre>
     * if(addGame->graphic == addGame->data('loading') && graphic!=null) return;
     * dc = new UXDirectoryChooser; dc->title=_('ADDGAME.FILECHOOSER');
     * path = dc->showDialog(visible?this:form('MainForm')); if(null) return;
     * try{switchGameButton('loading');}catch(Throwable){}
     * new Thread(fn(){
     *   files=scanDir(path);
     *   uiLater(fn(){
     *     try{switchGameButton('add','addGame');}catch(Throwable){}
     *     form=quUI::showFormAndFocus('newGameConfigurator',true);
     *     uiLater(fn(){form->prepareForGame(files,path);});
     *   });
     * })->start();
     * </pre>
     */
    @FXML
    private void handleAddGameClick(MouseEvent e) {
        Object loadingData = addGameButton != null ? addGameButton.getProperties().get("loading") : null;
        if (addGameButton != null && addGameButton.getGraphic() != null && addGameButton.getGraphic() == loadingData) return;
        DirectoryChooser dc = new DirectoryChooser();
        dc.setTitle(loc.get("ADDGAME.FILECHOOSER"));
        Stage owner = stageOf(addGameButton);
        if (owner == null) owner = stageOf(container);
        var path = dc.showDialog(owner);
        if (path == null) return;

        try { switchGameButton("loading"); } catch (Exception ex) { LOG.debug("switch loading failed", ex); }

        Thread.ofVirtual().start(() -> {
            var files = scanDir(path.toPath());
            LOG.info("scanDir {} returned {} files", path, files.size());
            Platform.runLater(() -> {
                try { switchGameButton("add"); } catch (Exception ex) { LOG.debug("switch add failed", ex); }
                showModal("newGameConfigurator");
                if (currentModalController instanceof NewGameConfigurator ngc) {
                    ngc.prepareForGame(files, path.toPath().toString());
                    LOG.info("newGameConfigurator shown in modal with {} files", files.size());
                }
            });
        });
    }

    private void handleEscKey() {
        if (gamePanel != null && gamePanel.isVisible()) hideGameMenu();
    }

    // -----------------------------------------------------------------------
    // Core business logic – mirrors PHP helpers
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code switchGameButton($status)}:
     * <pre>
     * if ($status=='add'){ $addGame->graphic=$addGame->data('add'); $addGame->text=_('MAINFORM.ADDGAME');}
     * else { $addGame->graphic=$addGame->data('loading'); $addGame->text=_('MAINFORM.LOADINGGAME');}
     * </pre>
     * Theme: uses same graphic instances stored via getProperties("add"/"loading") for reference equality.
     */
    public void switchGameButton(String status) {
        if (addGameButton == null) return;
        Runnable r = () -> {
            if ("add".equals(status)) {
                Object add = addGameButton.getProperties().get("add");
                if (add instanceof Node n) addGameButton.setGraphic(n);
                else if (addImage != null) addGameButton.setGraphic(createSizedImageView(addImage, 20));
                addGameButton.setText(loc.get("MAINFORM.ADDGAME"));
            } else {
                Object loading = addGameButton.getProperties().get("loading");
                if (loading instanceof Node n) addGameButton.setGraphic(n);
                else addGameButton.setGraphic(loadingIndicator);
                addGameButton.setText(loc.get("MAINFORM.LOADINGGAME"));
            }
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Mirrors PHP {@code static function scanDir($path)}.
     * Returns files matching {@code .(exe|vbs|bat|rar)} – excludes directories.
     * PHP uses Regex::match('\.(exe|vbs|bat|rar)$',$f) with File::isDirectory guard.
     */
    public static List<Path> scanDir(Path path) {
        var pattern = Pattern.compile("\\.(exe|vbs|bat|rar)$", Pattern.CASE_INSENSITIVE);
        var result = new java.util.ArrayList<Path>();
        if (!Files.isDirectory(path)) return result;
        try (var walk = Files.walk(path)) {
            walk.filter(Files::isRegularFile)
                .filter(p -> pattern.matcher(p.getFileName().toString()).find())
                .forEach(result::add);
        } catch (IOException e) {
            LOG.warn("scanDir failed for {}", path, e);
        }
        return result;
    }

    /**
     * Mirrors PHP {@code addGame($gameName,$exec,$overrides,$image = null,$icon = null)}:
     * <pre>
     * $gamePanel = instance('prototypes.panel');
     * $iconView->size=[34,34]; proportional=centered=stretch=true;
     * $clip->size=$gamePanel->children[1]->size; arc = borderRadius*2;
     * $gamePanel->children[3]->children[0]->text=$gameName;
     * $gamePanel->children[1]->image = fs::isFile(image)?image:noBanner;
     * $gamePanel->children[3]->children[0]->graphic=$iconView;
     * $gamePanel->on('click',fn()=>showGameMenu(gameName, children[1]->image, sender));
     * container->content->children->add(gamePanel);
     * if (noGamesHeader->visible) noGamesHeader->visible=false;
     * addBasicEffects(gamePanel);
     * </pre>
     * Theme colors: #333333 panel, #333337cd hbox, #ffffff label, #e6e6e6 status.
     * Data handling: tile stored via getProperties("gameName"), getUserData Map.
     * Covers from ~/.config/CorkyTux/banners/*.jpg handled via resolve fallback.
     */
    public void addGame(String gameName, String exec, String overrides, String image, String icon) {
        Runnable r = () -> {
            Pane tile = createGameTile(gameName, image, icon);
            // Data handling – mirrors gamePanel data but for tile we store via properties + userData
            tile.getProperties().put("gameName", gameName);
            tile.setUserData(Map.of("gameName", gameName, "exec", exec != null ? exec : "", "overrides", overrides != null ? overrides : ""));
            // click handler – mirrors on('click') -> showGameMenu($gameName,$e->sender->children[1]->image,$e->sender)
            tile.setOnMouseClicked(ev -> {
                // banner is children[1] in prototype – our tileBanner
                var img = (tile.lookup("#tileBanner") instanceof ImageView iv) ? iv.getImage()
                        : (tile.lookup("#imageAlt") instanceof ImageView iv2) ? iv2.getImage() : null;
                // fallback to currentBannerImage or resolved banner
                showGameMenu(gameName, img, tile);
            });
            var flow = flowContent != null ? flowContent : findFlowPane();
            if (flow != null) {
                flow.getChildren().add(tile);
                addBasicEffects(tile);
            } else {
                LOG.warn("No FlowPane to add game {}", gameName);
            }
            // Also add to sidebar game list (one per row below search)
            // Register in master list first (for filter rebuilds)
            if (!masterGameList.contains(gameName)) {
                masterGameList.add(gameName);
                if (image != null) masterGameImage.put(gameName, image);
                if (icon != null) masterGameIcon.put(gameName, icon);
            }
            addGameToSidebarList(gameName, image, icon);
            // noGamesHeader vs container logic 1:1 – if header visible, hide it
            if (noGamesHeader != null && noGamesHeader.isVisible()) noGamesHeader.setVisible(false);
            refreshRecentlyPlayed();
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Removes a game from master list + sidebar + center grid (after GameRemover deletion).
     */
    public void removeGameFromUI(String gameName) {
        if (gameName == null) return;
        Runnable r = () -> {
            masterGameList.remove(gameName);
            masterGameImage.remove(gameName);
            masterGameIcon.remove(gameName);
            if (gameList != null) {
                gameList.getChildren().removeIf(n ->
                    gameName.equals(n.getProperties().get("gameName")));
            }
            try {
                var flow = flowContent != null ? flowContent : findFlowPane();
                if (flow != null) {
                    flow.getChildren().removeIf(n ->
                        gameName.equals(n.getProperties().get("gameName")));
                }
            } catch (Exception e) {
                LOG.debug("removeGameFromUI center failed", e);
            }
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }
    private void addGameToSidebarList(String gameName, String imagePath, String iconPath) {
        if (gameList == null) return;
        // Rebuild via applyFilter so new game respects current sort/filter
        applyFilter();
    }

    /**
     * Sets the active library filter (single selection) and refreshes styles + list.
     */
    private void setActiveFilter(String filter) {
        this.activeFilter = filter;
        updateFilterStyles();
        applyFilter();
    }

    /**
     * Highlights the active filter button with accent-color text.
     */
    private void updateFilterStyles() {
        styleFilterButton(filterAll, "all".equals(activeFilter));
        styleFilterButton(filterFavorites, "favorites".equals(activeFilter));
        styleFilterButton(filterAZ, "az".equals(activeFilter));
        styleFilterButton(filterMostPlayed, "mostplayed".equals(activeFilter));
        styleFilterButton(filterRecent, "recent".equals(activeFilter));
    }

    private void styleFilterButton(Button btn, boolean selected) {
        if (btn == null) return;
        btn.getStyleClass().remove("filter-selected");
        if (selected && !btn.getStyleClass().contains("filter-selected")) {
            btn.getStyleClass().add("filter-selected");
        }
    }

    /**
     * Applies the active filter/sort to sidebar gameList and center grid.
     * Rebuilds sidebar from masterGameList (non-destructive).
     * - all: insertion order, no sort
     * - favorites: only games with favorite=1/true
     * - az: sort A-Z by name
     * - mostplayed: sort by timeSpent desc
     * - recent: reverse insertion order (newest first)
     */
    private void applyFilter() {
        if (gameList == null) return;
        java.util.List<String> ordered = new java.util.ArrayList<>(masterGameList);
        switch (activeFilter) {
            case "favorites" -> ordered.removeIf(n -> {
                String f = appModule.getGame("favorite", n);
                return f == null || (!f.equals("1") && !f.equalsIgnoreCase("true"));
            });
            case "az" -> ordered.sort(String.CASE_INSENSITIVE_ORDER);
            case "mostplayed" -> ordered.sort((a, b) ->
                Long.compare(getTimeSpentSeconds(b), getTimeSpentSeconds(a)));
            case "recent" -> java.util.Collections.reverse(ordered);
            default -> { /* all: keep insertion order */ }
        }
        // Rebuild sidebar rows from scratch (non-destructive)
        gameList.getChildren().clear();
        for (String n : ordered) {
            var row = buildSidebarRow(n, masterGameImage.get(n), masterGameIcon.get(n));
            if (row != null) gameList.getChildren().add(row);
        }
        // Reorder center grid tiles to match (same order), restore visibility
        try {
            var flow = flowContent != null ? flowContent : findFlowPane();
            if (flow != null) {
                var tilesByName = new java.util.HashMap<String, Node>();
                for (var child : flow.getChildren()) {
                    child.setVisible(true);
                    child.setManaged(true);
                    Object gn = child.getProperties().get("gameName");
                    if (gn instanceof String s) tilesByName.put(s, child);
                }
                var reordered = new java.util.ArrayList<Node>();
                for (String n : ordered) {
                    Node t = tilesByName.remove(n);
                    if (t != null) reordered.add(t);
                }
                reordered.addAll(tilesByName.values());
                flow.getChildren().setAll(reordered);
            }
        } catch (Exception e) {
            LOG.debug("applyFilter center reorder failed", e);
        }
    }

    /**
     * Builds a sidebar row node for the given game (icon + name).
     */
    private HBox buildSidebarRow(String gameName, String imagePath, String iconPath) {
        var row = new HBox(8);
        row.getStyleClass().add("game-list-item");
        row.setAlignment(javafx.geometry.Pos.CENTER_LEFT);
        row.setPadding(new javafx.geometry.Insets(4, 8, 4, 8));

        var iconView = new ImageView();
        iconView.setFitWidth(24);
        iconView.setFitHeight(24);
        iconView.setPreserveRatio(true);
        iconView.setSmooth(true);
        Image iconImg = resolveImage(iconPath, "/img/noImage.png");
        if (iconImg == null || iconImg.isError()) {
            try (var is = getClass().getResourceAsStream("/.data/img/noImage.png")) {
                if (is != null) iconImg = new Image(is);
            } catch (Exception ignored) {}
        }
        if (iconImg != null) iconView.setImage(iconImg);

        var nameLabel = new Label(gameName);
        nameLabel.setStyle("-fx-font-size:12;");
        nameLabel.setMaxWidth(Double.MAX_VALUE);
        HBox.setHgrow(nameLabel, javafx.scene.layout.Priority.ALWAYS);

        row.getChildren().addAll(iconView, nameLabel);
        row.setOnMouseClicked(ev -> showGameMenu(gameName, null, row));
        row.getProperties().put("gameName", gameName);
        return row;
    }

    private long getTimeSpentSeconds(String gameName) {
        try {
            String raw = appModule.getGame("timeSpent", gameName);
            if (raw != null) return Long.parseLong(raw.trim());
        } catch (Exception ignored) {}
        return 0;
    }

    /**
     * Rebuilds the RECENTLY PLAYED center section: top 5 games by lastPlayed
     * (big tiles). Hidden when nothing has been played yet.
     */
    public void refreshRecentlyPlayed() {
        if (recentlyPlayedSection == null || recentlyPlayedFlow == null) return;
        Runnable r = () -> {
            try {
                var scored = new java.util.ArrayList<Map.Entry<String, Long>>();
                for (String n : masterGameList) {
                    long lp = 0;
                    try {
                        String raw = appModule.getGame("lastPlayed", n);
                        if (raw != null) lp = Long.parseLong(raw.trim());
                    } catch (Exception ignored) {}
                    if (lp > 0) scored.add(Map.entry(n, lp));
                }
                scored.sort((a, b) -> Long.compare(b.getValue(), a.getValue()));
                recentlyPlayedFlow.getChildren().clear();
                int shown = 0;
                for (var e : scored) {
                    if (shown >= 5) break;
                    String n = e.getKey();
                    try {
                        Pane tile = createGameTile(n, masterGameImage.get(n), masterGameIcon.get(n));
                        tile.getProperties().put("gameName", n);
                        tile.setOnMouseClicked(ev -> showGameMenu(n, null, tile));
                        recentlyPlayedFlow.getChildren().add(tile);
                        addBasicEffects(tile);
                        shown++;
                    } catch (Exception ex) {
                        LOG.debug("recent tile failed for {}", n, ex);
                    }
                }
                boolean any = shown > 0;
                recentlyPlayedSection.setVisible(any);
                recentlyPlayedSection.setManaged(any);
            } catch (Exception e) {
                LOG.debug("refreshRecentlyPlayed failed", e);
            }
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Live search: shows only games whose name contains the query (case-insensitive).
     * Applies on top of the active category filter, both sidebar and center grid.
     */
    private void applySearchFilter(String query) {
        String q = query != null ? query.trim().toLowerCase() : "";
        if (q.isEmpty()) {
            applyFilter(); // restore category filter view
            return;
        }
        if (gameList == null) return;
        // Sidebar: rebuild from master list matching query
        gameList.getChildren().clear();
        var matched = new java.util.ArrayList<String>();
        for (String n : masterGameList) {
            if (n.toLowerCase().contains(q)) matched.add(n);
        }
        for (String n : matched) {
            var row = buildSidebarRow(n, masterGameImage.get(n), masterGameIcon.get(n));
            if (row != null) gameList.getChildren().add(row);
        }
        // Center grid: show only matching tiles
        try {
            var flow = flowContent != null ? flowContent : findFlowPane();
            if (flow != null) {
                for (var child : flow.getChildren()) {
                    Object gn = child.getProperties().get("gameName");
                    boolean show = gn instanceof String s && s.toLowerCase().contains(q);
                    child.setVisible(show);
                    child.setManaged(show);
                }
            }
        } catch (Exception e) {
            LOG.debug("applySearchFilter center failed", e);
        }
    }

    /**
     * Toggles favorite status for the currently shown game.
     * Star button (former gameMenuButton) in game details header.
     */
    private void toggleFavorite() {
        if (currentGameName == null || currentGameName.isBlank()) return;
        try {
            String f = appModule.getGame("favorite", currentGameName);
            boolean isFav = f != null && (f.equals("1") || f.equalsIgnoreCase("true"));
            appModule.setGame("favorite", isFav ? "0" : "1", currentGameName);
            updateFavoriteStar(currentGameName);
            // If favorites filter active, refresh list
            if ("favorites".equals(activeFilter)) applyFilter();
        } catch (Exception e) {
            LOG.warn("toggleFavorite failed for {}", currentGameName, e);
        }
    }

    /**
     * Updates star icon: ★ filled gold if favorite, ☆ outline gray if not.
     */
    private void updateFavoriteStar(String gameName) {
        if (favoriteStarLabel == null) return;
        try {
            String f = appModule.getGame("favorite", gameName);
            boolean isFav = f != null && (f.equals("1") || f.equalsIgnoreCase("true"));
            if (isFav) {
                favoriteStarLabel.setText("★");
                favoriteStarLabel.setStyle("-fx-font-size:24; -fx-text-fill:#FFD700;");
            } else {
                favoriteStarLabel.setText("☆");
                favoriteStarLabel.setStyle("-fx-font-size:24; -fx-text-fill:#a7a7a7;");
            }
        } catch (Exception ignored) {
            favoriteStarLabel.setText("☆");
        }
    }

    /**
     * Creates tile via Prototypes.createPanel for 1:1 parity with prototypes.panel FXML.
     * Delegates to Prototypes factory which already handles banner fallback (including banners dir)
     * and theme colors.
     */
    private Pane createGameTile(String gameName, String imagePath, String iconPath) {
        // Delegate to Prototypes which handles covers from ~/.config/CorkyTux/banners/*.png/.jpg
        // and icons from ~/.config/CorkyTux/icons – matches original PHP exactly.
        return Prototypes.createPanel(gameName, imagePath, iconPath);
    }

    private Image resolveImage(String path, String fallbackResource) {
        if (path != null && Files.isRegularFile(Path.of(path))) {
            try { return new Image(Path.of(path).toUri().toString()); }
            catch (Exception e) { LOG.debug("resolveImage file failed {}", path, e); }
        }
        // Also try banners dir if gameName implied (handled in createGameTile, but keep generic fallback)
        try (var is = getClass().getResourceAsStream(fallbackResource)) {
            if (is != null) return new Image(is);
        } catch (Exception e) { LOG.debug("resolveImage fallback failed {}", fallbackResource, e); }
        return null;
    }

    /**
     * Mirrors PHP {@code addStubGame()}:
     * <pre>
     * if (noGamesHeader->visible) noGamesHeader->visible=false;
     * $box = instance('prototypes.gameStubBox');
     * addBasicEffects(box);
     * container->content->children->add(box);
     * $childrens = box->children->toArray();
     * return ['box'=>box,'gameName'=>childrens[0],'status'=>childrens[2]]; // actually 0 and 2 in PHP stub
     * </pre>
     * In PHP stub gameStubBox is VBox with label4 (gameName) and label5 (status).
     * We mirror via Prototypes.createStubPanel.
     */
    /**
     * Imports an external game (Steam/Lutris scan) into the launcher.
     * Skips if a game with the same name already exists.
     * @return true if imported, false if already existed
     */
    public boolean importExternalGame(String name, Map<String, String> data) {
        if (name == null || name.isBlank()) return false;
        try {
            var existing = appModule.getGamesToArray();
            if (existing.containsKey(name)) return false;
            String defaultProton = appModule.getLauncher("defaultProton", "User Settings");
            if (defaultProton == null) defaultProton = "GE-Proton Latest";
            appModule.setGame("proton", defaultProton, name);
            if (data != null) {
                for (var e : data.entrySet()) {
                    if (e.getValue() != null && !e.getValue().isBlank()) {
                        appModule.setGame(e.getKey(), e.getValue(), name);
                    }
                }
            }
            // Show in UI
            String exec = data != null ? data.get("executable") : null;
            String overrides = data != null ? data.get("overrides") : null;
            String banner = data != null ? data.get("banner") : null;
            String icon = data != null ? data.get("icon") : null;
            addGame(name, exec, overrides, banner, icon);
            LOG.info("Imported external game: {}", name);
            return true;
        } catch (Exception e) {
            LOG.warn("importExternalGame failed for {}", name, e);
            return false;
        }
    }

    public StubGame addStubGame() {
        // noGamesHeader logic 1:1
        Runnable hideHeader = () -> { if (noGamesHeader != null && noGamesHeader.isVisible()) noGamesHeader.setVisible(false); };
        if (Platform.isFxApplicationThread()) hideHeader.run();
        else Platform.runLater(hideHeader);

        var box = Prototypes.createStubPanel(null);
        // Extract labels for return shape – mirrors childrens[0] and childrens[2] (but our VBox has 2 children)
        var children = box.getChildren();
        var nameLabel = children.size() > 0 && children.get(0) instanceof Label l ? l : new Label("...");
        var statusLabel = children.size() > 1 && children.get(1) instanceof Label l ? l : new Label(loc.get("NEWGAMECONFIG.DLLS"));
        // Ensure status text matches loc
        statusLabel.setText(loc.get("NEWGAMECONFIG.DLLS") != null ? loc.get("NEWGAMECONFIG.DLLS") : "Please wait until game added");
        statusLabel.setTextFill(Color.web("#e6e6e6"));
        nameLabel.setTextFill(Color.WHITE);

        var flow = flowContent != null ? flowContent : findFlowPane();
        if (flow != null) {
            Runnable add = () -> {
                flow.getChildren().add(box);
                addBasicEffects(box);
            };
            if (Platform.isFxApplicationThread()) add.run();
            else Platform.runLater(add);
        }
        return new StubGame(box, nameLabel, statusLabel);
    }

    /**
     * Mirrors PHP {@code removeStubGame($box)}:
     * <pre>
     * $box->free();
     * if (container->content->children->isEmpty()) noGamesHeader->visible=true;
     * </pre>
     * 1:1: free box (remove from parent) and show header iff container empty.
     */
    public void removeStubGame(Pane box) {
        Runnable r = () -> {
            var flow = flowContent != null ? flowContent : findFlowPane();
            if (flow != null) flow.getChildren().remove(box);
            else if (box.getParent() instanceof Pane p) p.getChildren().remove(box);
            // 1:1 header logic – visible true iff no children
            var flowCheck = flowContent != null ? flowContent : findFlowPane();
            boolean isEmpty = flowCheck == null || flowCheck.getChildren().isEmpty();
            if (isEmpty && noGamesHeader != null) noGamesHeader.setVisible(true);
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Implements removeGame parity for task requirement: remove tile by gameName
     * and handle noGamesHeader visibility 1:1, mirroring PHP removal in gameRemover.
     * Not in original MainForm.php but required for theme + container logic.
     */
    public void removeGame(String gameName) {
        if (gameName == null || gameName.isBlank()) return;
        Runnable r = () -> {
            var flow = flowContent != null ? flowContent : findFlowPane();
            if (flow != null) {
                var toRemove = flow.getChildren().stream()
                        .filter(n -> gameName.equals(n.getProperties().get("gameName"))
                                || gameName.equals(n.getUserData() instanceof Map<?,?> m ? m.get("gameName") : null))
                        .toList();
                flow.getChildren().removeAll(toRemove);
                if (flow.getChildren().isEmpty() && noGamesHeader != null) noGamesHeader.setVisible(true);
            }
            if (gameName.equals(currentGameName)) {
                currentGameName = null;
                currentOpenerNode = null;
                currentBannerImage = null;
                if (gamePanel != null) {
                    gamePanel.getProperties().remove("gameName");
                    gamePanel.getProperties().remove("opener");
                }
                hideGameMenu();
            }
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Mirrors PHP {@code showGameMenu($name,$header,$sender)}:
     * <pre>
     * gamePanel->show();
     * null->selected = isFile(xdg-user-dir DESKTOP+"/name.desktop");
     * null->selected = isFile(home/.local/share/applications/name.desktop);
     * on('mouseDown', fn(e){ if(e.x<gamePanel.x) doKeyUpEsc();});
     * addGame->hide();
     * gameHeader->image=header;
     * gamePanel->data('gameName',name); gamePanel->data('opener',sender);
     * protonDB/steam/steamDB enabled = games.get('steamID',name)!=null;
     * updateTimeSpent(name); switchPlayButton('stop');
     * new Thread(fn()=>waitForWineServerTerminate(name))->start();
     * if(prism.forceGPU){ background->image=sender.children[1]->image; quUI::animateWithoutConflict('FadeIn',background,1.4);}
     * Animation::fadeTo(null,450,0.5,fn()=>null.enabled=false);
     * Animation::fadeTo(container,450,0.5,fn()=>container.enabled=false);
     * quUI::animateWithoutConflict('FadeInRight',gamePanel,1.4);
     * </pre>
     */
    public void showGameMenu(String name, Image header, Node opener) {
        // Set title immediately (before runLater) so it never shows stale "Game Title"
        // even if the async block is delayed on first open.
        try {
            if (gameTitle != null && name != null) {
                if (Platform.isFxApplicationThread()) gameTitle.setText(name);
                else Platform.runLater(() -> gameTitle.setText(name));
            }
        } catch (Exception ignored) {}
        currentGameName = name;
        Platform.runLater(() -> {
            if (gamePanel == null) {
                LOG.warn("showGameMenu: gamePanel is null!");
                return;
            }
            LOG.info("showGameMenu: showing panel for game={} opener={}", name, opener != null ? opener.getClass().getSimpleName() : "null");
            gamePanel.setVisible(true);
            gamePanel.setManaged(true);
            gamePanel.toFront();
            gamePanel.setOpacity(1);
            gamePanel.setTranslateX(0);
            // gamePanel is now a StackPane overlay (not BorderPane.right) – just request layout
            gamePanel.requestLayout();
            currentGameName = name;
            currentOpenerNode = opener;
            currentBannerImage = header;

            // Toggle desktop/menu shortcuts (wrapped – must not block title/info update)
            boolean hasDesktop = false;
            boolean hasMenu = false;
            try {
                String desktop = execReadFully("xdg-user-dir DESKTOP");
                if (desktop == null || desktop.isBlank()) desktop = com.corkytux.launcher.modules.FilesWorker.getExpectedHome() + "/Desktop";
                else desktop = desktop.trim();
                hasDesktop = Files.isRegularFile(Path.of(desktop, name + ".desktop"));
                hasMenu = Files.isRegularFile(Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".local/share/applications", name + ".desktop"));
            } catch (Exception e) {
                LOG.debug("Desktop/menu shortcut check failed for {}", name, e);
            }

            // Store mouseDown handler for hide on outside click
            Node root = container != null ? container : container;
            if (root != null) {
                Object prev = root.getProperties().get("mouseDownHandler");
                if (prev instanceof javafx.event.EventHandler<?> h) root.removeEventFilter(MouseEvent.MOUSE_PRESSED, (javafx.event.EventHandler<MouseEvent>) h);
                javafx.event.EventHandler<MouseEvent> handler = e -> {
                    if (gamePanel.isVisible() && e.getX() < gamePanel.getLayoutX()) handleEscKey();
                };
                root.addEventFilter(MouseEvent.MOUSE_PRESSED, handler);
                root.getProperties().put("mouseDownHandler", handler);
                gamePanel.getProperties().put("mouseDownHandler", handler);
            }

            if (addGameButton != null) addGameButton.setVisible(false);
            if (gameTitle != null) gameTitle.setText(name);
            updateFavoriteStar(name);
            Image bannerImg = header;
            if (bannerImg == null) {
                // 1) Stored banner key from Games.ini (covers Store API / SGDB downloads)
                try {
                    String stored = appModule.getGame("banner", name);
                    if (stored != null && !stored.isBlank()
                            && java.nio.file.Files.isRegularFile(java.nio.file.Path.of(stored.trim()))) {
                        bannerImg = new Image(java.nio.file.Path.of(stored.trim()).toUri().toString());
                    }
                } catch (Exception ignored) {}
                // 2) banners/{name}.jpg fallback (for sidebar clicks)
                if (bannerImg == null) {
                    bannerImg = resolveImage(null, "/img/noBanner.png");
                    String home = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
                    for (String ext : new String[]{".jpg", ".png"}) {
                        var p = java.nio.file.Path.of(home, ".config", "CorkyTux", "banners", name + ext);
                        if (java.nio.file.Files.isRegularFile(p)) {
                            try { bannerImg = new Image(p.toUri().toString()); break; }
                            catch (Exception ignored) {}
                        }
                    }
                }
            }
            if (gameHeader != null && bannerImg != null) gameHeader.setImage(bannerImg);
            gamePanel.getProperties().put("gameName", name);
            gamePanel.getProperties().put("opener", opener);
            gamePanel.setUserData(Map.of("gameName", name, "opener", opener));

            String steamId = appModule.getGame("steamID", name);
            boolean hasSteamId = steamId != null && !steamId.isBlank();
            if (protonDBButton != null) protonDBButton.setDisable(!hasSteamId);
            if (steamButton != null) steamButton.setDisable(!hasSteamId);
            if (steamDBButton != null) steamDBButton.setDisable(!hasSteamId);

            updateTimeSpent(name);
            switchPlayButton("stop");
            if (utilitiesButton != null) { utilitiesButton.setDisable(false); utilitiesButton.setOpacity(1.0); }
            if (runInPrefixButton != null) { runInPrefixButton.setDisable(false); runInPrefixButton.setOpacity(1.0); }
            if (gameFolderButton != null) { gameFolderButton.setDisable(false); gameFolderButton.setOpacity(1.0); }
            if (gameSettingsButton != null) { gameSettingsButton.setDisable(false); gameSettingsButton.setOpacity(1.0); }
            if (gameDeleteButton != null) { gameDeleteButton.setDisable(false); gameDeleteButton.setOpacity(1.0); }

            // Fade container to 50% opacity
            var flow = flowContent != null ? flowContent : findFlowPane();
            Node containerNode = flow != null ? flow : (container != null ? container : root);
            if (containerNode != null) {
                var ft2 = new FadeTransition(Duration.millis(200), containerNode);
                ft2.setFromValue(containerNode.getOpacity()); ft2.setToValue(0.5); ft2.setOnFinished(e -> containerNode.setDisable(true)); ft2.play();
            }
            // Node cache during slide-in: rasterizes heavy ScrollPane subtree once,
            // avoiding per-frame repaints that caused intermittent lag.
            gamePanel.setCache(true);
            gamePanel.setCacheHint(javafx.scene.CacheHint.SPEED);
            Runnable uncache = () -> {
                gamePanel.setCache(false);
                gamePanel.setCacheHint(javafx.scene.CacheHint.DEFAULT);
            };
            try { com.corkytux.launcher.util.QuUI.animateWithoutConflict("FadeInRight", gamePanel, 1.6, uncache); }
            catch (Exception ex) { fadeInRight(gamePanel, 250); uncache.run(); }

            Thread.ofVirtual().start(() -> waitForWineServerTerminate(name));
        });
    }

    /**
     * Mirrors PHP {@code hideGameMenu()}:
     * <pre>
     * if(playButton.graphic == playButton.data('wait')) return;
     * off('mouseDown');
     * addGame->show();
     * container->enabled=true;
     * Animation::fadeIn(null,450,fn()=>null.enabled=true);
     * Animation::fadeIn(container,450,fn()=>container.enabled=true);
     * quUI::animateWithoutConflict('FadeOutRight',gamePanel,1.4,fn()=>gamePanel.hide());
     * if(prism.forceGPU) quUI::animateWithoutConflict('FadeOut',background,1.4);
     * </pre>
     */
    public void hideGameMenu() {
        Platform.runLater(() -> {
            // Check playButton graphic == data('wait') reference equality
            Object waitGraphic = playButton != null ? playButton.getProperties().get("wait") : null;
            if (playButton != null && waitGraphic instanceof Node n && playButton.getGraphic() == n) return;
            // Also fallback image check
            if (playButton != null && playButton.getGraphic() instanceof ImageView iv && iv.getImage() == waitImage && waitGraphic == null) return;
            if (gamePanel == null) return;
            Node root = container != null ? container : container;
            if (root != null) {
                Object h = root.getProperties().remove("mouseDownHandler");
                if (h instanceof javafx.event.EventHandler<?> eh) root.removeEventFilter(MouseEvent.MOUSE_PRESSED, (javafx.event.EventHandler<MouseEvent>) eh);
            }
            gamePanel.getProperties().remove("mouseDownHandler");
            if (addGameButton != null) addGameButton.setVisible(true); // show()

            var flow = flowContent != null ? flowContent : findFlowPane();
            Node containerNode = flow != null ? flow : container;
            if (containerNode != null) {
                containerNode.setDisable(false);
                // Animation::fadeIn container 450 -> enabled true (we already enabled, now fade)
                var ft = new FadeTransition(Duration.millis(200), containerNode);
                ft.setFromValue(containerNode.getOpacity()); ft.setToValue(1.0); ft.setOnFinished(e -> containerNode.setDisable(false)); ft.play();
            }
            // Hide panel instantly (no fade-out over heavy ScrollPane subtree – avoids lag);
            // container still fades back in smoothly below.
            try {
                Object prev = gamePanel.getProperties().get("quUIAnimation");
                if (prev instanceof javafx.animation.Transition t) t.stop();
            } catch (Exception ignored) {}
            gamePanel.setOpacity(1);
            gamePanel.setTranslateX(0);
            gamePanel.setVisible(false);
            gamePanel.setManaged(false);
        });
    }

    private void fadeIn(Node node, int millis) {
        var ft = new FadeTransition(Duration.millis(millis), node);
        ft.setFromValue(node.getOpacity());
        ft.setToValue(1.0);
        ft.play();
    }

    private void fadeOut(Node node, int millis) {
        var ft = new FadeTransition(Duration.millis(millis), node);
        ft.setFromValue(node.getOpacity());
        ft.setToValue(0.0);
        ft.play();
    }

    private void fadeTo(Node node, double to, int millis) {
        var ft = new FadeTransition(Duration.millis(millis), node);
        ft.setFromValue(node.getOpacity());
        ft.setToValue(to);
        ft.play();
    }

    private void fadeInRight(Node node, int millis) {
        node.setTranslateX(40);
        node.setOpacity(0);
        var ft = new FadeTransition(Duration.millis(millis), node);
        ft.setFromValue(0); ft.setToValue(1); ft.play();
        var tt = new javafx.animation.TranslateTransition(Duration.millis(millis), node);
        tt.setFromX(40); tt.setToX(0); tt.play();
    }

    private void fadeOutRight(Node node, int millis, Runnable onFinished) {
        var ft = new FadeTransition(Duration.millis(millis), node);
        ft.setFromValue(1); ft.setToValue(0);
        ft.setOnFinished(e -> { if (onFinished != null) onFinished.run(); });
        ft.play();
        var tt = new javafx.animation.TranslateTransition(Duration.millis(millis), node);
        tt.setFromX(0); tt.setToX(80); tt.play();
    }

    /**
     * Mirrors PHP {@code static function addBasicEffects($object)}.
     * Scale on hover + drop shadow.
     */
    public static void addBasicEffects(Node node) {
        // Hover scale
        node.setOnMouseEntered(e -> {
            var st = new ScaleTransition(Duration.millis(300), node);
            st.setToX(1.05); st.setToY(1.05); st.play();
        });
        node.setOnMouseExited(e -> {
            var st = new ScaleTransition(Duration.millis(300), node);
            st.setToX(1.0); st.setToY(1.0); st.play();
        });
        var shadow = new DropShadow();
        shadow.setColor(Color.web("#0000004d"));
        shadow.setRadius(10);
        shadow.setOffsetY(4);
        node.setEffect(shadow);
    }

    /**
     * Mirrors PHP {@code runGame($gameName,$debug=false)}.
     */
    public void runGame(String gameName) {
        runGame(gameName, null);
    }

    public void runGame(String gameName, String debug) {
        // Track last played for Recently Played section
        try {
            appModule.setGame("lastPlayed", String.valueOf(System.currentTimeMillis() / 1000), gameName);
            Platform.runLater(this::refreshRecentlyPlayed);
        } catch (Exception e) {
            LOG.debug("lastPlayed track failed", e);
        }
        Thread.ofVirtual().start(() -> {
            var installedProtons = FilesWorker.getInstalledProtons();
            if (installedProtons.isEmpty()) {
                LOG.info("No proton installed – auto-downloading latest for {}", gameName);
                Platform.runLater(() -> {
                    toast(loc.get("MAINFORM.AUTODOWNLOAD"));
                    switchPlayButton("wait");
                });
                var releases = FilesWorker.fetchProtonReleases();
                if (releases == null || releases.isEmpty()) {
                    Platform.runLater(() -> {
                        toast(loc.get("FILESWORKER.PROTON.NOTFOUND"));
                        switchPlayButton("play");
                    });
                    return;
                }
                var first = releases.entrySet().iterator().next();
                String releaseName = first.getKey();
                String releaseUrl = first.getValue().get("url");

                Platform.runLater(() -> {
                    Launcher.showForm("protonDownloader");
                    Object ctrl = Launcher.getFormController("protonDownloader");
                    if (ctrl instanceof ProtonDownloader pd) {
                        pd.startDownload(releaseName, releaseUrl);
                    }
                });

                int waited = 0;
                while (waited < 600_000) {
                    try { Thread.sleep(2000); } catch (InterruptedException e) { Thread.currentThread().interrupt(); return; }
                    Object ctrl = null;
                    try { ctrl = Launcher.getFormController("protonDownloader"); } catch (Exception ignored) {}
                    boolean busy = ctrl instanceof ProtonDownloader pd && pd.isDownloading();
                    if (!busy && waited > 4) break;
                    waited += 2000;
                }
                LOG.info("Auto-download wait finished after {}ms", waited);
                try { Thread.sleep(3000); } catch (InterruptedException e) { Thread.currentThread().interrupt(); return; }

                installedProtons = FilesWorker.getInstalledProtons();
                if (installedProtons.isEmpty()) {
                    Platform.runLater(() -> {
                        toast(loc.get("FILESWORKER.PROTON.NOTFOUND"));
                        switchPlayButton("play");
                    });
                    return;
                }
            }

            var pb = FilesWorker.generateProcess(gameName, debug != null && !debug.isBlank());
            if (pb == null) {
                Platform.runLater(() -> switchPlayButton("play"));
                return;
            }
            if (debug != null && !debug.isBlank()) {
                pb.environment().put("WINEDEBUG", debug);
            }
            Platform.runLater(() -> switchPlayButton("stop"));
            try {
                var process = pb.start();
                boolean isDebug = debug != null && !debug.isBlank();
                FilesWorker.run(process, gameName, isDebug);
                LOG.info("{} process has finished, waiting for the wineprefix to complete", gameName);
                waitForWineServerTerminate(gameName);
                LOG.info("{} fully terminated", gameName);
            } catch (IOException e) {
                LOG.error("runGame failed for {}", gameName, e);
                Platform.runLater(() -> {
                    toast("Failed to launch: " + e.getMessage());
                    switchPlayButton("play");
                });
            }
        });
    }

    /**
     * Mirrors PHP {@code waitForWineServerTerminate($gameName)}:
     * <pre>
     * $protonExec = FilesWorker::getProtonExecutable($gameName,'wineserver',true);
     * $prefixDir = FilesWorker::getProtonPrefixPath($gamePanel->data('gameName'),'wine');
     * if (isDir(prefixDir) and protonExec!=false){
     *   try{new Process([$protonExec,'-w'],null,['WINEPREFIX'=>prefixDir])->startAndWait();}catch(Throwable){}
     * }
     * uiLater(fn()=> if(gamePanel.data('gameName')==gameName){ switchPlayButton('play'); updateTimeSpent(gameName);});
     * </pre>
     */
    public void waitForWineServerTerminate(String gameName) {
        String protonExec = FilesWorker.getProtonExecutable(gameName, "wineserver", true);
        // PHP uses $this->gamePanel->data('gameName') for prefix – use the passed gameName for wine prefix path
        String prefixDir = FilesWorker.getProtonPrefixPath(gameName, "wine");
        if (prefixDir != null && Files.isDirectory(Path.of(prefixDir)) && protonExec != null) {
            try {
                var pb = new ProcessBuilder(protonExec, "-w");
                pb.environment().put("WINEPREFIX", prefixDir);
                pb.redirectOutput(ProcessBuilder.Redirect.DISCARD);
                pb.redirectError(ProcessBuilder.Redirect.DISCARD);
                pb.start().waitFor();
            } catch (Exception ignored) { }
        }
        Platform.runLater(() -> {
            String panelName = gamePanel != null ? (String) gamePanel.getProperties().get("gameName") : null;
            if (gameName.equals(panelName) || gameName.equals(currentGameName)) {
                switchPlayButton("play");
                updateTimeSpent(gameName);
            }
        });
    }

    /**
     * Mirrors PHP {@code switchPlayButton($status)}:
     * <pre>
     * switch(status){
     *   case('stop'): text='STOP'; graphic=data('stop'); gameDebugButton->enabled=false; break;
     *   case('wait'): text='WAIT'; graphic=data('wait'); gameDebugButton->enabled=playButton->enabled=false; return;
     *   default:      text='PLAY'; graphic=data('play'); gameDebugButton->enabled=true; break;
     * }
     * playButton->enabled=true;
     * </pre>
     * Data handling: graphic stored via getProperties("play"/"stop"/"wait") for reference equality.
     * Theme: textFill #ffffff, graphic size 20.
     */
    public void switchPlayButton(String status) {
        if (playButton == null) return;
        Runnable r = () -> {
            switch (status) {
                case "stop" -> {
                    playButton.setText(loc.get("MAINFORM.STOP"));
                    Object stop = playButton.getProperties().get("stop");
                    if (stop instanceof Node n) playButton.setGraphic(n);
                    else if (stopImage != null) playButton.setGraphic(createSizedImageView(stopImage, 20));
                    if (gameDebugButton != null) {
                        gameDebugButton.setDisable(true); // Debug disabled while running – gray intentionally
                        gameDebugButton.setOpacity(0.7);
                    }
                    playButton.setDisable(false);
                    playButton.setOpacity(1.0);
                    // Ensure yellow when stopped (still enabled, not gray)
                    ensurePlayButtonYellow(playButton);
                }
                case "wait" -> {
                    playButton.setText(loc.get("MAINFORM.WAIT"));
                    Object wait = playButton.getProperties().get("wait");
                    if (wait instanceof Node n) playButton.setGraphic(n);
                    else if (waitImage != null) playButton.setGraphic(createSizedImageView(waitImage, 20));
                    if (gameDebugButton != null) {
                        gameDebugButton.setDisable(true);
                        gameDebugButton.setOpacity(0.6);
                    }
                    playButton.setDisable(true);
                    playButton.setOpacity(0.9);
                    return; // mirror PHP return – do not re-enable; wait is intentional gray (#808080)
                }
                default -> {
                    playButton.setText(loc.get("MAINFORM.PLAY"));
                    Object play = playButton.getProperties().get("play");
                    if (play instanceof Node n) playButton.setGraphic(n);
                    else if (playImage != null) playButton.setGraphic(createSizedImageView(playImage, 20));
                    // Fix: Debug must NOT be gray when idle – previous bug left it disabled due to network error.
                    if (gameDebugButton != null) {
                        gameDebugButton.setDisable(false);
                        gameDebugButton.setOpacity(1.0);
                    }
                    playButton.setDisable(false);
                    playButton.setOpacity(1.0);
                    ensurePlayButtonYellow(playButton);
                    // Also ensure Run + Utilities not left gray after network failure
                    if (runInPrefixButton != null) {
                        runInPrefixButton.setDisable(false);
                        runInPrefixButton.setOpacity(1.0);
                    }
                    if (utilitiesButton != null) {
                        utilitiesButton.setDisable(false);
                        utilitiesButton.setOpacity(1.0);
                    }
                }
            }
            // PHP default break path enables playButton; wait already returned
            if (!"wait".equals(status)) {
                playButton.setDisable(false);
                playButton.setOpacity(1.0);
                ensurePlayButtonYellow(playButton);
                // Fix: Ensure after any transition (except wait), debug reflects idle state correctly
                if ("play".equals(status) && gameDebugButton != null) {
                    gameDebugButton.setDisable(false);
                    gameDebugButton.setOpacity(1.0);
                }
            }
        };
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    /**
     * Ensures play button stays neon yellow (#FFEB3B) when enabled – not gray #808080.
     * Gray is only for :disabled pseudo (CSS .jfx-button:disabled). We keep enabled opacity 1
     * and ensure style class includes jfx-button. Bright neon #FFEB3B with dark #212121 text.
     */
    private void ensurePlayButtonYellow(Button btn) {
        if (btn == null || btn.isDisabled()) return;
        btn.setOpacity(1.0);
        if (!btn.getStyleClass().contains("jfx-button")) btn.getStyleClass().add("jfx-button");
        // Force re-apply style to avoid stale gray from previous disabled pulse
        btn.applyCss();
    }

    /**
     * Mirrors PHP {@code updateTimeSpent($gameName)}.
     */
    public void updateTimeSpent(String gameName) {
        if (timeLabel == null) return;
        String raw = appModule.getGame("timeSpent", gameName);
        long seconds = 0;
        if (raw != null) try { seconds = Long.parseLong(raw.trim()); } catch (NumberFormatException ignored) {}
        String text;
        if (seconds < 3600) {
            long minutes = Math.round(seconds / 60.0);
            if (minutes < 1) minutes = 0;
            text = String.format(loc.get("MAINFORM.TIMESPENT.MINUTES"), minutes);
        } else {
            long hours = Math.round(seconds / 3600.0);
            text = String.format(loc.get("MAINFORM.TIMESPENT.HOURS"), hours);
        }
        Platform.runLater(() -> timeLabel.setText(text));
        // Also refresh install info synchronously (fixes empty info on first open)
        updateGameInfo(gameName);
    }

    /**
     * Populates game detail info: size, install path, prefix path.
     * Called from showGameMenu so info appears on first open.
     */
    private void updateGameInfo(String gameName) {
        try {
            String exec = appModule.getGame("executable", gameName);
            if (exec == null || exec.isBlank()) exec = appModule.getGame("mainPath", gameName);
            String prefix = null;
            String installPath = null;
            if (exec != null && !exec.isBlank()) {
                var execPath = java.nio.file.Path.of(exec.trim());
                if (java.nio.file.Files.isDirectory(execPath)) {
                    installPath = execPath.toString(); // Steam/Lutris dir import
                } else if (execPath.getParent() != null) {
                    installPath = execPath.getParent().toString();
                }
                // Calculate folder size async (can be slow)
                if (installPath != null) {
                    final String ip = installPath;
                    Thread.ofVirtual().start(() -> {
                        try {
                            long bytes = java.nio.file.Files.walk(java.nio.file.Path.of(ip))
                                .filter(p -> p.toFile().isFile())
                                .mapToLong(p -> p.toFile().length()).sum();
                            String sizeText = bytes > 1073741824
                                ? String.format("%.1f GB", bytes / 1073741824.0)
                                : String.format("%.0f MB", bytes / 1048576.0);
                            Platform.runLater(() -> { if (gameSize != null) gameSize.setText(sizeText); });
                        } catch (Exception ignored) {
                            Platform.runLater(() -> { if (gameSize != null) gameSize.setText("--"); });
                        }
                    });
                }
            }
            // Skip proton prefix for emulator/system-binary games (no wine prefix exists)
            boolean isEmulator = false;
            try {
                String runner = appModule.getGame("lutrisRunner", gameName);
                if (runner != null && runner.matches("(?i)melonds|dolphin|retroarch|rpcs3|yuzu|ryujinx|cemu|ppsspp|duckstation|pcsx2|xemu")) {
                    isEmulator = true;
                }
                if (!isEmulator && exec != null && (exec.startsWith("/usr/bin/") || exec.startsWith("/usr/local/bin/"))) {
                    isEmulator = true;
                }
            } catch (Exception ignored) {}
            if (!isEmulator) {
                try { prefix = com.corkytux.launcher.modules.FilesWorker.getProtonPrefixPath(gameName, "wine"); }
                catch (Exception ignored) {}
            } else {
                prefix = "N/A (emulator)";
            }
            final String fInstall = installPath != null ? installPath : "--";
            final String fPrefix = prefix != null ? prefix : "--";
            Platform.runLater(() -> {
                if (gameInstallPath != null) gameInstallPath.setText(fInstall);
                if (gamePrefixPath != null) gamePrefixPath.setText(fPrefix);
            });
        } catch (Exception e) {
            LOG.debug("updateGameInfo failed for {}", gameName, e);
        }
    }

    // -----------------------------------------------------------------------
    // Menus – mirrors doUtilitiesButtonConstruct / doGameFolderButtonConstruct
    // -----------------------------------------------------------------------

    private void buildUtilitiesMenu() {
        utilitiesMenu = new ContextMenu();
        var winecfg = new MenuItem(loc.get("MAINFORM.UTILITIES.WINECFG"));
        var taskmgr = new MenuItem(loc.get("MAINFORM.UTILITIES.TASKMGR"));
        var control = new MenuItem(loc.get("MAINFORM.UTILITIES.CONTROL"));
        var explorer = new MenuItem(loc.get("MAINFORM.UTILITIES.EXPLORER"));
        var cmd = new MenuItem(loc.get("MAINFORM.UTILITIES.CMD"));
        var regedit = new MenuItem(loc.get("MAINFORM.UTILITIES.REGEDIT"));
        var winetricks = new MenuItem("Winetricks");

        // Fix: Utilities must work offline – previously all showed as if no internet due to
        // network check failing and disabling menu or setting noInternet.png.
        // Utilities are local wine tools (winecfg, taskmgr, etc.) – zero internet required.
        // Ensure each item explicitly enabled (except winetricks if binary missing) and
        // that missing dependency does not gate the whole Utilities button gray.
        winecfg.setOnAction(e -> runWineUtil("winecfg.exe"));
        taskmgr.setOnAction(e -> runWineUtil("taskmgr.exe"));
        control.setOnAction(e -> runWineUtil("control.exe"));
        explorer.setOnAction(e -> runWineUtil("explorer.exe"));
        cmd.setOnAction(e -> runWineUtil("wineconsole"));
        regedit.setOnAction(e -> runWineUtil("regedit.exe"));
        winetricks.setOnAction(e -> runWineUtil("winetricks"));

        // Ensure none appear as no-internet disabled – only winetricks depends on /usr/bin/winetricks
        for (var item : List.of(winecfg, taskmgr, control, explorer, cmd, regedit)) {
            item.setDisable(false);
        }
        try {
            boolean hasWinetricks = Files.isRegularFile(Path.of("/usr/bin/winetricks"));
            winetricks.setDisable(!hasWinetricks);
            // MenuItem has no opacity – disabled state already indicates gray via CSS; keep enabled visuals
        } catch (Exception ex) {
            LOG.debug("winetricks check failed, leaving enabled for offline retry", ex);
            winetricks.setDisable(false);
        }

        utilitiesMenu.getItems().addAll(winecfg, taskmgr, control, explorer, cmd, regedit, new SeparatorMenuItem(), winetricks);

        // Ensure Utilities button itself never gray due to network: force enabled.
        if (utilitiesButton != null) {
            utilitiesButton.setDisable(false);
            utilitiesButton.setOpacity(1.0);
        }
    }

    private void runWineUtil(String util) {
        // Fix: Must handle offline + missing deps without leaving Utilities gray.
        // No internet check – all wine utils are local. Wrap proton lookup to avoid network exception disabling UI.
        String wine = null;
        String prefix = null;
        try {
            wine = FilesWorker.getProtonExecutable(currentGameName, "wine", true); // skipIfNotFound=true avoids network fetch
            if (wine == null) wine = FilesWorker.getProtonExecutable(currentGameName, "wine");
            prefix = FilesWorker.getProtonPrefixPath(currentGameName, "wine");
        } catch (Exception ex) {
            LOG.warn("Utilities: proton resolve failed offline for {}", util, ex);
        }
        if (wine == null || !Files.isRegularFile(Path.of(wine))) {
            toast(loc.get("FILESWORKER.PROTON.NOTFOUND"));
            LOG.warn("Utilities {} aborted – wine not found offline safe", util);
            // Ensure button not left disabled/gray
            if (utilitiesButton != null) {
                utilitiesButton.setDisable(false);
                utilitiesButton.setOpacity(1.0);
            }
            return;
        }
        // Prefix always created by getProtonPrefixPath; ensure it exists without network
        if (prefix != null) {
            try { Files.createDirectories(Path.of(prefix)); } catch (IOException ignored) {}
        }
        if (utilitiesButton != null) utilitiesButton.setDisable(true);
        final String finalWine = wine;
        final String finalPrefix = prefix;
        Thread.ofVirtual().start(() -> {
            Process proc = null;
            try {
                switch (util) {
                    case "winetricks" -> {
                        // Winetricks needs WINE + WINEPREFIX – start once with env, offline capable.
                        var pb = new ProcessBuilder("winetricks");
                        pb.environment().put("WINE", finalWine);
                        pb.environment().put("WINEPREFIX", finalPrefix);
                        proc = pb.start();
                    }
                    case "wineconsole" -> {
                        var pb = new ProcessBuilder(finalWine, "start", "cmd.exe");
                        pb.environment().put("WINEPREFIX", finalPrefix);
                        proc = pb.start();
                    }
                    default -> {
                        // Try syswow64, fallback to system32 if not exists (some prefixes)
                        var candidate = Path.of(finalPrefix + "/drive_c/windows/syswow64/" + util);
                        if (!Files.isRegularFile(candidate)) {
                            candidate = Path.of(finalPrefix + "/drive_c/windows/system32/" + util);
                            if (!Files.isRegularFile(candidate)) {
                                candidate = Path.of(finalPrefix + "/drive_c/windows/syswow64/" + util);
                            }
                        }
                        var pb = new ProcessBuilder(finalWine, candidate.toString());
                        pb.environment().put("WINEPREFIX", finalPrefix);
                        pb.environment().put("WINEDEBUG", "-all");
                        proc = pb.start();
                    }
                }
                if (proc != null) FilesWorker.hookProcessOuts(proc, false, false);
            } catch (IOException e) {
                LOG.error("runWineUtil failed {} (missing dep offline safe)", util, e);
                Platform.runLater(() -> toast("Failed " + util + ": " + e.getMessage()));
            } catch (Exception e) {
                LOG.error("runWineUtil unexpected {}", util, e);
            } finally {
                Platform.runLater(() -> {
                    if (utilitiesButton != null) {
                        utilitiesButton.setDisable(false);
                        utilitiesButton.setOpacity(1.0);
                    }
                });
            }
        });
    }

    private void buildGameFolderMenu() {
        gameFolderMenu = new ContextMenu();
        var gameFolder = new MenuItem(loc.get("GAMESETTINGS.FOLDERS.GAME"));
        var prefixFolder = new MenuItem(loc.get("GAMESETTINGS.FOLDERS.PREFIX"));
        gameFolder.setOnAction(e -> {
            if (currentGameName == null) return;
            String mainPath = appModule.getGame("mainPath", currentGameName);
            String exe = appModule.getGame("executable", currentGameName);
            String target = mainPath != null ? mainPath
                    : (exe != null && Path.of(exe).getParent() != null ? Path.of(exe).getParent().toString() : null);
            if (target != null) openPath(target);
        });
        prefixFolder.setOnAction(e -> {
            if (currentGameName == null) return;
            String prefixDir = FilesWorker.getProtonPrefixPath(currentGameName);
            if (prefixDir != null && Files.isDirectory(Path.of(prefixDir))) openPath(prefixDir);
            else toast(loc.get("GAMESETTINGS.WINETRICKS.NOPREFIX"));
        });
        gameFolderMenu.getItems().addAll(gameFolder, prefixFolder);
    }

    // -----------------------------------------------------------------------
    // Utilities
    // -----------------------------------------------------------------------

    private FlowPane findFlowPane() {
        if (flowContent != null) return flowContent;
        if (container != null && container.getContent() instanceof FlowPane fp) return fp;
        if (containerContent != null) {
            for (Node n : containerContent.getChildren()) if (n instanceof FlowPane fp) return fp;
        }
        return null;
    }

    private Stage stageOf(Node node) {
        if (node == null) return null;
        var scene = node.getScene();
        if (scene == null) return null;
        var win = scene.getWindow();
        return win instanceof Stage s ? s : null;
    }

    private void openUrl(String url) {
        try { com.corkytux.launcher.modules.FilesWorker.openWithXdgOpen(url); }
        catch (IOException e) { LOG.warn("xdg-open failed {}", url, e); }
        catch (Exception e) { LOG.warn("xdg-open failed {}", url, e); }
    }

    private void openPath(String path) {
        // Fix thumbnail cache perms before opening folders via Nemo (xdg-open).
        // When running as root: chowns root-owned dirs to correct user directly.
        // When running as non-root: skips root-owned dirs silently (no admin prompt).
        try { com.corkytux.launcher.modules.FilesWorker.fixThumbnailCachePermissions(); } catch (Exception ignored) {}
        try {
            // Ensure target folder itself is not root-owned to avoid admin prompt
            if (path != null) {
                try {
                    var p = java.nio.file.Path.of(path);
                    if (java.nio.file.Files.exists(p)) {
                        var owner = java.nio.file.Files.getOwner(p).getName();
                        String currentUser = System.getProperty("user.name");
                        if ("root".equals(owner)) {
                            String expected = System.getenv("SUDO_USER");
                            if (expected == null || expected.isBlank()) expected = currentUser;
                            if (expected == null || expected.isBlank() || "root".equals(expected)) expected = com.corkytux.launcher.modules.FilesWorker.getExpectedUser();
                            boolean isRootUser = "root".equals(currentUser);
                            if (isRootUser && expected != null && !"root".equals(expected)) {
                                LOG.warn("openPath target {} owned by root – fixing before xdg-open (running as root)", p);
                                var chownPb = new ProcessBuilder("chown", "-R", expected + ":" + expected, p.toString());
                                chownPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chownPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chownPb.start().waitFor();
                                var chmodPb = new ProcessBuilder("chmod", "-R", "755", p.toString());
                                chmodPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chmodPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                chmodPb.start().waitFor();
                            } else if (!isRootUser) {
                                // Non-root: skip root-owned dirs silently, never prompt for admin
                                LOG.debug("openPath target {} owned by root while running as {} – skipping (run launcher as root once to fix)", p, currentUser);
                            } else {
                                LOG.warn("openPath {} owned by root, running as root without SUDO_USER – xdg-open may need admin", p);
                            }
                        }
                        // Ensure cache perms right before xdg-open — always target the real user's cache, not /root
                        String cacheUser = "root".equals(currentUser)
                                ? com.corkytux.launcher.modules.FilesWorker.getExpectedUser()
                                : currentUser;
                        var cache = java.nio.file.Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".cache/thumbnails");
                        if (java.nio.file.Files.isDirectory(cache)) {
                            try { java.nio.file.Files.setPosixFilePermissions(cache, java.nio.file.attribute.PosixFilePermissions.fromString("rwxr-xr-x")); } catch (Exception ignored2) {}
                            for (var sub : new String[]{"normal","large","fail"}) {
                                var s = cache.resolve(sub);
                                if (java.nio.file.Files.isDirectory(s)) {
                                    try { java.nio.file.Files.setPosixFilePermissions(s, java.nio.file.attribute.PosixFilePermissions.fromString("rwxr-xr-x")); } catch (Exception ignored3) {}
                                }
                            }
                        }
                    }
                } catch (Exception ex) { LOG.debug("openPath pre-check failed", ex); }
            }
            // Prefer gio open for Wayland-safe, then xdg-open fallback – both handle Wayland/X11
            // Ensure launcher works for both Wayland and X11: don't force GDK_BACKEND, use desktop's default
            // Java 25: xdg-open must run as the user not root to avoid Nemo thumbnail admin error
            // Use FilesWorker.buildXdgOpenCommand which does sudo -u the user when root
            try {
                com.corkytux.launcher.modules.FilesWorker.openWithXdgOpen(path);
            } catch (Exception ex) {
                LOG.warn("xdg-open via FilesWorker failed for {}, fallback to direct xdg-open", path, ex);
                try { com.corkytux.launcher.modules.FilesWorker.buildXdgOpenCommand(path).start(); } catch (Exception ignored) {}
            }
        }
        catch (Exception e) { LOG.warn("open path failed {}", path, e); }
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

    private void toast(String message) {
        LOG.info("TOAST: {}", message);
        Platform.runLater(() -> {
            var stage = stageOf(container != null ? container : container);
            if (stage == null) return;
            // Minimal toast: Alert with auto-close, or status label fallback
            Alert alert = new Alert(Alert.AlertType.INFORMATION, message, ButtonType.OK);
            alert.setHeaderText(null);
            alert.initOwner(stage);
            // Auto-close after 2s to mimic toast
            var timer = new javafx.animation.PauseTransition(Duration.seconds(2));
            timer.setOnFinished(ev -> alert.close());
            timer.play();
            alert.show();
        });
    }

    /**
     * Loads secondary form controller via Launcher registry (FXMLLoader with correct fx:controller).
     * Falls back to direct registry lookup – no reflection, no NoSuchMethod.
     */
    private Object loadFormController(String formName) {
        // Ensure form is loaded via Launcher registry; if not yet loaded, trigger load synchronously via showForm logic
        Object ctrl = Launcher.getFormController(formName);
        if (ctrl != null) return ctrl;
        // Trigger lazy load on FX thread if needed – try to show then return controller
        if (javafx.application.Platform.isFxApplicationThread()) {
            try {
                // Force load by calling Launcher.showForm and waiting briefly – but better to use registry load directly
                Launcher.showForm(formName);
                // After Platform.runLater the controller will be available; try immediate fallback load via direct FXMLLoader if still null
                ctrl = Launcher.getFormController(formName);
                if (ctrl == null) {
                    // Direct fallback: try to load via Launcher helper (if form not yet shown, controller may still be null until runLater executes)
                    // We attempt to return null and let caller handle fallback via showForm
                    LOG.debug("loadFormController {} still null after showForm – will lazy show", formName);
                }
                return ctrl;
            } catch (Exception e) {
                LOG.debug("loadFormController {} failed via Launcher", formName, e);
                return null;
            }
        } else {
            // Non-FX thread – trigger and wait briefly
            var latch = new java.util.concurrent.CountDownLatch(1);
            javafx.application.Platform.runLater(() -> { Launcher.showForm(formName); latch.countDown(); });
            try { latch.await(2, java.util.concurrent.TimeUnit.SECONDS); } catch (InterruptedException ie) { Thread.currentThread().interrupt(); }
            return Launcher.getFormController(formName);
        }
    }

    /**
     * Shows secondary form via Launcher registry (creates Stage via FXMLLoader, correct fx:controller, handles show/hide).
     * Uses direct Launcher.showForm instead of reflection to avoid NoSuchMethodException.
     */
    private void showForm(String formName) {
        Launcher.showForm(formName);
    }

    /**
     * Shows secondary stage for already-configured controller. Ensures the form's Stage is created
     * via Launcher (FXMLLoader + fx:controller) and brings it to front. Handles hide/show lifecycle.
     * Wires MainForm to gameSettings/launcherSettings/protonManager (protonDownloader) stages.
     *
     * @param controller the controller instance previously configured (e.g. GameSettings with gameName, banner, icon)
     * @param hint       either the FXML name (e.g. "gameSettings") or a title/gameName hint – resolved to formName via controller type
     */
    private void showFormStage(Object controller, String hint) {
        if (controller == null || hint == null) {
            LOG.warn("showFormStage called with null controller/hint");
            return;
        }
        // Resolve formName: if hint is a known FXML name use it, otherwise infer from controller type for the three wired secondaries
        String resolved = hint;
        java.util.Set<String> known = java.util.Set.of("gameSettings", "launcherSettings", "protonDownloader", "protonManager", "newGameConfigurator", "gameRemover", "gameStarting", "envViewer", "envEditor", "log", "prototypes", "MainForm");
        if (!known.contains(hint)) {
            if (controller instanceof GameSettings) resolved = "gameSettings";
            else if (controller instanceof LauncherSettings) resolved = "launcherSettings";
            else if (controller instanceof ProtonDownloader) resolved = "protonDownloader";
            else if (controller instanceof NewGameConfigurator) resolved = "newGameConfigurator";
            else resolved = hint;
            if ("protonManager".equals(hint)) resolved = "protonDownloader";
        } else if ("protonManager".equals(hint)) {
            resolved = "protonDownloader";
        }
        final String formName = resolved;
        LOG.info("showFormStage {} hint={} -> form={}", controller.getClass().getSimpleName(), hint, formName);
        Object registered = Launcher.getFormController(formName);
        if (registered != null && registered == controller) {
            Launcher.showForm(formName);
            return;
        }
        if ("gameSettings".equals(formName) && controller instanceof GameSettings gs) {
            Object reg = Launcher.getFormController(formName);
            if (reg instanceof GameSettings regGs) {
                try { regGs.setGameName(gs.getGameNameValue()); } catch (Exception ignored) {}
            }
            String pendingGame = gs.getGameNameValue();
            final String pendingForm = formName;
            javafx.application.Platform.runLater(() -> {
                Object after = Launcher.getFormController(pendingForm);
                if (after instanceof GameSettings afterGs && pendingGame != null) {
                    afterGs.setGameName(pendingGame);
                }
            });
        }
        Launcher.showForm(formName);
    }

    // Overload for legacy callers passing title as gameName – map to gameSettings
    private void showFormStage(Object controller, String title, String formName) {
        showFormStage(controller, formName);
    }

    // Explicit wiring helpers for the three required secondaries – used by FXML actions and tests
    private void showGameSettingsStage() { showModal("gameSettings"); }
    private void showLauncherSettingsStage() { showModal("launcherSettings"); }
    private void showProtonManagerStage() { showModal("protonDownloader"); } // protonManager = protonDownloader

    // -----------------------------------------------------------------------
    // Accessors for tests / inter-form communication
    // -----------------------------------------------------------------------

    public String getCurrentGameName() { return currentGameName; }
    public Pane getGamePanelNode() { return gamePanel; }
    public Label getTimeLabel() { return timeLabel; }

    // -----------------------------------------------------------------------
    // Modal overlay system
    // -----------------------------------------------------------------------

    private void showModal(String formName) {
        try {
            String fxmlPath = "/fxml/" + formName + ".fxml";
            java.net.URL fxml = Launcher.class.getResource(fxmlPath);
            if (fxml == null) {
                LOG.warn("FXML not found for modal: {}", fxmlPath);
                return;
            }
            FXMLLoader loader = new FXMLLoader(fxml);
            Parent root = loader.load();
            Object controller = loader.getController();
            currentModalController = controller;

            // Set modal title based on form name
            String title = switch (formName) {
                case "launcherSettings" -> "Settings";
                case "gameSettings" -> "Game Settings";
                case "newGameConfigurator" -> "Add Game";
                case "gameRemover" -> "Remove Game";
                default -> formName;
            };
            if (modalTitle != null) modalTitle.setText(title);

            // Compact modal for small dialogs, full size otherwise
            if (modalContent != null) {
                if ("gameRemover".equals(formName)) {
                    modalContent.setPrefSize(420, 260);
                    modalContent.setMaxSize(420, 260);
                } else if ("newGameConfigurator".equals(formName)) {
                    modalContent.setPrefSize(440, 500);
                    modalContent.setMaxSize(440, 500);
                } else {
                    modalContent.setPrefSize(900, 600);
                    modalContent.setMaxSize(900, 600);
                }
            }

            // Clear previous content and add new
            if (modalBody != null) {
                modalBody.getChildren().clear();
                modalBody.getChildren().add(root);
                // Make the loaded content fill the modal body
                if (root instanceof Region r) {
                    r.setMinWidth(0);
                    r.setMinHeight(0);
                    StackPane.setAlignment(r, javafx.geometry.Pos.CENTER);
                    StackPane.setMargin(r, new javafx.geometry.Insets(0));
                }
            }

            // Show overlay
            if (modalOverlay != null) {
                modalOverlay.setVisible(true);
                modalOverlay.setManaged(true);
                modalOverlay.toFront();
            }

            LOG.info("Modal shown: {}", formName);
        } catch (Exception ex) {
            LOG.error("Failed to show modal: {}", formName, ex);
        }
    }

    private void hideModal() {
        if (modalOverlay != null) {
            modalOverlay.setVisible(false);
            modalOverlay.setManaged(false);
        }
        if (modalBody != null) {
            modalBody.getChildren().clear();
        }
        LOG.info("Modal hidden");
    }
}

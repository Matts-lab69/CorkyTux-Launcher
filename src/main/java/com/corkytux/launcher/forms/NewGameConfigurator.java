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
import com.corkytux.launcher.modules.FtpInstaller;
import com.corkytux.launcher.modules.Localization;
import com.corkytux.launcher.modules.RarExtractor;
import javafx.animation.FadeTransition;
import javafx.application.Platform;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.Node;
import javafx.scene.control.*;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.input.MouseEvent;
import javafx.scene.layout.HBox;
import javafx.scene.layout.VBox;
import javafx.stage.DirectoryChooser;
import javafx.stage.FileChooser;
import javafx.stage.Stage;
import javafx.util.Callback;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.regex.Pattern;
import java.util.stream.Collectors;

/**
 * Java 25 / JavaFX 21 port of {@code newGameConfigurator.php} (706 lines).
 *
 * <p>Handles adding a game: scanning a directory, presenting candidates in a ListView,
 * parsing RAR archives, resolving install-vs-add paths, extracting icons/banners,
 * and creating the persistent {@code Games.ini} entry. Mirrors every PHP helper:
 * {@code prepareForGame}, {@code checkAreCanListed}, {@code checkAreInstallPossible},
 * {@code checkAreAutoSelectPossible}, {@code detectBasePath}, and the full
 * {@code doAddGameAction} async pipeline.</p>
 *
 * <p>FXML: {@code /fxml/newGameConfigurator.fxml}</p>
 */
public class NewGameConfigurator implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(NewGameConfigurator.class);

    // -----------------------------------------------------------------------
    // FXML
    // -----------------------------------------------------------------------

    @FXML private VBox mainSelectBox;
    @FXML private VBox gameParamsBox;
    @FXML private ListView<Candidate> listView;
    @FXML private Button selectFileButton;
    @FXML private Label mainSelectLabel;

    @FXML private Label label; // header
    @FXML private Label label3;
    @FXML private Label label4;
    @FXML private Label label7;
    @FXML private Label labelAlt;
    @FXML private TextField gameName;
    @FXML private TextField gamePath;
    @FXML private TextField prefixPath;
    @FXML private TextField ftpInstallerPath;
    @FXML private VBox gamePathBox;
    @FXML private VBox ftpInstallerBox;
    @FXML private VBox vbox4; // prefix path wrapper
    @FXML private VBox vbox6; // gameName wrapper
    @FXML private Button addGame;
    @FXML private Button cancelButton;
    @FXML private javafx.scene.control.Button cleanAfterAdd;
    @FXML private CheckBox cleanAfterAddCheck;
    @FXML private Separator separator;
    @FXML private Separator separatorAlt;
    @FXML private HBox hbox;

    // -----------------------------------------------------------------------
    // State – mirrors PHP $gameParams associative array
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code $gameParams} – holds transient state between
     * {@code prepareForGame} and {@code doAddGameAction}.
     */
    public static final class GameParams {
        public String path;               // scan root supplied to prepareForGame
        public String mainFile;           // selected candidate absolute path
        public String originalFile;       // original rar if nested
        public String unpackedPath;       // base path detected inside rar
        public boolean canInstall;        // whether ftp installer flow applies
        public boolean openedFromAria = false; // mirrors PHP gameParams['openedFromAria']
        public boolean skipConfig = false;     // set by ftpInstaller headless flow
    }

    private final GameParams gameParams = new GameParams();
    private boolean isFree = false; // mirrors PHP isFree() – true after doAddGameAction free()

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();

    // -----------------------------------------------------------------------
    // Candidate record – mirrors PHP candidate array [name, dir, canInstall]
    // -----------------------------------------------------------------------

    public record Candidate(String fileName, String directory, boolean canInstall) {
        @Override public String toString() { return fileName + "  " + directory; }
    }

    // -----------------------------------------------------------------------
    // Skip / auto-select patterns – verbatim from PHP newGameConfigurator.php lines 624-683
    // PHP skipList contains 32 executable skip regexes + 1 generic non-executable guard
    // (task description says "26 skip regexes" for core engine/DRM skips, the remaining
    // 6 cover profilers/crash handlers + generic guard – we preserve the full 32 for 1:1 parity)
    // rar/ftp handling mirrored in checkAreInstallPossible / checkAreAutoSelectPossible below.
    // -----------------------------------------------------------------------

    private static final List<Pattern> SKIP_PATTERNS = List.of(
            Pattern.compile("^vcredist.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^vc_redist.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^dxsetup\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^directx.*setup.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^dxwebsetup\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^dotnet.*setup.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^ndp[A-Za-z0-9._-]*(?=.*-KB\\d+)(?=.*-(?:x86|x64))(?=.*-AllOS)(?=.*-ENU)[A-Za-z0-9._-]*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^install.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^uninstall.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^unins[0-9]+\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^updater.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^patch.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^steam\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^origin.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^uplay.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^epicgames.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^gog.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^unitycrashhandler.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^unitybugreporter.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^winpixeventruntime.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^monodistribution.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^ue4.*prereq.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^crashreportclient.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^bugreporter.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^sandbox.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^editor.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^config.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^settings.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^options.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^benchmark.*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^profil(er|ing).*\\.exe$", Pattern.CASE_INSENSITIVE),
            Pattern.compile("^crashpad_handler\\.exe$", Pattern.CASE_INSENSITIVE)
    );

    private static final Pattern NON_EXECUTABLE = Pattern.compile("^(?!.*\\.(exe|vbs|bat|rar)$)", Pattern.CASE_INSENSITIVE);

    // -----------------------------------------------------------------------
    // Initializable
    // -----------------------------------------------------------------------

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        // ListView cell factory – mirrors doListViewConstruct
        if (listView != null) {
            listView.setCellFactory(new Callback<>() {
                @Override public ListCell<Candidate> call(ListView<Candidate> lv) {
                    return new ListCell<>() {
                        @Override protected void updateItem(Candidate item, boolean empty) {
                            super.updateItem(item, empty);
                            if (empty || item == null) {
                                setText(null); setGraphic(null);
                            } else {
                                var vbox = new VBox(0);
                                vbox.setAlignment(javafx.geometry.Pos.CENTER_LEFT);
                                var l1 = new Label(item.fileName());
                                l1.setStyle("-fx-font-weight: bold; -fx-text-fill: white; -fx-font-size: 12;");
                                var l2 = new Label(item.directory());
                                l2.setStyle("-fx-text-fill: #e6e6e6; -fx-font-size: 12;");
                                vbox.getChildren().addAll(l1, l2);
                                setGraphic(vbox);
                                setText(null);
                                getProperties().put("canInstall", item.canInstall());
                            }
                        }
                    };
                }
            });
        }

        applyLocalizations();
        wireActions();

        // initial visibility: show mainSelectBox, hide gameParamsBox – mirrors FXML defaults
        if (mainSelectBox != null) mainSelectBox.setVisible(true);
        if (gameParamsBox != null) gameParamsBox.setVisible(false);

        // Double-click on ListView selects candidate – mirrors PHP listItem click
        if (listView != null) {
            listView.setOnMouseClicked(e -> {
                if (e.getClickCount() == 2) handleSelectFile(null);
            });
            // Also Enter key triggers selection
            listView.setOnKeyReleased(e -> {
                if (e.getCode() == KeyCode.ENTER) handleSelectFile(null);
            });
        }

        // Esc handler – mirrors doKeyUpEsc
        var root = mainSelectBox != null ? mainSelectBox : gameParamsBox;
        if (root != null) {
            root.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.getCode() == KeyCode.ESCAPE) handleHide();
            });
        }

        // Ensure cleanAfterAdd toggle has proper switch
        if (cleanAfterAdd != null && cleanAfterAdd.getProperties().get("quUIElement") == null) {
            String txt = loc.get("NEWGAMECONFIG.CLEANAFTERADD");
            var sw = new com.corkytux.launcher.ui.SwitchComponent(txt != null ? txt : "Clean after add");
            cleanAfterAdd.getProperties().put("quUIElement", sw);
            cleanAfterAdd.setGraphic(sw);
            cleanAfterAdd.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
            cleanAfterAdd.setPrefHeight(34);
            cleanAfterAdd.setMinHeight(34);
            cleanAfterAdd.setMaxHeight(34);
        }

        // Ensure initial pane state matches FXML: mainSelectBox visible, gameParamsBox hidden
        if (mainSelectBox != null) { mainSelectBox.setVisible(true); mainSelectBox.setManaged(true); }
        if (gameParamsBox != null) { gameParamsBox.setVisible(false); gameParamsBox.setManaged(false); }
    }

    private void applyLocalizations() {
        if (label != null) label.setText(loc.get("NEWGAMECONFIG.HEADER"));
        if (label3 != null) label3.setText(loc.get("NEWGAMECONFIG.NAME"));
        if (label4 != null) label4.setText(loc.get("NEWGAMECONFIG.GAMEPATH"));
        if (label7 != null) label7.setText(loc.get("NEWGAMECONFIG.PREFIXPATH"));
        if (labelAlt != null) labelAlt.setText(loc.get("NEWGAMECONFIG.FREETP"));
        if (cancelButton != null) cancelButton.setText(loc.get("CANCEL"));
        if (mainSelectLabel != null) mainSelectLabel.setText(loc.get("NEWGAMECONFIG.SELECTMAIN"));
        if (selectFileButton != null) selectFileButton.setText(loc.get("NEXT"));
        if (addGame != null) addGame.setText(loc.get("ADD"));
        // cleanAfterAdd ToggleSwitch text – mirrors doCleanAfterAddConstruct
        if (cleanAfterAdd != null) cleanAfterAdd.setText(loc.get("NEWGAMECONFIG.CLEANAFTERADD"));
        if (cleanAfterAddCheck != null) cleanAfterAddCheck.setText(loc.get("NEWGAMECONFIG.CLEANAFTERADD"));
    }

    private void wireActions() {
        if (selectFileButton != null) selectFileButton.setOnAction(e -> handleSelectFile(null));
        if (addGame != null) addGame.setOnAction(e -> handleAddGame());
        if (cancelButton != null) cancelButton.setOnAction(e -> handleCancel());

        // VBox click delegates – mirrors doGamePathBoxClick etc.
        if (gamePathBox != null) gamePathBox.setOnMouseClicked(e -> handleGamePathClick());
        if (vbox4 != null) vbox4.setOnMouseClicked(e -> handlePrefixPathClick());
        if (vbox6 != null) vbox6.setOnMouseClicked(e -> { if (gameName != null) gameName.requestFocus(); });
        if (gamePath != null) gamePath.setOnMouseClicked(e -> handleGamePathClick());
        if (prefixPath != null) prefixPath.setOnMouseClicked(e -> handlePrefixPathClick());
        if (ftpInstallerBox != null) ftpInstallerBox.setOnMouseClicked(e -> handleFtpInstallerPathClick());
        if (ftpInstallerPath != null) ftpInstallerPath.setOnMouseClicked(e -> handleFtpInstallerPathClick());

        // live update of derived paths – mirrors doGameNameKeyUp
        if (gameName != null) {
            gameName.setOnKeyReleased(e -> {
                String text = gameName.getText();
                String installDefault = appModule.getLauncher("installsPath", "User Settings");
                String prefixParent = prefixPath != null && prefixPath.getText() != null
                        ? Path.of(prefixPath.getText()).getParent() != null ? Path.of(prefixPath.getText()).getParent().toString() : ""
                        : "";
                String prefixDefault = getBasePathFor("prefixes");
                if (prefixDefault.equals(prefixParent) && text != null) {
                    if (prefixPath != null) prefixPath.setText(prefixParent + "/" + text);
                }
                if (gamePath != null && gamePath.getText() != null && !Files.isDirectory(Path.of(gamePath.getText()))) {
                    if (installDefault != null && text != null) gamePath.setText(installDefault + "/" + text);
                }
            });
        }
    }

    // -----------------------------------------------------------------------
    // Handlers – mirrors PHP @event methods
    // -----------------------------------------------------------------------

    @FXML
    private void handleGamePathClick() {
        var dc = new DirectoryChooser();
        var win = stageOf(gamePath);
        var dir = dc.showDialog(win);
        if (dir == null) return;
        // mirrors FtpInstallerBox enabled check + NONEMPTY guard
        boolean ftpEnabled = ftpInstallerBox == null || !ftpInstallerBox.isDisable();
        try {
            boolean hasFiles = false;
            try (var s = Files.list(dir.toPath())) { hasFiles = s.findAny().isPresent(); }
            if (!ftpEnabled && hasFiles) {
                showAlert(loc.get("NEWGAMECONFIG.PATH.NONEMPTY"), Alert.AlertType.ERROR);
                handleGamePathClick(); // recurse like PHP
                return;
            }
        } catch (IOException e) { LOG.debug("handleGamePathClick list failed", e); }
        if (gamePath != null) gamePath.setText(dir.getAbsolutePath());
    }

    @FXML
    private void handlePrefixPathClick() {
        var dc = new DirectoryChooser();
        var win = stageOf(prefixPath);
        var dir = dc.showDialog(win);
        if (dir == null) return;
        try (var s = Files.list(dir.toPath())) {
            if (s.findAny().isPresent()) showAlert(loc.get("NEWGAMECONFIG.PREFIX.PATHNONEMPTY"), Alert.AlertType.WARNING);
        } catch (IOException e) { LOG.debug("prefix path check failed", e); }
        if (prefixPath != null) prefixPath.setText(dir.getAbsolutePath());
    }

    @FXML
    private void handleFtpInstallerPathClick() {
        var fc = new FileChooser();
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter(loc.get("NEWGAMECONFIG.FREETP.FILECHOOSER"), "*.exe"));
        var win = stageOf(ftpInstallerPath);
        var file = fc.showOpenDialog(win);
        if (file == null) return;
        if (ftpInstallerPath != null) ftpInstallerPath.setText(file.getAbsolutePath());
    }

    @FXML
    private void handleHide() {
        if (isFree) return;
        free();
        handleCancel();
    }

    @FXML
    private void handleCancel() {
        if (gameParams.openedFromAria && (!isFree || confirm(loc.get("NEWGAMECONFIG.ARESURE")))) {
            if (gameParams.originalFile != null) {
                var parent = Path.of(gameParams.originalFile).getParent();
                if (parent != null) deleteRecursively(parent);
            }
            hideStage();
        } else if (gameParams.skipConfig && !isFree) {
            if (prefixPath != null && prefixPath.getText() != null) deleteRecursively(Path.of(prefixPath.getText()));
            hideStage();
        } else if (!gameParams.openedFromAria && !isFree) {
            hideStage();
        }
    }

    private void free() {
        isFree = true;
        // mirrors PHP free() – disable form
        if (mainSelectBox != null) mainSelectBox.setDisable(true);
        if (gameParamsBox != null) gameParamsBox.setDisable(true);
    }

    public boolean isFree() { return isFree; }

    // -----------------------------------------------------------------------
    // File selection – mirrors doSelectFileButtonAction
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code doSelectFileButtonAction($e=null,$candidate=null)}.
     * If {@code candidate} is null, uses the ListView selection.
     */
    public void handleSelectFile(Candidate explicitCandidate) {
        Candidate candidate = explicitCandidate;
        boolean canInstall;
        String filePath;
        if (candidate == null) {
            if (listView == null || listView.getSelectionModel().getSelectedItem() == null) {
                toast(loc.get("NOTHING.SELECTED"));
                return;
            }
            var sel = listView.getSelectionModel().getSelectedItem();
            canInstall = sel.canInstall();
            filePath = (gameParams.path != null ? gameParams.path : "") + sel.directory() + sel.fileName();
            candidate = sel;
        } else {
            canInstall = candidate.canInstall();
            filePath = (gameParams.path != null ? gameParams.path : "") + candidate.directory() + candidate.fileName();
        }

        String nameNoExt = stripExtension(Path.of(filePath).getFileName() != null ? Path.of(filePath).getFileName().toString() : filePath);
        String ext = getExtension(filePath);
        boolean isLauncher = nameNoExt.toLowerCase().contains("launcher");

        if (canInstall && "rar".equalsIgnoreCase(ext)) {
            gameParams.originalFile = filePath;
            parseFromRar(filePath);
            return;
        }

        gameParams.mainFile = filePath;
        gameParams.canInstall = canInstall;

        String defaultInstallPath = appModule.getLauncher("installsPath", "User Settings");
        if (!canInstall || !"exe".equalsIgnoreCase(ext)) {
            if (gamePath != null) gamePath.setText(gameParams.path != null ? gameParams.path : (defaultInstallPath != null ? defaultInstallPath : ""));
            if (ftpInstallerBox != null) ftpInstallerBox.setDisable(true);
        } else if (defaultInstallPath != null && "exe".equalsIgnoreCase(ext) && canInstall) {
            String parentName = Path.of(filePath).getParent() != null ? Path.of(filePath).getParent().getFileName().toString() : "";
            if (gamePath != null) gamePath.setText(defaultInstallPath + "/" + parentName);
        }

        if (gameParams.skipConfig) {
            handleAddGame();
            return;
        }

        if ("exe".equalsIgnoreCase(ext) && !isLauncher && !canInstall) {
            if (gameName != null) gameName.setText(nameNoExt);
        } else {
            String parentName = Path.of(filePath).getParent() != null ? Path.of(filePath).getParent().getFileName().toString() : nameNoExt;
            if (gameName != null) gameName.setText(parentName);
            nameNoExt = gameName != null ? gameName.getText() : parentName;
        }

        if (gameParams.path != null && (ftpInstallerBox == null || ftpInstallerBox.isDisable())) {
            if (gamePathBox != null) gamePathBox.setDisable(true);
            setCleanAfterAddEnabled(false);
        }

        String prefixBase = getBasePathFor("prefixes");
        if (prefixPath != null) prefixPath.setText(prefixBase + "/" + nameNoExt);

        // switch panes – mirrors mainSelectBox->free() / gameParamsBox->show()
        if (mainSelectBox != null) {
            mainSelectBox.setVisible(false);
            mainSelectBox.setManaged(false);
        }
        if (gameParamsBox != null) {
            gameParamsBox.setVisible(true);
            gameParamsBox.setManaged(true);
        }
    }

    private void parseFromRar(String file) {
        List<String> files;
        var extractor = new RarExtractor();
        try {
            files = extractor.getRarContent(file);
        } catch (Exception ex) {
            var result = extractor.retryWithEnsureError(ex.getMessage(), file);
            if (result == null) { showAlert(ex.getMessage(), Alert.AlertType.ERROR); return; }
            if (result instanceof List<?> list) {
                @SuppressWarnings("unchecked")
                List<String> typed = (List<String>) list;
                var parsedRetry = typed.stream().map(f -> "/" + f).collect(Collectors.toList());
                if (parsedRetry.isEmpty()) {
                    showAlert(loc.get("NEWGAMECONFIG.EMPTYRAR"), Alert.AlertType.ERROR);
                    return;
                }
                prepareForGame(parsedRetry, null);
            }
            return;
        }
        var parsedFiles = files.stream().map(f -> "/" + f).collect(Collectors.toList());
        if (parsedFiles.isEmpty()) {
            showAlert(loc.get("NEWGAMECONFIG.EMPTYRAR"), Alert.AlertType.ERROR);
            return;
        }
        prepareForGame(parsedFiles, null);
    }

    // -----------------------------------------------------------------------
    // Add game – mirrors doAddGameAction (the long async pipeline)
    // -----------------------------------------------------------------------

    @FXML
    private void handleAddGame() {
        if (!gameParams.skipConfig) {
            var check = checkAreAddPossible();
            if (!(check instanceof Boolean b && b)) {
                String msg = check instanceof String s ? s : loc.get("NEWGAMECONFIG.FIELDSEMPTY");
                showAlert(msg, Alert.AlertType.ERROR);
                return;
            }
        }

        free();

        if (gameParams.canInstall) {
            String prefix = prefixPath != null ? prefixPath.getText() : "";
            String gPath = gamePath != null ? gamePath.getText() : "";
            String ftpPath = ftpInstallerPath != null ? ftpInstallerPath.getText() : "";
            boolean clean = isCleanAfterAddSelected();
            String name = gameName != null ? gameName.getText() : "";
            FtpInstaller.install(name, List.of(gameParams.mainFile, ftpPath), prefix, gPath, clean);
            hideStage();
            return;
        }

        // Non-install (simple add) – stub + background thread mirrors PHP
        var mainForm = findMainForm();
        MainForm.StubGame stub = null;
        if (mainForm != null) {
            stub = mainForm.addStubGame();
            final var s = stub;
            String displayName = gameName != null ? gameName.getText() : "";
            Platform.runLater(() -> s.gameNameLabel().setText(displayName));
        }

        final MainForm.StubGame finalStub = stub;
        Thread.ofVirtual().start(() -> {
            // 1. unpack rar if needed
            if (gameParams.originalFile != null && "rar".equalsIgnoreCase(getExtension(gameParams.originalFile))) {
                if (finalStub != null) Platform.runLater(() -> finalStub.statusLabel().setText(loc.get("NEWGAMECONFIG.UNPACKING")));
                var extractor = new RarExtractor();
                try {
                    extractor.unpackRar(gameParams.originalFile, gamePath.getText());
                } catch (Exception ex) {
                    var result = extractor.retryWithEnsureError(ex.getMessage(), gameParams.originalFile, gamePath.getText());
                    if (!Boolean.TRUE.equals(result)) {
                        if (finalStub != null && mainForm != null) Platform.runLater(() -> mainForm.removeStubGame(finalStub.box()));
                        return;
                    }
                }
            }

            // 2. fix up mainFile path if not absolute
            if (!Files.isRegularFile(Path.of(gameParams.mainFile))) {
                gameParams.mainFile = gamePath.getText() + gameParams.mainFile;
            }
            String resolvedPath = gamePath.getText();
            if (gameParams.unpackedPath != null) {
                // PHP: $path = $this->gamePath->text .= $this->gameParams['unpackedPath'];
                resolvedPath = gamePath.getText() + gameParams.unpackedPath;
            }

            if (finalStub != null) Platform.runLater(() -> finalStub.statusLabel().setText(loc.get("NEWGAMECONFIG.DLLS")));

            var parsed = FixParser.parseDlls(resolvedPath);
            if (parsed == null) parsed = Map.of("overrides", "", "fixPath", "");
            String fakeAppId = parsed.get("fakeAppId");
            String realAppId = parsed.get("realAppId");
            String overrides = parsed.getOrDefault("overrides", "");
            String fixPath = parsed.get("fixPath");

            if (fakeAppId != null) appModule.setGame("fakeSteamID", fakeAppId, gameName.getText());

            String bannerPath = null;
            if (realAppId != null) {
                String rawBanner = FixParser.parseBanner(realAppId);
                if (rawBanner != null && Files.isRegularFile(Path.of(rawBanner))) {
                    bannerPath = rawBanner;
                    appModule.setGame("banner", bannerPath, gameName.getText());
                }
                // else: banner download failed – don't store error string as banner
                appModule.setGame("steamID", realAppId, gameName.getText());
            }
            // Unified artwork fallback: Steam CDN + Lutris local + SteamGridDB
            // (fills banner/icon when FixParser found nothing)
            try {
                var im = com.corkytux.launcher.modules.IntegrationsManager.getInstance();
                var art = im.resolveArtwork(gameName.getText(), realAppId);
                if (bannerPath == null && art.containsKey("banner")) {
                    bannerPath = art.get("banner");
                    appModule.setGame("banner", bannerPath, gameName.getText());
                }
                if (art.containsKey("icon")) {
                    appModule.setGame("icon", art.get("icon"), gameName.getText());
                }
            } catch (Exception e) {
                LOG.debug("unified artwork failed", e);
            }

            if (finalStub != null) Platform.runLater(() -> finalStub.statusLabel().setText(loc.get("NEWGAMECONFIG.ICOEXTRACT")));

            String appIcon = null;
            try {
                appIcon = FixParser.parseIcon(gameParams.mainFile);
                if (appIcon != null && Files.isRegularFile(Path.of(appIcon))) {
                    appModule.setGame("icon", appIcon, gameName.getText());
                }
            } catch (Exception ex) {
                final String msg = ex.getMessage();
                Platform.runLater(() -> showAlert(String.format(loc.get("MAINFORM.ICONPARSERERROR"), msg), Alert.AlertType.ERROR));
            }

            if (finalStub != null) Platform.runLater(() -> finalStub.statusLabel().setText(loc.get("NEWGAMECONFIG.SETTINGS")));

            String defaultProton = appModule.getLauncher("defaultProton", "User Settings");
            if (defaultProton == null) defaultProton = "GE-Proton Latest";
            String steamRuntimeVal = FilesWorker.findSteamRuntime(defaultProton) != null ? "true" : "false";
            String wined3dVal = appModule.getLauncher("gamesUsesWined3d", "User Settings");
            String waylandVal = appModule.getLauncher("gamesUsesWayland", "User Settings");

            // Persist full game entry – mirrors games->put([...], name)
            var gameData = new LinkedHashMap<String, String>();
            LOG.info("Persisting game '{}'", gameName.getText());
            gameData.put("overrides", overrides);
            gameData.put("executable", gameParams.mainFile);
            gameData.put("mainPath", resolvedPath);
            gameData.put("prefixPath", prefixPath.getText());
            gameData.put("proton", defaultProton);
            gameData.put("steamRuntime", steamRuntimeVal);
            gameData.put("steamOverlay", "true");
            gameData.put("wined3d", wined3dVal);
            gameData.put("nativeWayland", waylandVal);
            if (fixPath != null) gameData.put("fixPath", fixPath);

            for (var e : gameData.entrySet()) {
                if (e.getValue() != null) appModule.setGame(e.getKey(), e.getValue(), gameName.getText());
            }

            final String fBanner = bannerPath;
            final String fIcon = appIcon;
            final String fOverrides = overrides;
            Platform.runLater(() -> {
                if (mainForm != null && finalStub != null) {
                    mainForm.removeStubGame(finalStub.box());
                    mainForm.addGame(gameName.getText(), gameParams.mainFile, fOverrides, fBanner, fIcon);
                }
                hideStage();
            });

            if (isCleanAfterAddSelected()) {
                String origParent = gameParams.originalFile != null ? Path.of(gameParams.originalFile).getParent() != null ? Path.of(gameParams.originalFile).getParent().toString() : null : null;
                if (origParent != null && resolvedPath.contains(origParent)) {
                    Platform.runLater(() -> showAlert(loc.get("NEWGAMECONFIG.CLEANSKIPPED"), Alert.AlertType.WARNING));
                    return;
                }
                if (origParent != null) deleteRecursively(Path.of(origParent));
            }
        });
    }

    private Object checkAreAddPossible() {
        String name = gameName != null ? gameName.getText() : null;
        String gPath = gamePath != null ? gamePath.getText() : null;
        String pPath = prefixPath != null ? prefixPath.getText() : null;
        if (name == null || name.isBlank() || gPath == null || gPath.isBlank() || pPath == null || pPath.isBlank()) {
            return loc.get("NEWGAMECONFIG.FIELDSEMPTY");
        }
        // check section exists – mirrors games->section(name) != []
        String exe = appModule.getGame("executable", name);
        if (exe != null) return loc.get("MAINFORM.GAMEEXISTS");
        // also check ini directly for completeness
        var iniPath = Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".config/CorkyTux/Games.ini");
        if (Files.isRegularFile(iniPath)) {
            try {
                var wini = new org.ini4j.Wini(iniPath.toFile());
                if (wini.get(name) != null && !wini.get(name).isEmpty()) return loc.get("MAINFORM.GAMEEXISTS");
            } catch (Exception ignored) {}
        }
        return Boolean.TRUE;
    }

    // -----------------------------------------------------------------------
    // prepareForGame – mirrors PHP prepareForGame($files,$path=null)
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code prepareForGame($files,$path=null)}.
     * Scans candidates, applies skip/install logic, auto-selects single entries.
     */
    public void prepareForGame(List<?> files, String path) {
        gameParams.path = path;
        if (path == null) {
            // files came from rar – detect base path inside archive
            List<String> strFiles = files.stream().map(Object::toString).collect(Collectors.toList());
            gameParams.unpackedPath = detectBasePath(strFiles);
        }
        if (gameParams.openedFromAria) {
            setCleanAfterAddEnabled(false);
            setCleanAfterAddSelected(true);
        }

        var candidates = new ArrayList<Candidate>();
        var candidateNames = new ArrayList<String>();

        // Need string form for rarFiles context when calling autoSelect
        List<String> strFilesForAuto = files.stream().map(Object::toString).collect(Collectors.toList());

        for (Object raw : files) {
            String file = raw.toString();
            if (!checkAreCanListed(file)) continue;

            if ("rar".equalsIgnoreCase(getExtension(file))) {
                if (Path.of(file).getParent() != null && "Fix Repair".equals(Path.of(file).getParent().getFileName().toString())) continue;
                if (file.contains(".part")) {
                    String noPart = file.substring(0, file.lastIndexOf(".part")) + ".rar";
                    if (!candidateNames.contains(Path.of(noPart).getFileName().toString())) {
                        file = noPart;
                    } else continue;
                }
            }

            String fileName = Path.of(file).getFileName().toString();
            String parent = Path.of(file).getParent() != null ? Path.of(file).getParent().toString() : "";
            String dirPart = "";
            if (path != null && !path.isEmpty() && parent.startsWith(path)) {
                dirPart = parent.substring(path.length());
                if (!dirPart.endsWith("/")) dirPart += "/";
                if (dirPart.equals("/")) dirPart = "/";
            } else if (!parent.isEmpty()) {
                dirPart = parent + "/";
            } else {
                dirPart = "/";
            }
            // Normalize: PHP uses str::replace(fs::parent($file),$path,null).'/'
            boolean canInstall = checkAreInstallPossible(file);
            var cand = new Candidate(fileName, dirPart, canInstall);
            candidates.add(cand);
            candidateNames.add(fileName);

            // auto-select check – mirrors checkAreAutoSelectPossible
            List<String> rarFilesArg = gameParams.originalFile != null ? strFilesForAuto : null;
            if (checkAreAutoSelectPossible(file, rarFilesArg)) {
                handleSelectFile(candidates.get(candidates.size() - 1));
                return;
            }
        }

        int count = candidates.size();
        if (count == 0) {
            showAlert(loc.get("NOTHING.FOUND"), Alert.AlertType.ERROR);
            return;
        } else if (count == 1) {
            LOG.info("Only one file, so auto-select");
            handleSelectFile(candidates.get(0));
        } else {
            // Multi-candidate: show list, ensure pane visibility 1:1 with FXML defaults and focus
            Runnable showList = () -> {
                if (mainSelectBox != null) { mainSelectBox.setVisible(true); mainSelectBox.setManaged(true); }
                if (gameParamsBox != null) { gameParamsBox.setVisible(false); gameParamsBox.setManaged(false); }
                if (listView != null) {
                    ObservableList<Candidate> items = FXCollections.observableArrayList(candidates);
                    listView.setItems(items);
                    listView.refresh();
                    listView.setVisible(true);
                    listView.setManaged(true);
                    if (!items.isEmpty()) {
                        listView.getSelectionModel().select(0);
                        listView.requestFocus();
                        listView.scrollTo(0);
                    }
                    LOG.info("prepareForGame: showing {} candidates", items.size());
                } else {
                    LOG.warn("prepareForGame: listView is null, cannot show {} candidates", candidates.size());
                }
                Stage s = stageOf(mainSelectBox != null ? mainSelectBox : gameParamsBox);
                if (s != null && !s.isShowing()) s.show();
            };
            if (Platform.isFxApplicationThread()) showList.run();
            else Platform.runLater(showList);
        }
    }

    // overload for Path list (used by FtpInstaller path)
    public void prepareForGamePaths(List<Path> files, String path) {
        prepareForGame(files.stream().map(Path::toString).collect(Collectors.toList()), path);
    }

    // -----------------------------------------------------------------------
    // Static helpers – mirrors PHP static functions
    // -----------------------------------------------------------------------

    public static String detectBasePath(List<String> files) {
        String best = null;
        int bestCount = Integer.MAX_VALUE;
        for (String file : files) {
            String parent = Path.of(file).getParent() != null ? Path.of(file).getParent().toString() : "";
            int count = parent.isEmpty() ? 0 : parent.split("/").length;
            if (best == null || count < bestCount) {
                best = parent;
                bestCount = count;
            }
        }
        return best != null ? best : "";
    }

    private static boolean isFile(String file, List<String> files) {
        if (files != null) {
            var lower = files.stream().map(String::toLowerCase).collect(Collectors.toSet());
            return lower.contains(file.toLowerCase());
        }
        return Files.isRegularFile(Path.of(file));
    }

    public static boolean checkAreAutoSelectPossible(String file, List<String> rarFiles) {
        var allowNames = Set.of("eosauthlauncher.exe", "launcher.exe");
        String lower = Path.of(file).getFileName().toString().toLowerCase();
        if (!allowNames.contains(lower)) return false;
        if ("launcher.exe".equals(lower)) {
            // need onlinefix.json sibling check
            String parent = Path.of(file).getParent() != null ? Path.of(file).getParent().toString() : "";
            String check = (parent.isEmpty() ? "" : parent + "/") + "onlinefix.json";
            if (!isFile(check.toLowerCase(), rarFiles)) return false;
        }
        return true;
    }

    public static boolean checkAreCanListed(String file) {
        String name = Path.of(file).getFileName().toString();
        for (Pattern p : SKIP_PATTERNS) {
            if (p.matcher(name).find()) return false;
        }
        if (NON_EXECUTABLE.matcher(name).find()) return false;
        return true;
    }

    public static boolean checkAreInstallPossible(String file) {
        String ext = getExtension(file);
        if ("rar".equalsIgnoreCase(ext)) return true;
        if ("exe".equalsIgnoreCase(ext)) {
            String parent = Path.of(file).getParent() != null ? Path.of(file).getParent().toString() : ".";
            try (var stream = Files.list(Path.of(parent))) {
                return stream.anyMatch(p -> "ftp".equalsIgnoreCase(getExtension(p.toString())));
            } catch (IOException e) {
                return false;
            }
        }
        return false;
    }

    // -----------------------------------------------------------------------
    // CleanAfterAdd helpers – handles both ToggleButton and CheckBox variants
    // -----------------------------------------------------------------------

    private boolean isCleanAfterAddSelected() {
        if (cleanAfterAdd != null) {
            Object o = cleanAfterAdd.getProperties().get("quUIElement");
            if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) return sw.isSelected();
        }
        if (cleanAfterAddCheck != null) return cleanAfterAddCheck.isSelected();
        return false;
    }

    private void setCleanAfterAddSelected(boolean v) {
        if (cleanAfterAdd != null) {
            Object o = cleanAfterAdd.getProperties().get("quUIElement");
            if (o instanceof com.corkytux.launcher.ui.SwitchComponent sw) sw.setSelected(v);
        }
        if (cleanAfterAddCheck != null) cleanAfterAddCheck.setSelected(v);
    }

    private void setCleanAfterAddEnabled(boolean v) {
        if (cleanAfterAdd != null) cleanAfterAdd.setDisable(!v);
        if (cleanAfterAddCheck != null) cleanAfterAddCheck.setDisable(!v);
    }

    // -----------------------------------------------------------------------
    // Misc helpers
    // -----------------------------------------------------------------------

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
                LOG.debug("hideModal fallback failed", e);
            }
            Stage s = stageOf(mainSelectBox != null ? mainSelectBox : gameParamsBox);
            if (s != null) s.hide();
            else LOG.debug("hideStage: no window to hide");
        });
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            Alert a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
        if (type == Alert.AlertType.ERROR) LOG.error(msg);
        else LOG.warn(msg);
    }

    private void toast(String msg) {
        LOG.info("TOAST: {}", msg);
        showAlert(msg, Alert.AlertType.INFORMATION);
    }

    private boolean confirm(String msg) {
        var a = new Alert(Alert.AlertType.CONFIRMATION, msg, ButtonType.YES, ButtonType.NO);
        a.setHeaderText(null);
        var r = a.showAndWait();
        return r.isPresent() && r.get() == ButtonType.YES;
    }

    private static String getExtension(String filename) {
        int dot = filename.lastIndexOf('.');
        if (dot == -1) return "";
        return filename.substring(dot + 1);
    }

    private static String stripExtension(String filename) {
        int dot = filename.lastIndexOf('.');
        if (dot == -1) return filename;
        return filename.substring(0, dot);
    }

    private static String getBasePathFor(String forWhat) {
        boolean isRoot = "root".equals(System.getProperty("user.name"));
        String userHome = isRoot ? com.corkytux.launcher.modules.FilesWorker.getExpectedHome() : System.getProperty("user.home");
        String defaultDir = Path.of(userHome, ".local/share/CorkyTux", forWhat).toString();
        String userDir = AppModule.getInstance().getLauncher(forWhat + "Path", "User Settings");
        if (userDir == null || userDir.isBlank()) return defaultDir;
        return userDir;
    }

    private static void deleteRecursively(Path path) {
        if (path == null || !Files.exists(path)) return;
        try {
            var pb = new ProcessBuilder("rm", "-rf", path.toString());
            pb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
            pb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
            pb.start().waitFor();
        }
        catch (Exception e) { LOG.warn("deleteRecursively failed {}", path, e); }
    }

    private MainForm findMainForm() {
        try {
            var cls = Class.forName("com.corkytux.launcher.forms.MainForm");
            // try to get singleton via Launcher
            var launcher = Class.forName("com.corkytux.launcher.Launcher");
            var m = launcher.getMethod("getMainForm");
            Object mf = m.invoke(null);
            if (mf instanceof MainForm form) return form;
        } catch (Exception e) { LOG.debug("findMainForm failed", e); }
        return null;
    }

    // For testing / external wiring
    public GameParams getGameParams() { return gameParams; }

    public void setGameParams(GameParams params) {
        this.gameParams.path = params.path;
        this.gameParams.mainFile = params.mainFile;
        this.gameParams.originalFile = params.originalFile;
        this.gameParams.unpackedPath = params.unpackedPath;
        this.gameParams.canInstall = params.canInstall;
        this.gameParams.openedFromAria = params.openedFromAria;
        this.gameParams.skipConfig = params.skipConfig;
    }
    public ListView<Candidate> getListView() { return listView; }
}

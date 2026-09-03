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

package com.corkytux.launcher;

import javafx.application.Application;
import javafx.application.Platform;
import javafx.fxml.FXMLLoader;
import javafx.scene.Node;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.control.ButtonBase;
import javafx.scene.control.Labeled;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.Pane;
import javafx.stage.Modality;
import javafx.stage.Stage;
import javafx.stage.StageStyle;
import com.corkytux.launcher.modules.AppModule;
import com.corkytux.launcher.util.Data;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.InputStream;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Java 25 / JavaFX 21 entry point – mirrors DevelNext Application bootstrap.
 * Loads {@code style.fx.css} (covering .jfx-button, .menu-button, .toggle-button)
 * for every scene, handles Data graphic/image propagation and Panel backgrounds,
 * and manages form lifecycle without auto-opening secondary forms.
 */
public class Launcher extends Application {

    private static final Logger LOG = LoggerFactory.getLogger(Launcher.class);

    private static Launcher instance;
    private static Stage primaryStage;
    private static Object mainFormController;
    public static String[] argv;

    // Form registry – lazily loaded on demand, not auto-opened
    private static final Map<String, Stage> formStages = new HashMap<>();
    private static final Map<String, Object> formControllers = new HashMap<>();
    private static final Map<String, Parent> formRoots = new HashMap<>();

    // Maps FXML file name to controller class when fx:controller missing
    // protonManager alias maps to protonDownloader (same FXML/controller) for MainForm wiring parity
    private static final Map<String, String> CONTROLLER_MAP = Map.ofEntries(
            Map.entry("MainForm", "com.corkytux.launcher.forms.MainForm"),
            Map.entry("newGameConfigurator", "com.corkytux.launcher.forms.NewGameConfigurator"),
            Map.entry("gameSettings", "com.corkytux.launcher.forms.GameSettings"),
            Map.entry("launcherSettings", "com.corkytux.launcher.forms.LauncherSettings"),
            Map.entry("gameRemover", "com.corkytux.launcher.forms.GameRemover"),
            Map.entry("gameStarting", "com.corkytux.launcher.forms.GameStarting"),
            Map.entry("protonDownloader", "com.corkytux.launcher.forms.ProtonDownloader"),
            Map.entry("protonManager", "com.corkytux.launcher.forms.ProtonDownloader"),
            Map.entry("envViewer", "com.corkytux.launcher.forms.EnvViewer"),
            Map.entry("envEditor", "com.corkytux.launcher.forms.EnvEditor"),
            Map.entry("log", "com.corkytux.launcher.forms.LogForm"),
            Map.entry("prototypes", "com.corkytux.launcher.forms.Prototypes")
    );

    // Form metadata from .conf files – backgroundColor and icon 1:1
    private static final Map<String, String> FORM_BACKGROUND = Map.ofEntries(
            Map.entry("MainForm", "#222226"),
            Map.entry("newGameConfigurator", "#222226"),
            Map.entry("gameSettings", "#222226"),
            Map.entry("launcherSettings", "#222226"),
            Map.entry("gameRemover", "#222226"),
            Map.entry("gameStarting", "#000000"),
            Map.entry("protonDownloader", "#222226"),
            Map.entry("envViewer", "#222226"),
            Map.entry("envEditor", "#222226"),
            Map.entry("log", "#222226")
    );

    @Override
    public void start(Stage stage) throws Exception {
        instance = this;
        primaryStage = stage;

        if (Locale.getDefault().getLanguage().equals("tr")) {
            Locale.setDefault(Locale.US);
            System.out.println("Turkish locale detected, switched to en_US.UTF-8");
        }

        String[] rawArgs = getParameters().getRaw().toArray(new String[0]);
        argv = rawArgs;
        AppModule appModule = AppModule.getInstance();
        appModule.doAction(rawArgs);

        // If AppModule entered minimal mode (gameStarting shown), do not create MainForm
        // Minimal detection: argv[1] executable exists -> AppModule shows gameStarting and returns
        Stage gameStartingStage = formStages.get("gameStarting");
        if (gameStartingStage != null || formRoots.containsKey("gameStarting")) {
            // AppModule handled minimal launch – hide placeholder primaryStage if not used
            if (gameStartingStage != null && gameStartingStage != stage) {
                primaryStage = gameStartingStage;
                if (stage.isShowing()) stage.hide();
                else stage.close();
            }
            LOG.info("Launcher started in minimal mode – gameStarting visible, MainForm suppressed");
            return;
        }

        // Duplicate-window guard: AppModule.doAction() already calls showForm("MainForm") on FX thread.
        // If MainForm stage already exists from AppModule, reuse it instead of creating a second Stage.
        if (formStages.containsKey("MainForm") && formRoots.containsKey("MainForm")) {
            Stage existing = formStages.get("MainForm");
            Parent existingRoot = formRoots.get("MainForm");
            if (existing != null && existingRoot != null) {
                if (existing != stage) {
                    // AppModule created a second Stage – reuse it as primaryStage and close placeholder
                    primaryStage = existing;
                    mainFormController = formControllers.get("MainForm");
                    existing.setTitle("CorkyTux — Java +21 Port");
                    if (!existing.isShowing()) existing.show();
                    existing.toFront();
                    existing.requestFocus();
                    if (stage.isShowing()) stage.hide();
                    else stage.close();
                    LOG.info("Launcher started – reused MainForm stage from AppModule, no duplicate (title fixed)");
                    return;
                } else {
                    // Same instance – just fix title
                    stage.setTitle("CorkyTux — Java +21 Port");
                    LOG.info("Launcher started – MainForm already on primaryStage, title fixed");
                    return;
                }
            }
        }

        // Normal path: Load MainForm via registry so stylesheet/Data/Panel handling is uniform
        Parent root = loadFormRoot("MainForm");
        Object controller = formControllers.get("MainForm");
        mainFormController = controller;

        Scene scene = new Scene(root);
        attachStylesheet(scene);
        stage.setTitle("CorkyTux — Java +21 Port");
        applyFormIcon(stage, "MainForm");
        stage.setScene(scene);
        formStages.put("MainForm", stage);
        formRoots.put("MainForm", root);
        stage.show();

        LOG.info("Launcher started – style.fx.css loaded, MainForm visible, secondary forms not auto-opened");
    }

    // -----------------------------------------------------------------------
    // Stylesheet handling – covers .jfx-button, .menu-button, .toggle-button
    // -----------------------------------------------------------------------

    private static void attachStylesheet(Scene scene) {
        URL css = Launcher.class.getResource("/style.fx.css");
        if (css != null) {
            String url = css.toExternalForm();
            if (!scene.getStylesheets().contains(url)) {
                scene.getStylesheets().add(url);
                LOG.debug("Attached style.fx.css to scene {}", scene);
            }
        } else {
            LOG.warn("style.fx.css not found on classpath – theme will be missing");
        }
        // Attach accent color override if user has changed from default
        attachAccentOverride(scene);
    }

    private static void attachAccentOverride(Scene scene) {
        try {
            var cssFile = java.nio.file.Path.of(
                    com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".config", "CorkyTux", "accent-override.css");
            if (java.nio.file.Files.exists(cssFile)) {
                String url = cssFile.toUri().toString();
                var sheets = scene.getStylesheets();
                sheets.removeIf(s -> s.contains("accent-override"));
                sheets.add(url);
                LOG.debug("Attached accent override CSS");
            }
        } catch (Exception e) {
            LOG.debug("No accent override CSS", e);
        }
    }

    // -----------------------------------------------------------------------
    // Data / Panel handling
    // -----------------------------------------------------------------------

    /**
     * Applies Data graphic/image to sibling controls and ensures Panel backgrounds
     * are rendered. Scans for {@link Data} nodes where id="data-<targetId>" and
     * graphic=".data/img/..." or image="...".
     */
    static void applyDataAndPanels(Parent root) {
        if (root == null) return;
        // Find all Data nodes recursively
        var allData = root.lookupAll(".data");
        // Also lookup by type via traversal
        collectDataNodes(root).forEach(data -> {
            String dataId = data.getId();
            if (dataId == null || !dataId.startsWith("data-")) return;
            String targetId = dataId.substring(5);
            Node target = root.lookup("#" + targetId);
            // Fallback: search in whole scene graph including nested
            if (target == null) {
                // try deep search via traversal
                target = findById(root, targetId);
            }
            if (target == null) {
                LOG.trace("Data {} has no target {}", dataId, targetId);
                return;
            }
            String graphic = data.getGraphic();
            if (graphic != null && !graphic.isBlank()) {
                applyGraphicToNode(target, graphic);
            }
            String image = data.getImage();
            if (image != null && !image.isBlank()) {
                applyImageToNode(target, image);
            }
        });

        // Panel backgrounds: original <Panel backgroundColor="#333333" borderRadius="15">
        // were converted to AnchorPane with inline style. Ensure Panel/Data backgrounds are correctly styled.
        // GameSettings uses AnchorPane ids panel / panelAlt that should have #333333 radius 15.
        for (String panelId : new String[]{"panel", "panelAlt"}) {
            Node n = root.lookup("#" + panelId);
            if (n == null) n = findById(root, panelId);
            if (n instanceof Pane pane) {
                String cur = pane.getStyle();
                if (cur == null || !cur.contains("-fx-background-color")) {
                    pane.setStyle("-fx-background-color:#333333;-fx-background-radius:15px;");
                    LOG.trace("Applied Panel background to #{}", panelId);
                }
            }
        }
        // Root background #222226 if not already set – per FORM_BACKGROUND
        if (root instanceof Pane pane) {
            String style = pane.getStyle();
            if (style == null || !style.contains("-fx-background-color")) {
                // Apply per-form background if known
                // We don't know form name here, so leave as is – Stage icon/background handled separately
            }
        }
    }

    private static void applyGraphicToNode(Node target, String graphicPath) {
        if (!(target instanceof Labeled labeled)) return;
        // graphicPath like ".data/img/add.png" or "res://.data/img/add.png"
        String res = graphicPath.replace("res://", "").replace(".data/img/", "/img/").replace(".data/", "/.data/");
        // Try multiple resource locations
        Image img = loadImageResource(res);
        if (img == null) img = loadImageResource(graphicPath);
        if (img == null) img = loadImageResource("/.data/img/" + Path.of(graphicPath).getFileName());
        if (img == null) {
            // Try banners/icons dir if path hints
            if (graphicPath.contains("banners") || graphicPath.contains("icons")) {
                Path p = Path.of(com.corkytux.launcher.modules.FilesWorker.getExpectedHome(), ".config/CorkyTux", graphicPath.replace(".data/", "").replace("res://", ""));
                if (Files.isRegularFile(p)) {
                    try { img = new Image(p.toUri().toString()); } catch (Exception ignored) {}
                }
            }
        }
        if (img != null) {
            var iv = new ImageView(img);
            iv.setFitWidth(20);
            iv.setFitHeight(20);
            iv.setPreserveRatio(true);
            // Keep original size for small icons like 14x14 edit
            if (graphicPath.contains("edit.png") || graphicPath.contains("ok.png")) {
                iv.setFitWidth(14); iv.setFitHeight(14);
            }
            labeled.setGraphic(iv);
            LOG.trace("Applied graphic {} to #{}", graphicPath, target.getId());
        } else {
            LOG.debug("Failed to load graphic resource {} for #{}", graphicPath, target.getId());
        }
    }

    private static void applyImageToNode(Node target, String imagePath) {
        if (!(target instanceof ImageView iv)) return;
        Image img = loadImageResource(imagePath);
        if (img == null) img = loadImageResource("/.data/img/" + Path.of(imagePath).getFileName());
        if (img == null && Files.isRegularFile(Path.of(imagePath))) {
            try { img = new Image(Path.of(imagePath).toUri().toString()); } catch (Exception ignored) {}
        }
        if (img != null) {
            iv.setImage(img);
            LOG.trace("Applied image {} to #{}", imagePath, target.getId());
        }
    }

    private static Image loadImageResource(String path) {
        if (path == null) return null;
        // Normalize: ensure leading slash
        String p = path;
        if (!p.startsWith("/") && !p.startsWith(".")) p = "/" + p;
        // Try as classpath resource
        try (InputStream is = Launcher.class.getResourceAsStream(p)) {
            if (is != null) return new Image(is);
        } catch (Exception ignored) {}
        // Try without leading slash
        try (InputStream is = Launcher.class.getResourceAsStream(p.replaceFirst("^/", ""))) {
            if (is != null) return new Image(is);
        } catch (Exception ignored) {}
        // Try /img fallback
        String fileName = Path.of(p).getFileName() != null ? Path.of(p).getFileName().toString() : null;
        if (fileName != null) {
            try (InputStream is = Launcher.class.getResourceAsStream("/img/" + fileName)) {
                if (is != null) return new Image(is);
            } catch (Exception ignored) {}
        }
        return null;
    }

    private static java.util.List<Data> collectDataNodes(Parent root) {
        var list = new java.util.ArrayList<Data>();
        collectDataRecursive(root, list);
        return list;
    }

    private static void collectDataRecursive(Node node, java.util.List<Data> out) {
        if (node instanceof Data d) out.add(d);
        if (node instanceof Parent p) {
            for (Node child : p.getChildrenUnmodifiable()) {
                collectDataRecursive(child, out);
            }
        }
        // Also check Pane children
        if (node instanceof Pane pane) {
            for (Node child : pane.getChildren()) {
                // Avoid double count if already via getChildrenUnmodifiable
                if (child instanceof Data && !out.contains(child)) out.add((Data) child);
                else if (child instanceof Parent) collectDataRecursive(child, out);
            }
        }
    }

    private static Node findById(Parent root, String id) {
        if (id.equals(root.getId())) return root;
        for (Node child : root.getChildrenUnmodifiable()) {
            if (id.equals(child.getId())) return child;
            if (child instanceof Parent p) {
                Node found = findById(p, id);
                if (found != null) return found;
            }
        }
        if (root instanceof Pane pane) {
            for (Node child : pane.getChildren()) {
                if (id.equals(child.getId())) return child;
                if (child instanceof Parent pp) {
                    Node f = findById(pp, id);
                    if (f != null) return f;
                }
            }
        }
        return null;
    }

    private static void applyFormIcon(Stage stage, String formName) {
        String iconPath = "/img/corkytux.png";
        try (InputStream is = Launcher.class.getResourceAsStream(iconPath)) {
            if (is != null) stage.getIcons().add(new Image(is));
        } catch (Exception e) {
            LOG.debug("Failed to set icon for {}", formName, e);
        }
    }

    // -----------------------------------------------------------------------
    // Form loading registry
    // -----------------------------------------------------------------------

    private static Parent loadFormRoot(String formName) throws Exception {
        // Alias: protonManager -> protonDownloader (same FXML, wired via MainForm for Proton stage)
        String effectiveName = "protonManager".equals(formName) ? "protonDownloader" : formName;
        String fxmlPath = "/fxml/" + effectiveName + ".fxml";
        URL fxml = Launcher.class.getResource(fxmlPath);
        if (fxml == null) throw new IllegalArgumentException("FXML not found: " + fxmlPath);
        // Correct FXMLLoader usage – fx:controller in FXML is authoritative (all secondary forms have it).
        // CONTROLLER_MAP is fallback only for controller-less FXML (e.g. prototypes).
        FXMLLoader loader = new FXMLLoader(fxml);
        String controllerClass = CONTROLLER_MAP.get(formName);
        if (controllerClass != null) {
            loader.setControllerFactory(cls -> {
                try {
                    return cls.getDeclaredConstructor().newInstance();
                } catch (Exception e) {
                    LOG.warn("Controller factory failed for {}", cls, e);
                    throw new RuntimeException(e);
                }
            });
        }

        Parent root = loader.load();
        Object controller = loader.getController();
        if (controller != null) {
            formControllers.put(formName, controller);
            // For alias, also register under effectiveName for stage lookup parity
            if (!formName.equals(effectiveName)) formControllers.put(effectiveName, controller);
            LOG.debug("Loaded {} controller={} via fx:controller", formName, controller.getClass().getName());
        } else {
            // Controller-less form (prototypes) – instantiate from map if available
            if (controllerClass != null) {
                try {
                    Class<?> cc = Class.forName(controllerClass);
                    Object ctrl = cc.getDeclaredConstructor().newInstance();
                    formControllers.put(formName, ctrl);
                    loader.setController(ctrl);
                    LOG.debug("No fx:controller for {} – fallback controller {}", formName, controllerClass);
                } catch (Exception e) {
                    LOG.debug("No controller for {} – FXML may be controller-less (prototypes)", formName);
                }
            } else {
                LOG.debug("No controller for {} – FXML may be controller-less", formName);
            }
        }
        formRoots.put(formName, root);
        if (!formName.equals(effectiveName)) formRoots.put(effectiveName, root);

        // Apply Data graphics and Panel backgrounds (Panel -> AnchorPane conversion parity)
        applyDataAndPanels(root);

        // Ensure root has background color #222226 if form expects it and not already styled
        String bg = FORM_BACKGROUND.get(formName);
        if (bg == null) bg = FORM_BACKGROUND.get(effectiveName);
        if (bg != null && root instanceof Pane pane) {
            String cur = pane.getStyle();
            if (cur == null || !cur.contains("-fx-background-color")) {
                pane.setStyle((cur != null ? cur + ";" : "") + "-fx-background-color:" + bg + ";");
            }
        }

        return root;
    }

    private static Stage createStageForForm(String formName, Parent root) {
        String effectiveName = "protonManager".equals(formName) ? "protonDownloader" : formName;
        Stage stage = new Stage();
        if (primaryStage != null) stage.initOwner(primaryStage);
        stage.initModality(Modality.WINDOW_MODAL);

        boolean undecorated = "gameStarting".equals(formName);
        if (undecorated) {
            stage.initStyle(StageStyle.TRANSPARENT);
        }

        Scene scene;
        if (undecorated) {
            scene = new Scene(root, javafx.scene.paint.Color.TRANSPARENT);
            root.setStyle("-fx-background-color:transparent;");
        } else {
            scene = new Scene(root);
        }
        attachStylesheet(scene);
        stage.setScene(scene);
        String bg = FORM_BACKGROUND.get(formName);
        if (bg == null) bg = FORM_BACKGROUND.get(effectiveName);
        if (bg != null && !undecorated) {
            try { scene.setFill(javafx.scene.paint.Color.web(bg)); } catch (Exception ignored) {}
        }
        applyFormIcon(stage, effectiveName);
        // Titles from .conf – localized via Launcher title mapping; include protonManager alias
        Map<String, String> titles = Map.of(
                "newGameConfigurator", "Add Game",
                "gameSettings", "Game Settings",
                "launcherSettings", "Launcher Settings",
                "gameRemover", "Remove Game",
                "envViewer", "Environment Variables",
                "envEditor", "Edit Variable",
                "log", "Game Log",
                "protonDownloader", "Proton Manager",
                "protonManager", "Proton Manager"
        );
        String t = titles.get(formName);
        if (t == null) t = titles.get(effectiveName);
        if (t != null) stage.setTitle(t);
        else stage.setTitle(formName);

        // Esc closes stage – mirrors doKeyUpEsc (hide on Esc, mirrors doKeyUpEsc/doHide)
        scene.setOnKeyReleased(e -> {
            if (e.getCode() == javafx.scene.input.KeyCode.ESCAPE) stage.hide();
        });

        formStages.put(formName, stage);
        // Alias handling: keep protonManager/protonDownloader in sync (same Stage instance)
        String eff = "protonManager".equals(formName) ? "protonDownloader" : "protonDownloader".equals(formName) ? "protonManager" : null;
        if (eff != null) formStages.put(eff, stage);
        return stage;
    }

    // -----------------------------------------------------------------------
    // Public API – used via reflection from MainForm, AppModule, etc.
    // -----------------------------------------------------------------------

    public static Launcher getInstance() { return instance; }
    public static Stage getPrimaryStage() { return primaryStage; }
    public static Object getMainForm() { return mainFormController; }
    public static Stage getMainStage() { return primaryStage; }

    public static Object getForm(String formName) {
        Object v = formControllers.get(formName);
        if (v == null && "protonManager".equals(formName)) v = formControllers.get("protonDownloader");
        if (v == null && "protonDownloader".equals(formName)) v = formControllers.get("protonManager");
        return v;
    }

    public static Object getFormController(String formName) {
        Object v = formControllers.get(formName);
        if (v == null && "protonManager".equals(formName)) v = formControllers.get("protonDownloader");
        if (v == null && "protonDownloader".equals(formName)) v = formControllers.get("protonManager");
        return v;
    }

    public static Parent getFormRoot(String formName) {
        Parent v = formRoots.get(formName);
        if (v == null && "protonManager".equals(formName)) v = formRoots.get("protonDownloader");
        if (v == null && "protonDownloader".equals(formName)) v = formRoots.get("protonManager");
        return v;
    }

    public static synchronized void showForm(String formName) {
        Runnable task = () -> {
            try {
                Stage stage = formStages.get(formName);
                Parent root = formRoots.get(formName);
                if (stage == null || root == null) {
                    root = loadFormRoot(formName);
                    stage = createStageForForm(formName, root);
                }
                // For dialogs that were hidden, re-apply Data after possible state changes
                applyDataAndPanels(root);
                attachStylesheet(stage.getScene());
                stage.show();
                stage.toFront();
                stage.requestFocus();
                // Call doShow() on controller if available (used by GameStarting to launch game)
                Object ctrl = formControllers.get(formName);
                if (ctrl != null) {
                    try {
                        var m = ctrl.getClass().getMethod("doShow");
                        m.invoke(ctrl);
                    } catch (NoSuchMethodException ignored) {} catch (Exception e) {
                        LOG.debug("doShow failed for {}", formName, e);
                    }
                }
                LOG.info("showForm {}", formName);
            } catch (Exception e) {
                LOG.error("showForm failed for {}", formName, e);
            }
        };
        if (Platform.isFxApplicationThread()) task.run();
        else Platform.runLater(task);
    }

    public static synchronized void showNewForm(String formName) {
        Runnable task = () -> {
            try {
                // Always create new instance
                Parent root = loadFormRoot(formName);
                Stage stage = createStageForForm(formName, root);
                // Replace registry entry
                formStages.put(formName, stage);
                stage.show();
                stage.toFront();
                LOG.info("showNewForm {}", formName);
            } catch (Exception e) {
                LOG.error("showNewForm failed for {}", formName, e);
            }
        };
        if (Platform.isFxApplicationThread()) task.run();
        else Platform.runLater(task);
    }

    public static synchronized Object showFormAndWait(String formName) {
        return showFormAndWait(formName, false);
    }

    public static synchronized Object showFormAndWait(String formName, boolean isNew) {
        final Object[] result = new Object[1];
        final Exception[] err = new Exception[1];
        // Must run on FX thread but block – use CountDownLatch
        if (Platform.isFxApplicationThread()) {
            try {
                Parent root;
                Stage stage;
                if (isNew || !formStages.containsKey(formName)) {
                    root = loadFormRoot(formName);
                    stage = createStageForForm(formName, root);
                    formStages.put(formName, stage);
                } else {
                    stage = formStages.get(formName);
                    root = formRoots.get(formName);
                }
                attachStylesheet(stage.getScene());
                stage.showAndWait();
                result[0] = formControllers.get(formName);
            } catch (Exception e) {
                err[0] = e;
                LOG.error("showFormAndWait failed {}", formName, e);
            }
        } else {
            var latch = new java.util.concurrent.CountDownLatch(1);
            Platform.runLater(() -> {
                try {
                    Parent root;
                    Stage stage;
                    if (isNew || !formStages.containsKey(formName)) {
                        root = loadFormRoot(formName);
                        stage = createStageForForm(formName, root);
                        formStages.put(formName, stage);
                    } else {
                        stage = formStages.get(formName);
                        root = formRoots.get(formName);
                    }
                    attachStylesheet(stage.getScene());
                    stage.showAndWait();
                    result[0] = formControllers.get(formName);
                } catch (Exception e) {
                    err[0] = e;
                    LOG.error("showFormAndWait failed {}", formName, e);
                } finally {
                    latch.countDown();
                }
            });
            try { latch.await(60, java.util.concurrent.TimeUnit.SECONDS); } catch (InterruptedException ie) { Thread.currentThread().interrupt(); }
        }
        if (err[0] != null) throw new RuntimeException(err[0]);
        return result[0];
    }

    /**
     * Mirrors quUI::showFormAndFocus – shows form and requests focus.
     */
    public static Object showFormAndFocus(String formName, boolean isNew) {
        Object form = isNew ? null : formControllers.get(formName);
        if (form == null) {
            try {
                Parent root = loadFormRoot(formName);
                Stage stage = createStageForForm(formName, root);
                formStages.put(formName, stage);
                Runnable showTask = () -> { stage.show(); stage.requestFocus(); };
                if (Platform.isFxApplicationThread()) showTask.run();
                else Platform.runLater(showTask);
                return formControllers.get(formName);
            } catch (Exception e) {
                LOG.error("showFormAndFocus failed {}", formName, e);
                return null;
            }
        } else {
            showForm(formName);
            return form;
        }
    }

    public static Object showFormAndFocus(String formName) {
        return showFormAndFocus(formName, false);
    }

    public static Stage getStage(String formName) {
        Stage v = formStages.get(formName);
        if (v == null && "protonManager".equals(formName)) v = formStages.get("protonDownloader");
        if (v == null && "protonDownloader".equals(formName)) v = formStages.get("protonManager");
        return v;
    }

    public static void main(String[] args) {
        System.setProperty("prism.forceGPU", "true");
        launch(args);
    }
}

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
import com.corkytux.launcher.modules.Localization;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.Node;
import javafx.scene.control.Label;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.AnchorPane;
import javafx.scene.layout.Pane;
import javafx.scene.shape.Rectangle;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ResourceBundle;

/**
 * Java 25 / JavaFX 21 port of {@code gameStarting.php} (54 lines).
 *
 * <p>Minimal-mode form shown when launcher is invoked with a game name argument
 * ({@code corkytux "GameName"}). It resolves and launches the
 * game via {@link FilesWorker}, hiding itself after 5 seconds and shutting down
 * if no process could be generated. The background banner is clipped with a
 * 25-px rounded rectangle and sourced from {@code Games.ini} {@code banner}
 * or {@code /img/noBanner.png}.</p>
 *
 * <p>FXML: {@code /fxml/gameStarting.fxml} – fx:ids {@code background}
 * ({@link Pane} or {@link ImageView}), {@code label}.</p>
 */
public class GameStarting implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(GameStarting.class);

    @FXML private Label label;
    @FXML private ImageView backgroundImage;
    @FXML private AnchorPane root;

    private final AppModule appModule = AppModule.getInstance();
    private final Localization loc = Localization.getInstance();

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        initLabel();
        initBackground();
    }

    // -----------------------------------------------------------------------
    // show – mirrors @event show
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code @event show}.
     * <pre>
     * new Thread(function (){
     *   $process = FilesWorker::generateProcess($GLOBALS['argv'][1]);
     *   if ($process==null){ app()->shutdown(); return; }
     *   waitAsync('5s',function (){$this->hide();});
     *   FilesWorker::run($process,$GLOBALS['argv'][1]);
     * })->start();
     * </pre>
     *
     * <p>Called by FXML {@code onShown} or manually after stage is shown.
     * The game name is taken from application parameters – stored in
     * {@code Launcher.argv[1]} or system property {@code corkytux.gameName}.</p>
     */
    @FXML
    public void handleShow() {
        String gameName = resolveArgvGameName();
        if (gameName == null) {
            LOG.warn("gameStarting: no game name in argv");
            return;
        }
        Thread.ofVirtual().start(() -> {
            var pb = FilesWorker.generateProcess(gameName);
            if (pb == null) {
                LOG.info("generateProcess returned null for {} – shutting down", gameName);
                Platform.runLater(() -> {
                    try {
                        javafx.application.Platform.exit();
                    } catch (Exception e) {
                        LOG.debug("Platform.exit failed", e);
                    }
                    System.exit(0);
                });
                return;
            }
            // waitAsync('5s', hide) – mirrors closing splash after 5 seconds
            Thread.ofVirtual().start(() -> {
                try { Thread.sleep(5000); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
                Platform.runLater(this::hideStage);
            });

            try {
                var process = pb.start();
                FilesWorker.run(process, gameName, false);
            } catch (Exception e) {
                LOG.error("Failed to run game {}", gameName, e);
            }
        });
    }

    public void doShow() { handleShow(); }

    // -----------------------------------------------------------------------
    // construct handlers – mirrors PHP @event construct methods
    // -----------------------------------------------------------------------

    private void initLabel() {
        if (label == null) return;
        String gameName = resolveArgvGameName();
        String safeName = gameName != null ? gameName : "Game";
        // GAMESTARTER.STARTING = "%s is starting" – mirrors sprintf
        String text = String.format(loc.get("GAMESTARTER.STARTING"), safeName);
        label.setText(text);
    }

    private void initBackground() {
        // Mirrors doBackgroundConstruct – clip with arc 25 and set banner image
        if (root != null) {
            var clip = new Rectangle();
            // size will be bound to background size after layout
            clip.widthProperty().bind(root.widthProperty());
            clip.heightProperty().bind(root.heightProperty());
            clip.setArcWidth(25);
            clip.setArcHeight(25);
            root.setClip(clip);

            String gameName = resolveArgvGameName();
            String bannerPath = null;
            if (gameName != null) bannerPath = appModule.getGame("banner", gameName);

            Image banner = null;
            if (bannerPath != null && Files.isRegularFile(Path.of(bannerPath))) {
                try { banner = new Image(Path.of(bannerPath).toUri().toString()); }
                catch (Exception e) { LOG.debug("banner load failed {}", bannerPath, e); }
            }
            if (banner == null) {
                try (var is = getClass().getResourceAsStream("/img/noBanner.png")) {
                    if (is != null) banner = new Image(is);
                } catch (Exception e) { LOG.debug("fallback banner failed", e); }
            }
            if (banner != null) {
                // If background is a Pane we set via CSS background image
                final Image bgImg = banner;
                Platform.runLater(() -> {
                    // Use inline style for background image
                    String url = bgImg.getUrl();
                    if (url != null) {
                        root.setStyle("-fx-background-image: url('" + url + "'); -fx-background-size: cover; -fx-background-radius: 25;");
                    }
                });
            }
        }
        if (backgroundImage != null) {
            var clip = new Rectangle();
            clip.widthProperty().bind(backgroundImage.fitWidthProperty());
            clip.heightProperty().bind(backgroundImage.fitHeightProperty());
            clip.setArcWidth(25);
            clip.setArcHeight(25);
            backgroundImage.setClip(clip);

            String gameName = resolveArgvGameName();
            String bannerPath = gameName != null ? appModule.getGame("banner", gameName) : null;
            Image img = null;
            if (bannerPath != null && Files.isRegularFile(Path.of(bannerPath))) {
                try { img = new Image(Path.of(bannerPath).toUri().toString()); }
                catch (Exception e) { LOG.debug("banner load failed", e); }
            }
            if (img == null) {
                try (var is = getClass().getResourceAsStream("/img/noBanner.png")) {
                    if (is != null) img = new Image(is);
                } catch (Exception ex) { LOG.debug("fallback banner failed", ex); }
            }
            if (img != null) backgroundImage.setImage(img);
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private String resolveArgvGameName() {
        // Try system property first (set by Launcher main)
        String fromProp = System.getProperty("corkytux.gameName");
        if (fromProp != null && !fromProp.isBlank()) return fromProp.trim();
        // Try Launcher.argv via reflection
        try {
            var cls = Class.forName("com.corkytux.launcher.Launcher");
            var field = cls.getDeclaredField("argv");
            field.setAccessible(true);
            var argv = (String[]) field.get(null);
            if (argv != null) {
                // Try argv[1] (PHP-style) then argv[0] (JavaFX-style)
                if (argv.length > 1 && argv[1] != null) return argv[1];
                if (argv.length > 0 && argv[0] != null) return argv[0];
            }
        } catch (Exception e) { LOG.debug("resolve argv failed", e); }
        return null;
    }

    private void hideStage() {
        Node n = root != null ? root : label;
        if (n == null || n.getScene() == null) return;
        var w = n.getScene().getWindow();
        if (w instanceof Stage s) s.hide();
        else w.hide();
    }
}

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

import com.corkytux.launcher.modules.Localization;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.Node;
import javafx.scene.control.Alert;
import javafx.scene.control.Button;
import javafx.scene.control.ButtonType;
import javafx.scene.control.Label;
import javafx.scene.control.TextArea;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
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
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ResourceBundle;

/**
 * Java 25 / JavaFX 21 port of {@code log.php} (103 lines).
 *
 * <p>Note: PHP class is {@code log} (lowercase) which is not a valid Java type name.
 * Java port is named {@code LogForm} and FXML should declare
 * {@code fx:controller="com.corkytux.launcher.forms.LogForm"}; if legacy FXML
 * still uses {@code log}, a {@code <fx:define>} alias or Launcher reflection
 * fallback handles it.</p>
 *
 * <p>Behaviour mirrored in full:
 * <ul>
 *   <li>{@code @event hide} clears {@code textArea} and calls {@code free()}</li>
 *   <li>{@code label} / {@code labelAlt} set to {@code LOGFORM.HEADER/SUBHEADER}</li>
 *   <li>{@code button3} (save) with save.png icon + {@code SAVE}, action writes
 *       {@code ~/Documents/CorkyTux Logs/<gameName> yyyy-MM-dd HH:mm.log} then {@code xdg-open}</li>
 *   <li>{@code button} (telegram) with telegram.png</li>
 *   <li>{@code buttonAlt} with save.png + {@code MAINFORM.STOP}, hides form</li>
 *   <li>{@code Esc} hides</li>
 * </ul>
 * </p>
 *
 * <p>FXML ids: {@code textArea} ({@link TextArea}), {@code label}/{@code labelAlt},
 * {@code button3}, {@code button}, {@code buttonAlt}.</p>
 */
public class LogForm implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(LogForm.class);

    @FXML private TextArea textArea;
    @FXML private Label label;
    @FXML private Label labelAlt;
    @FXML private Button button3;   // save to Documents
    @FXML private Button button;    // github issues
    @FXML private Button buttonAlt; // stop / hide
    @FXML private VBox root;

    // Properties bag for data('gameName') – mirrors PHP $this->data('gameName')
    private String gameName;

    private final Localization loc = Localization.getInstance();

    // Buffer for log lines appended before FXML textArea is injected – mirrors PHP where
    // app()->form('log')->textArea exists even when form hidden. This ensures Wine/Proton
    // STDOUT/STDERR hooked in FilesWorker are not lost when debug window not yet shown.
    private final java.util.List<String> pendingAppend = new java.util.ArrayList<>();
    private static volatile LogForm lastInstance;

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        lastInstance = this;
        initLabels();
        initButtons();
        if (root != null) {
            root.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.getCode() == KeyCode.ESCAPE) handleHide();
            });
        }
        // Flush any pending appends from FilesWorker hooks that arrived before FXML injection
        if (!pendingAppend.isEmpty() && textArea != null) {
            for (String p : pendingAppend) {
                textArea.appendText(p + "\n");
            }
            pendingAppend.clear();
            LOG.debug("Flushed {} pending log lines after FXML init", pendingAppend.size());
        }
    }

    /** Accessor for static buffer fallback when LogForm not yet instantiated. */
    public static LogForm getLastInstance() { return lastInstance; }

    private void initLabels() {
        if (label != null) label.setText(loc.get("LOGFORM.HEADER"));
        if (labelAlt != null) labelAlt.setText(loc.get("LOGFORM.SUBHEADER"));
    }

    private void initButtons() {
        if (button3 != null) {
            var iv = imageView("/img/save.png", 20);
            if (iv != null) button3.setGraphic(iv);
            button3.setText(loc.get("SAVE"));
            button3.setOnAction(this::handleButton3Action);
        }
        if (button != null) {
            var iv = imageView("/img/github.png", 20);
            if (iv != null) button.setGraphic(iv);
            button.setOnAction(this::handleButtonAction);
        }
        if (buttonAlt != null) {
            var iv = imageView("/img/save.png", 20);
            if (iv != null) buttonAlt.setGraphic(iv);
            buttonAlt.setText(loc.get("MAINFORM.STOP"));
            buttonAlt.setOnAction(this::handleButtonAltAction);
        }
    }

    // -----------------------------------------------------------------------
    // Public API – gameName data & textArea access
    // -----------------------------------------------------------------------

    public void setGameName(String name) { this.gameName = name; }
    public String getGameName() { return gameName; }

    /** Mirrors PHP data('gameName') setter. */
    public void setDataGameName(String name) { this.gameName = name; }

    public TextArea getTextArea() { return textArea; }

    /**
     * Appends debug text – used by {@link com.corkytux.launcher.modules.FilesWorker}
     * debug hook (mirrors PHP FilesWorker::debug appending to log::textArea).
     * Buffers when textArea not yet injected so Wine rendering/debug logs are not lost
     * before the log window is shown (fixes "debug log not showing").
     */
    public void appendText(String text) {
        if (textArea == null) {
            synchronized (pendingAppend) {
                pendingAppend.add(text);
            }
            LOG.debug("appendText buffered before FXML init: {}", text);
            return;
        }
        // Ensure pending buffer flushed first
        synchronized (pendingAppend) {
            if (!pendingAppend.isEmpty()) {
                Platform.runLater(() -> {
                    for (String p : pendingAppend) textArea.appendText(p + "\n");
                    pendingAppend.clear();
                });
            }
        }
        Platform.runLater(() -> textArea.appendText(text + "\n"));
    }

    /**
     * Prepends text (used by debug() to insert header info at top like PHP:
     * {@code textArea->text = info + "\n" + existing}). Buffers if not yet ready.
     */
    public void prependText(String text) {
        if (textArea == null) {
            synchronized (pendingAppend) {
                pendingAppend.add(0, text);
            }
            LOG.debug("prependText buffered before FXML init");
            return;
        }
        Platform.runLater(() -> {
            String existing = textArea.getText() != null ? textArea.getText() : "";
            textArea.setText(text + "\n" + existing);
        });
    }

    // -----------------------------------------------------------------------
    // Handlers – mirrors PHP @event methods
    // -----------------------------------------------------------------------

    /**
     * Mirrors {@code @event hide}: clears textArea and free().
     * In JavaFX {@code free()} maps to closing stage resources.
     */
    @FXML
    private void handleHide() {
        if (textArea != null) textArea.clear();
        hideStage();
        // free – no explicit resource; stage will be GC'd
        LOG.debug("LogForm hidden and cleared");
    }

    public void doHide() { handleHide(); }

    @FXML
    private void handleButton3Action(javafx.event.ActionEvent e) {
        // Mirrors doButton3Action:
        // $documents = str::trim(execute('xdg-user-dir DOCUMENTS',true)->getInput()->readFully());
        // $fileName = $this->data('gameName').' '.Time::now()->toString('yyyy-MM-dd HH:mm').'.log';
        // fs::makeDir("$documents/CorkyTux Logs"); file_put_contents(...); open(...)
        String documents = execReadFully("xdg-user-dir DOCUMENTS");
        if (documents == null || documents.isBlank()) documents = System.getProperty("user.home") + "/Documents";
        else documents = documents.trim();

        String name = (gameName != null && !gameName.isBlank()) ? gameName : "Game";
        String timestamp = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm"));
        String fileName = name + " " + timestamp + ".log";

        Path dir = Path.of(documents, "CorkyTux Logs");
        try { Files.createDirectories(dir); } catch (IOException ex) { LOG.warn("create dir failed {}", dir, ex); }

        Path file = dir.resolve(fileName);
        String content = textArea != null ? textArea.getText() : "";
        try {
            Files.writeString(file, content != null ? content : "", StandardCharsets.UTF_8);
            LOG.info("Log saved to {}", file);
        } catch (IOException ex) {
            LOG.error("Failed to save log {}", file, ex);
            showAlert("Failed to save log: " + ex.getMessage(), Alert.AlertType.ERROR);
            return;
        }
        // open folder – mirrors open("$documents/CorkyTux Logs") – Java 25: use correct user (the user not root) to avoid thumbnail admin error
        try { com.corkytux.launcher.modules.FilesWorker.openWithXdgOpen(dir.toString()); }
        catch (IOException ex) { LOG.warn("xdg-open failed {}", dir, ex); }
        catch (Exception ex) { LOG.warn("xdg-open failed {}", dir, ex); }
    }

    @FXML
    private void handleButtonAction(javafx.event.ActionEvent e) {
        try { com.corkytux.launcher.modules.FilesWorker.openWithXdgOpen("https://github.com/Matts-lab69/corkytux/issues"); }
        catch (IOException ex) { LOG.warn("xdg-open github issues failed", ex); }
        catch (Exception ex) { LOG.warn("xdg-open github issues failed", ex); }
    }

    @FXML
    private void handleButtonAltAction(javafx.event.ActionEvent e) {
        handleHide();
    }

    @FXML
    private void handleKeyEsc(KeyEvent e) {
        if (e.getCode() == KeyCode.ESCAPE) handleHide();
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private String execReadFully(String command) {
        try {
            var proc = new ProcessBuilder("bash", "-c", command).start();
            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line = reader.readLine();
                proc.waitFor();
                return line;
            }
        } catch (Exception ex) {
            LOG.debug("exec failed {}", command, ex);
            return null;
        }
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

    private void hideStage() {
        Node n = root != null ? root : textArea;
        if (n == null || n.getScene() == null) return;
        var w = n.getScene().getWindow();
        if (w instanceof Stage s) s.hide();
        else w.hide();
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            var a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
    }
}

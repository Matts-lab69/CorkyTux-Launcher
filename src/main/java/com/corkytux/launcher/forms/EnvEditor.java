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
import javafx.scene.control.Alert;
import javafx.scene.control.Button;
import javafx.scene.control.ButtonType;
import javafx.scene.control.Label;
import javafx.scene.control.TextField;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.layout.VBox;
import javafx.stage.Popup;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.URL;
import java.util.ResourceBundle;
import java.util.Set;

/**
 * Java 25 / JavaFX 21 port of {@code envEditor.php} (136 lines).
 *
 * <p>Popup editor for a single environment variable (variable name + value).
 * Shown inside a {@link EnvViewer} popover – mirrors PHP {@code UXPopOver}
 * behavior. The form is embedded via {@code showInFragment} in
 * {@code envViewer::doConstruct} and shares its {@code popOver} reference.</p>
 *
 * <p>Validation mirrors PHP exactly:
 * <ul>
 *   <li>Empty variable → {@code ENVEDITOR.NOVARIABLE}</li>
 *   <li>{@code LD_PRELOAD} with Steam Overlay enabled → {@code ENVEDITOR.STEAMOVERLAY}</li>
 *   <li>Blacklisted {@code WINEDLLOVERRIDES}, {@code PROTON_ENABLE_WAYLAND}, {@code PROTON_USE_WINED3D}
 *       → {@code ENVEDITOR.BLACKLISTED}</li>
 * </ul>
 * On success the entry is upserted into {@link EnvViewer}'s table and
 * {@code saveButton} visibility is toggled based on dirty check vs
 * {@code originalValues}.</p>
 *
 * <p>FXML ids: {@code env} ({@link TextField} variable), {@code value} ({@link TextField}),
 * {@code saveButton}, {@code label}/{@code labelAlt}.</p>
 */
public class EnvEditor implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(EnvEditor.class);

    @FXML private TextField env;
    @FXML private TextField value;
    @FXML private Button saveButton;
    @FXML private Label label;
    @FXML private Label labelAlt;
    @FXML private VBox root;

    /** Mirrors PHP {@code $popOver} – set by {@link EnvViewer} during construct. */
    private Popup popOver;
    /** Alternative Stage-based popover when Popup not used. */
    private javafx.stage.PopupWindow popupWindow;
    /** Generic popover handle for hide(). */
    private Object popOverHandle;

    private final Localization loc = Localization.getInstance();

    private static final Set<String> BLACKLIST = Set.of(
            "WINEDLLOVERRIDES",
            "PROTON_ENABLE_WAYLAND",
            "PROTON_USE_WINED3D"
    );

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        if (label != null) label.setText(loc.get("ENVEDITOR.VARIABLE"));
        if (labelAlt != null) labelAlt.setText(loc.get("ENVEDITOR.VALUE"));
        if (saveButton != null) {
            saveButton.setText(loc.get("SAVE"));
            saveButton.setOnAction(this::handleSaveButtonAction);
        }
        // Key handlers – Enter saves, Ctrl+V handles "KEY=VALUE" paste
        if (env != null) {
            env.setOnKeyReleased(this::handleEnvKey);
            // Also handle Ctrl+V explicitly via key event
            env.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.isControlDown() && e.getCode() == KeyCode.V) handleEnvCtrlV(e);
            });
        }
        if (value != null) {
            value.setOnKeyReleased(e -> {
                if (e.getCode() == KeyCode.ENTER) handleSaveButtonAction(null);
            });
        }
    }

    // -----------------------------------------------------------------------
    // Popover wiring – called by EnvViewer
    // -----------------------------------------------------------------------

    public void setPopOver(Popup popup) { this.popOver = popup; }
    public void setPopOverWindow(javafx.stage.PopupWindow w) { this.popupWindow = w; }
    public void setPopOverHandle(Object handle) { this.popOverHandle = handle; }

    public TextField getEnvField() { return env; }
    public TextField getValueField() { return value; }

    // -----------------------------------------------------------------------
    // Handlers – mirrors PHP @event methods
    // -----------------------------------------------------------------------

    @FXML
    private void handleSaveButtonAction(javafx.event.ActionEvent e) {
        String varText = env != null ? env.getText() : null;
        if (varText == null || varText.isBlank()) {
            showAlert(loc.get("ENVEDITOR.NOVARIABLE"), Alert.AlertType.ERROR);
            return;
        }
        varText = varText.trim();

        // LD_PRELOAD + steamOverlay check – mirrors PHP
        if ("LD_PRELOAD".equals(varText)) {
            boolean overlaySelected = isSteamOverlaySelected();
            if (overlaySelected) {
                showAlert(loc.get("ENVEDITOR.STEAMOVERLAY"), Alert.AlertType.ERROR);
                return;
            }
        } else if (isBlacklistedEnv(varText)) {
            showAlert(loc.get("ENVEDITOR.BLACKLISTED"), Alert.AlertType.ERROR);
            return;
        }

        EnvViewer envViewer = findEnvViewer();
        if (envViewer == null) {
            LOG.warn("EnvEditor save: EnvViewer not found");
            hidePopover();
            return;
        }

        String valText = value != null ? value.getText() : "";
        if (valText == null) valText = "";

        // Upsert into envTable – mirrors PHP loop find index then remove/insert or add
        var newEntry = new EnvViewer.EnvEntry(varText, valText);
        var table = envViewer.getEnvTable();
        var items = envViewer.getObservableItems();
        int existingIndex = -1;
        for (int i = 0; i < items.size(); i++) {
            if (items.get(i).variable().equals(varText)) { existingIndex = i; break; }
        }
        if (existingIndex != -1) {
            items.remove(existingIndex);
            items.add(existingIndex, newEntry);
        } else {
            items.add(newEntry);
        }

        // Dirty check – mirrors data('originalValues') != items.toArray ? show : hide saveButton
        boolean isDirty = envViewer.isDirty();
        envViewer.setSaveButtonVisible(isDirty);

        hidePopover();
    }

    public void doSaveButtonAction() { handleSaveButtonAction(null); }

    private void handleEnvKey(KeyEvent e) {
        if (e.getCode() == KeyCode.ENTER) {
            handleSaveButtonAction(null);
        } else if (e.getCode() == KeyCode.EQUALS) {
            // Mirrors doEnvKeyUp – if codeName == 'Equals', strip '=' and focus value
            if (env != null) {
                String t = env.getText();
                if (t != null && t.contains("=")) {
                    env.setText(t.replace("=", ""));
                    if (value != null) {
                        value.requestFocus();
                        value.positionCaret(value.getText() != null ? value.getText().length() : 0);
                    }
                } else if (t != null) {
                    // Just move focus if equals was typed (even if not in text yet)
                    // The key event may not have inserted char yet – schedule check
                    Platform.runLater(() -> {
                        String next = env.getText();
                        if (next != null && next.contains("=")) {
                            env.setText(next.replace("=", ""));
                            if (value != null) value.requestFocus();
                        }
                    });
                }
            }
        }
    }

    private void handleEnvCtrlV(KeyEvent e) {
        if (env == null) return;
        String text = env.getText();
        if (text != null && text.contains("=")) {
            String[] parts = text.split("=", 2);
            env.setText(parts[0]);
            if (value != null) {
                value.setText(parts.length > 1 ? parts[1] : "");
                value.requestFocus();
                value.positionCaret(value.getText() != null ? value.getText().length() : 0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Static helper – mirrors PHP isBlacklistedEnv
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code static function isBlacklistedEnv($env)}.
     * Returns true if env is in {@code ['WINEDLLOVERRIDES','PROTON_ENABLE_WAYLAND','PROTON_USE_WINED3D']}.
     */
    public static boolean isBlacklistedEnv(String env) {
        return env != null && BLACKLIST.contains(env);
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private boolean isSteamOverlaySelected() {
        // Mirrors app()->form('gameSettings')->steamOverlay->data('quUIElement')->selected
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getForm = launcherCls.getMethod("getForm", String.class);
            Object gs = getForm.invoke(null, "gameSettings");
            if (gs != null) {
                // Try to get steamOverlay button then quUIElement
                var field = gs.getClass().getDeclaredField("steamOverlay");
                field.setAccessible(true);
                Object btn = field.get(gs);
                if (btn instanceof Button b) {
                    Object toggle = b.getProperties().get("quUIElement");
                    if (toggle instanceof javafx.scene.control.ToggleButton tb) return tb.isSelected();
                    if (toggle instanceof javafx.scene.control.CheckBox cb) return cb.isSelected();
                } else if (btn instanceof javafx.scene.control.ToggleButton tb) {
                    // Direct toggle
                    return tb.isSelected();
                }
            }
        } catch (Exception ex) {
            LOG.debug("steamOverlay check failed", ex);
        }
        return false;
    }

    private EnvViewer findEnvViewer() {
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            var getForm = launcherCls.getMethod("getForm", String.class);
            Object viewer = getForm.invoke(null, "envViewer");
            if (viewer instanceof EnvViewer ev) return ev;
            // Try cast via reflection without hard dep
            if (viewer != null && viewer.getClass().getSimpleName().equals("EnvViewer")) {
                // Use reflection to get items
                return (EnvViewer) viewer;
            }
        } catch (Exception e) {
            LOG.debug("findEnvViewer via Launcher failed", e);
        }
        // Fallback: try to locate via AppModule holder or singleton registry
        return EnvViewer.getLastInstance();
    }

    private void hidePopover() {
        if (popOver != null && popOver.isShowing()) popOver.hide();
        else if (popupWindow != null && popupWindow.isShowing()) popupWindow.hide();
        else if (popOverHandle != null) {
            try {
                var method = popOverHandle.getClass().getMethod("hide");
                method.invoke(popOverHandle);
            } catch (Exception e) {
                LOG.debug("hide popover handle failed", e);
            }
        } else {
            // Fallback: try to hide stage containing root
            if (root != null && root.getScene() != null) {
                var w = root.getScene().getWindow();
                if (w != null) w.hide();
            }
        }
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            var a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
    }
}

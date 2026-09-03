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

package com.corkytux.launcher.util;

import javafx.animation.FadeTransition;
import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ToggleButton;
import javafx.scene.layout.HBox;
import javafx.scene.text.Font;
import javafx.scene.text.FontWeight;
import javafx.util.Duration;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Java 25 / JavaFX 21 port of {@code quUI/quUI.php} (112 lines).
 *
 * <p>PHP original:</p>
 * <pre>
 * namespace quUI;
 * class quUI {
 *   static function generateSetButton(UXButton $button,string $text,UXNode $element)
 *   static function animateWithoutConflict($animation,$node,$speed,$callback)
 *   static function showFormAndFocus($form,$new)
 * }
 * </pre>
 *
 * <p>PHP used DevelNext controls: {@code UXButton.data('quUIElement')},
 * {@code UXHBox}, {@code UXLabel}, {@code UXToggleSwitch} and
 * {@code php\gui\animatefx\AnimationFX}. In JavaFX 21 these map to
 * {@link Button#getProperties()}, {@link HBox}, {@link Label},
 * {@link ToggleButton} / {@link javafx.scene.control.CheckBox} and
 * {@link FadeTransition} / {@link TranslateTransition}.</p>
 *
 * <p>The commented-out {@code generateContextMenu} helper from PHP is preserved
 * as a documented stub (see {@link #generateContextMenuStub}) for completeness.</p>
 */
public final class QuUI {

    private static final Logger LOG = LoggerFactory.getLogger(QuUI.class);

    private QuUI() {}

    // -----------------------------------------------------------------------
    // generateSetButton – mirrors PHP quUI::generateSetButton
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code generateSetButton(UXButton $button, string $text, UXNode $element)}.
     *
     * <p>PHP steps:</p>
     * <pre>
     * $hbox = new UXHBox;
     * $label = new UXLabel($text);
     * $label-&gt;font = UXFont::of('System',12);
     * $label-&gt;textColor = 'White';
     * $hbox-&gt;size = $label-&gt;size = $button-&gt;size;
     * $hbox-&gt;paddingLeft = $hbox-&gt;paddingRight = 8;
     * $hbox-&gt;alignment = 'CENTER_LEFT';
     * $hbox-&gt;children-&gt;addAll([$label,$element]);
     * $button-&gt;data('quUIElement',$element);
     * $button-&gt;graphic = $hbox;
     * $button-&gt;on('click',function () use ($element){$element-&gt;selected = !$element-&gt;selected;});
     * </pre>
     *
     * <p>Java maps {@code UXToggleSwitch} to {@link ToggleButton} (or any
     * {@link javafx.scene.control.Toggle} node). The toggle node is stashed in
     * {@code button.getProperties().put("quUIElement", element)} exactly as PHP
     * used {@code data('quUIElement')}. Clicking the host button flips
     * {@code selected} on the embedded toggle – the same observable contract
     * that {@code launcherSettings.php} and {@code gameSettings.php} rely on via
     * {@code data('quUIElement')->selected}.</p>
     *
     * @param button  host button whose graphic becomes the composite row
     * @param text    label text (e.g. localized "Fullscreen main window")
     * @param element toggle/switch node to embed (must expose selectedProperty)
     */
    public static void generateSetButton(Button button, String text, Node element) {
        if (button == null || element == null) {
            LOG.warn("generateSetButton called with null button/element: button={} element={}", button, element);
            return;
        }

        var label = new Label(text != null ? text : "");
        label.setFont(Font.font("System", 12));
        label.setStyle("-fx-text-fill: white;");

        var hbox = new HBox(8);
        hbox.setAlignment(Pos.CENTER_LEFT);
        hbox.setPadding(new Insets(0, 8, 0, 8));

        // Ensure toggle element has correct styling – wined3d/useWayland/steamRuntime etc use toggle-switch appearance
        if (element instanceof ToggleButton tb) {
            if (!tb.getStyleClass().contains("toggle-switch")) tb.getStyleClass().add("toggle-switch");
            tb.setFocusTraversable(false);
            tb.setMouseTransparent(true); // click handled by host Button, avoids double-flip
            tb.setMinSize(44, 24);
            tb.setPrefSize(44, 24);
            tb.setMaxSize(44, 24);
            tb.setText("");
        } else if (element instanceof javafx.scene.control.CheckBox cb) {
            cb.setFocusTraversable(false);
            cb.setMouseTransparent(true);
        }
        // Mirror PHP size sync: hbox/label inherit button size – use pref fallback when width not yet laid out
        double prefW = button.getPrefWidth() > 0 ? button.getPrefWidth() : button.getWidth() > 0 ? button.getWidth() : 272;
        double prefH = button.getPrefHeight() > 0 ? button.getPrefHeight() : button.getHeight() > 0 ? button.getHeight() : 58;
        hbox.setPrefSize(prefW, prefH);
        hbox.prefWidthProperty().bind(button.widthProperty());
        hbox.prefHeightProperty().bind(button.heightProperty());
        // Label width: leave room for toggle (approx 50) + padding
        label.setPrefWidth(prefW - 70);
        label.prefWidthProperty().bind(button.widthProperty().subtract(70));
        label.setMaxWidth(Double.MAX_VALUE);

        hbox.getChildren().addAll(label, element);
        HBox.setHgrow(label, javafx.scene.layout.Priority.ALWAYS);
        // Element must be visible – previous GameSettings wrap hid it, QuUI must show it for wined3d/useWayland
        element.setVisible(true);
        element.setManaged(true);

        button.getProperties().put("quUIElement", element);
        button.setGraphic(hbox);
        // PHP used on('click') which toggled selected; JavaFX Button fires ActionEvent on click.
        // We set a default toggle handler, but callers (GameSettings/LauncherSettings) overwrite with
        // custom handlers that flip explicitly via setSelected(!old). To avoid double-flip when
        // caller overwrites, we use addEventHandler not setOnAction – but store reference so overwrite
        // via setOnAction still keeps graphic. Use setOnAction only if no handler yet.
        if (button.getOnAction() == null) {
            button.setOnAction(e -> toggleSelected(element));
        } else {
            // Preserve existing handler and add toggle as event handler with lower priority
            button.addEventHandler(javafx.event.ActionEvent.ACTION, e -> {
                // Only toggle if handler didn't already flip – check if toggle selected matches expected?
                // Fallback: do not auto-flip here; caller handles flip explicitly.
            });
        }

        // Keep button contentDisplay graphic-only semantics – text is inside graphic HBox
        button.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
        LOG.debug("generateSetButton: '{}' -> {}", text, element.getClass().getSimpleName());
    }

    /**
     * Overload for {@link ToggleButton} hosts (used by some FXML where host is ToggleButton).
     */
    public static void generateSetButton(ToggleButton button, String text, Node element) {
        if (button == null || element == null) return;
        var label = new Label(text != null ? text : "");
        label.setFont(Font.font("System", 12));
        label.setStyle("-fx-text-fill: white;");
        var hbox = new HBox(8);
        hbox.setAlignment(Pos.CENTER_LEFT);
        hbox.setPadding(new Insets(0, 8, 0, 8));
        if (element instanceof ToggleButton tb) {
            if (!tb.getStyleClass().contains("toggle-switch")) tb.getStyleClass().add("toggle-switch");
            tb.setFocusTraversable(false);
            tb.setMouseTransparent(true);
            tb.setMinSize(44, 24);
            tb.setPrefSize(44, 24);
            tb.setMaxSize(44, 24);
            tb.setText("");
        }
        double prefW = button.getPrefWidth() > 0 ? button.getPrefWidth() : 272;
        hbox.setPrefSize(prefW, button.getPrefHeight() > 0 ? button.getPrefHeight() : 58);
        hbox.prefWidthProperty().bind(button.widthProperty());
        hbox.prefHeightProperty().bind(button.heightProperty());
        label.setPrefWidth(prefW - 70);
        label.prefWidthProperty().bind(button.widthProperty().subtract(70));
        element.setVisible(true);
        element.setManaged(true);
        hbox.getChildren().addAll(label, element);
        button.getProperties().put("quUIElement", element);
        button.setGraphic(hbox);
        if (button.getOnAction() == null) button.setOnAction(e -> toggleSelected(element));
        button.setContentDisplay(javafx.scene.control.ContentDisplay.GRAPHIC_ONLY);
    }

    private static void toggleSelected(Node element) {
        if (element instanceof ToggleButton tb) {
            tb.setSelected(!tb.isSelected());
        } else if (element instanceof javafx.scene.control.CheckBox cb) {
            cb.setSelected(!cb.isSelected());
        } else {
            // Generic toggle via properties – some callers stash a ToggleButton inside
            Object sel = element.getProperties().get("selected");
            if (sel instanceof Boolean b) element.getProperties().put("selected", !b);
            else LOG.debug("toggleSelected: unsupported element type {}", element.getClass());
        }
    }

    // -----------------------------------------------------------------------
    // generateContextMenu – PHP had this commented out; preserved as stub
    // -----------------------------------------------------------------------

    /**
     * Stub preserving the commented-out PHP helper {@code generateContextMenu}.
     * The original DevelNext version built a {@code UXListView} inside a
     * {@code UXPopOver} with expand arrow graphics and a callback on selection.
     * Kept for documentation; not used at runtime.
     *
     * @see <a href="quUI.php">quUI.php – commented block lines 29-79</a>
     */
    @SuppressWarnings("unused")
    private static void generateContextMenuStub() {
        LOG.debug("generateContextMenu is intentionally not ported – PHP source had it commented out (lines 29-79)");
    }

    // -----------------------------------------------------------------------
    // animateWithoutConflict – mirrors PHP animateWithoutConflict
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code animateWithoutConflict($animation,$node,$speed,$callback)}.
     *
     * <p>PHP steps:</p>
     * <pre>
     * if ($node-&gt;data('quUIAnimation') != null) $node-&gt;data('quUIAnimation')-&gt;stop();
     * $animation = new AnimationFX($animation,$node);
     * $animation-&gt;setOnFinished(function () use ($node,$callback){
     *     $node-&gt;data('quUIAnimation',null);
     *     if (is_callable($callback) and $node-&gt;opacity == 0 or $node-&gt;opacity == 1)
     *         $callback();
     * });
     * $node-&gt;data('quUIAnimation',$animation);
     * $animation-&gt;cycleCount = 1;
     * $animation-&gt;speed = $speed;
     * $animation-&gt;start();
     * </pre>
     *
     * <p>In JavaFX we represent {@code $animation} names ("FadeIn", "FadeOut",
     * "FadeInRight" etc.) with standard {@link FadeTransition} / {@link TranslateTransition}.
     * Conflict is avoided by stopping any transition stashed in
     * {@code node.getProperties().get("quUIAnimation")} before starting a new one.
     * The stored animation is cleared on finish and the callback is invoked only
     * when opacity has settled to 0 or 1 – matching the PHP guard
     * {@code opacity == 0 or opacity == 1}.</p>
     *
     * @param animationName name of AnimateFX animation (e.g. "FadeIn", "FadeOut", "FadeInRight")
     * @param node          target node
     * @param speed         rate multiplier (mirrors {@code AnimationFX.speed})
     * @param callback      optional callback invoked when opacity reaches 0 or 1
     */
    public static void animateWithoutConflict(String animationName, Node node, double speed, Runnable callback) {
        if (node == null) return;
        // Stop previous animation if any
        Object prev = node.getProperties().get("quUIAnimation");
        if (prev instanceof javafx.animation.Transition t) {
            t.stop();
        } else if (prev instanceof FadeTransition ft) {
            ft.stop();
        }

        javafx.animation.Transition transition = resolveAnimation(animationName, node);
        if (transition == null) {
            LOG.warn("animateWithoutConflict: unknown animation '{}', falling back to FadeIn", animationName);
            transition = new FadeTransition(Duration.millis(350), node);
            ((FadeTransition) transition).setFromValue(node.getOpacity());
            ((FadeTransition) transition).setToValue(1.0);
        }

        transition.setRate(speed);
        transition.setCycleCount(1);

        final javafx.animation.Transition t = transition;
        t.setOnFinished(e -> {
            node.getProperties().put("quUIAnimation", null);
            double opacity = node.getOpacity();
            // PHP guard: (is_callable(callback) and node.opacity==0 or node.opacity==1) – due to precedence,
            // PHP evaluates (callable && opacity==0) || opacity==1; we interpret strictly as 0 or 1.
            boolean settled = Math.abs(opacity) < 0.01 || Math.abs(opacity - 1.0) < 0.01;
            if (callback != null && settled) {
                try { callback.run(); } catch (Exception ex) { LOG.warn("animate callback failed", ex); }
            }
        });

        node.getProperties().put("quUIAnimation", t);
        t.play();
    }

    /**
     * Convenience overload with default speed 1.0.
     */
    public static void animateWithoutConflict(String animationName, Node node, Runnable callback) {
        animateWithoutConflict(animationName, node, 1.0, callback);
    }

    private static javafx.animation.Transition resolveAnimation(String name, Node node) {
        if (name == null) return null;
        return switch (name) {
            case "FadeIn" -> {
                var ft = new FadeTransition(Duration.millis(350), node);
                ft.setFromValue(0.0); ft.setToValue(1.0);
                yield ft;
            }
            case "FadeOut" -> {
                var ft = new FadeTransition(Duration.millis(350), node);
                ft.setFromValue(1.0); ft.setToValue(0.0);
                yield ft;
            }
            case "FadeInRight" -> {
                var ft = new FadeTransition(Duration.millis(350), node);
                ft.setFromValue(0.0); ft.setToValue(1.0);
                var tt = new TranslateTransition(Duration.millis(350), node);
                tt.setFromX(80); tt.setToX(0);
                // Combine via ParallelTransition
                var pt = new javafx.animation.ParallelTransition(ft, tt);
                yield pt;
            }
            case "FadeOutRight" -> {
                var ft = new FadeTransition(Duration.millis(350), node);
                ft.setFromValue(1.0); ft.setToValue(0.0);
                var tt = new TranslateTransition(Duration.millis(350), node);
                tt.setFromX(0); tt.setToX(80);
                yield new javafx.animation.ParallelTransition(ft, tt);
            }
            default -> null;
        };
    }

    // -----------------------------------------------------------------------
    // showFormAndFocus – mirrors PHP showFormAndFocus
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code showFormAndFocus($form,$new=false)}.
     *
     * <pre>
     * $form = $new ? app()->showNewForm($form) : app()->showForm($form);
     * uiLater(function () use ($form) {$form->requestFocus();});
     * return $form;
     * </pre>
     *
     * <p>In Java we look up {@code com.corkytux.launcher.Launcher} via reflection
     * to avoid hard-compile dependency, then request focus on FX thread.</p>
     *
     * @param formName FXML/name key (e.g. "launcherSettings")
     * @param isNew    if true, creates a new instance; otherwise reuses singleton
     * @return the shown stage/controller or null if launcher unavailable
     */
    public static Object showFormAndFocus(String formName, boolean isNew) {
        if (formName == null || formName.isBlank()) return null;
        try {
            var launcherCls = Class.forName("com.corkytux.launcher.Launcher");
            String methodName = isNew ? "showNewForm" : "showForm";
            var method = launcherCls.getMethod(methodName, String.class);
            Object form = method.invoke(null, formName);
            if (form != null) {
                Platform.runLater(() -> {
                    try {
                        // Try Stage or Node focus
                        if (form instanceof javafx.stage.Stage stage) stage.requestFocus();
                        else if (form instanceof Node n) n.requestFocus();
                        else {
                            var reqFocus = form.getClass().getMethod("requestFocus");
                            reqFocus.invoke(form);
                        }
                    } catch (Exception e) {
                        LOG.debug("requestFocus failed for {}", formName, e);
                    }
                });
            }
            return form;
        } catch (ClassNotFoundException e) {
            LOG.debug("Launcher not found – showFormAndFocus headless for {}", formName);
            return null;
        } catch (Exception e) {
            LOG.warn("showFormAndFocus failed for {}", formName, e);
            return null;
        }
    }

    public static Object showFormAndFocus(String formName) {
        return showFormAndFocus(formName, false);
    }
}

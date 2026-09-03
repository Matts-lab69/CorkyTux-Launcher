package com.corkytux.launcher.ui;

import com.corkytux.launcher.modules.AccentColorManager;
import javafx.animation.Interpolator;
import javafx.animation.KeyFrame;
import javafx.animation.KeyValue;
import javafx.animation.Timeline;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.control.Label;
import javafx.scene.layout.HBox;
import javafx.scene.layout.StackPane;
import javafx.scene.paint.Color;
import javafx.scene.paint.CycleMethod;
import javafx.scene.paint.LinearGradient;
import javafx.scene.paint.Stop;
import javafx.scene.shape.Circle;
import javafx.scene.shape.Rectangle;
import javafx.util.Duration;

/**
 * Custom toggle switch component.
 * The switch itself handles click events and fires onToggle directly.
 * This is necessary because when used as a Button's graphic,
 * the Button's onAction does NOT fire on click.
 */
public class SwitchComponent extends HBox {

    private final StackPane track;
    private final Circle thumb;
    private final Label label;
    private boolean selected;
    private Runnable onToggle;

    private static final double TRACK_W = 44;
    private static final double TRACK_H = 24;
    private static final double THUMB_R = 9;

    public SwitchComponent(String text) {
        setAlignment(Pos.CENTER_LEFT);
        setSpacing(10);
        setPadding(new Insets(2, 0, 2, 0));

        track = new StackPane();
        track.setPrefSize(TRACK_W, TRACK_H);
        track.setMinSize(TRACK_W, TRACK_H);
        track.setMaxSize(TRACK_W, TRACK_H);

        Rectangle bg = new Rectangle(TRACK_W, TRACK_H);
        bg.setArcWidth(TRACK_H);
        bg.setArcHeight(TRACK_H);
        bg.setFill(Color.web("#383838"));
        track.getChildren().add(bg);

        thumb = new Circle(THUMB_R, Color.web("#D1D1D1"));
        StackPane.setAlignment(thumb, Pos.CENTER_LEFT);
        thumb.setTranslateX(-12);
        track.getChildren().add(thumb);

        label = new Label(text);
        label.setTextFill(Color.WHITE);
        label.setStyle("-fx-font-size:12px;");
        HBox.setHgrow(label, javafx.scene.layout.Priority.ALWAYS);

        var spacer = new javafx.scene.layout.Region();
        HBox.setHgrow(spacer, javafx.scene.layout.Priority.ALWAYS);

        getChildren().addAll(label, spacer, track);

        // Click on the switch toggles it and fires onToggle directly.
        // This is the key fix: Button.onAction never fires for graphic clicks.
        setOnMouseClicked(e -> {
            e.consume();
            setSelected(!selected);
        });
        setStyle("-fx-cursor: hand;");
    }

    public boolean isSelected() { return selected; }

    public void setSelected(boolean sel) {
        if (this.selected == sel) return;
        this.selected = sel;
        animateThumb();
        if (onToggle != null) onToggle.run();
    }

    public void setSelectedSilent(boolean sel) {
        this.selected = sel;
        animateThumb();
    }

    public void setLabel(String text) { label.setText(text); }

    public void setOnToggle(Runnable r) { this.onToggle = r; }

    private void animateThumb() {
        double target = selected ? 23 : -12;
        Timeline tl = new Timeline(new KeyFrame(
                Duration.millis(150),
                new KeyValue(thumb.translateXProperty(), target, Interpolator.EASE_BOTH)));
        Rectangle bg = (Rectangle) track.getChildren().get(0);
        if (selected) {
            String accent = AccentColorManager.getInstance().getPrimary();
            String pressed = AccentColorManager.getInstance().getPressed();
            bg.setFill(new LinearGradient(0, 0, 1, 0, true, CycleMethod.NO_CYCLE,
                    new Stop(0, Color.web(accent)),
                    new Stop(1, Color.web(pressed))));
            thumb.setFill(Color.web("#fffffe"));
        } else {
            bg.setFill(Color.web("#383838"));
            thumb.setFill(Color.web("#D1D1D1"));
        }
        tl.play();
    }
}

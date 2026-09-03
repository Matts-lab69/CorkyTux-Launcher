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

import javafx.scene.control.Label;
import javafx.scene.effect.DropShadow;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.AnchorPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Pane;
import javafx.scene.layout.VBox;
import javafx.scene.paint.Color;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.shape.Rectangle;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Java 25 / JavaFX 21 port of {@code prototypes.php} (10 lines).
 *
 * <p>PHP original:</p>
 * <pre>
 * namespace app\forms;
 * use std, gui, framework, app;
 * class prototypes extends AbstractForm {}
 * </pre>
 *
 * <p>The DevelNext prototypes form is an empty container used only as a
 * visual factory: {@code prototypes.panel} and sibling ids are cloned to build
 * game tiles in {@link MainForm#addGame} / {@code prototypes.behaviour}. All
 * styling (rounded corners, shadows) and layout is defined in
 * {@code prototypes.fxml}. In Java the same factory role is kept explicit:
 * this class exposes static helpers to create prototype nodes programmatically
 * when FXML is unavailable, while still being a valid {@code Initializable}
 * controller if {@code prototypes.fxml} is ever loaded as a real scene.</p>
 */
public class Prototypes {

    private static final Logger LOG = LoggerFactory.getLogger(Prototypes.class);

    // The prototype panel id in FXML – documented for tooling parity
    public static final String PANEL_ID = "panel";

    private Prototypes() {}

    /**
     * Creates a tile matching {@code prototypes.panel} as used by
     * {@code MainForm.addGame()}. This is the Java fallback when the FXML
     * prototype loader is not available (headless tests, dynamic generation).
     *
     * <p>Structure mirrors {@code prototypes.fxml} exactly:</p>
     * <pre>
     * AnchorPane "panel" (224x136, #333333, radius 15)
     *   ImageView "imageAlt" / "tileBanner" (224x136, stretch true, clip arc 30)
     *   HBox "hbox" (224x48 at y=88, #333337cd, radius 0 0 15 15)
     *     Label "label" / "tileGameName" + ImageView icon (34x34)
     * </pre>
     *
     * <p>PHP addGame() mapping:</p>
     * <pre>
     *   $gamePanel = instance('prototypes.panel');
     *   $iconView->size = [34,34]; proportional=centered=stretch=true;
     *   $clip->size = $gamePanel->children[1]->size; arc = borderRadius*2 = 30;
     *   $gamePanel->children[3]->children[0]->text = $gameName;
     *   $gamePanel->children[1]->image = fs::isFile(image)?image:noBanner;
     *   $gamePanel->children[3]->children[0]->graphic = $iconView;
     * </pre>
     *
     * @param gameName  displayed name
     * @param imagePath absolute path to banner or null for fallback
     * @param iconPath  absolute path to icon or null for fallback
     * @return configured {@link Pane} tile ready to insert into FlowPane
     */
    public static Pane createPanel(String gameName, String imagePath, String iconPath) {
        var tile = new AnchorPane();
        tile.setId(PANEL_ID);
        tile.setPrefSize(224, 136);
        tile.setMinSize(224, 136);
        tile.setMaxSize(224, 136);
        // Matches FXML Panel backgroundColor #333333 borderRadius 15
        tile.setStyle("-fx-background-color:#333333;-fx-background-radius:15px;");
        tile.getProperties().put("gameName", gameName);

        var bannerView = new ImageView();
        bannerView.setId("tileBanner");
        // Also keep alias "imageAlt" for PHP children[1] parity
        bannerView.getProperties().put("alias", "imageAlt");
        bannerView.setFitWidth(224);
        bannerView.setFitHeight(136);
        bannerView.setPreserveRatio(false);
        bannerView.setSmooth(true);

        // Banner image: covers from ~/.config/CorkyTux/banners/{name}.jpg/.png – matches original PHP fix
        // PHP: $gamePanel->children[1]->image = new UXImage(fs::isFile($image) ? $image : 'res://.data/img/noBanner.png');
        // Our Java variant keeps 1:1 fallback chain: explicit imagePath -> banners/{gameName}.* -> noBanner.png (both resource roots)
        Image bannerImg = resolveImage(imagePath, null);
        if (bannerImg == null && imagePath != null) {
            // Try banners dir variant where imagePath may be bare appId without extension
            String home = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
            // Also handle case where imagePath was already a banners file but missing extension
            for (String ext : new String[]{".png", ".jpg", ".jpeg"}) {
                if (imagePath.endsWith(ext)) continue;
                String withExt = imagePath + ext;
                bannerImg = resolveImage(withExt, null);
                if (bannerImg != null) break;
            }
        }
        if (bannerImg == null) {
            String home = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
            for (String ext : new String[]{".png", ".jpg", ".jpeg"}) {
                String tryPath = home + "/.config/CorkyTux/banners/" + gameName + ext;
                bannerImg = resolveImage(tryPath, null);
                if (bannerImg != null) break;
            }
        }
        if (bannerImg == null) {
            // Try both resource roots – PHP used res://.data/img/noBanner.png, we have both /img and /.data/img on classpath
            bannerImg = resolveImage(null, "/img/noBanner.png");
            if (bannerImg == null) bannerImg = resolveImage(null, "/.data/img/noBanner.png");
            if (bannerImg == null) bannerImg = resolveImage(null, "/img/noBanner.png"); // final fallback if packaged differently
        }
        bannerView.setImage(bannerImg);

        var clip = new Rectangle(224, 136);
        // borderRadius 15 *2 =30 mirrors PHP $clip->arcHeight = $clip->arcWidth = $gamePanel->borderRadius*2
        clip.setArcWidth(30);
        clip.setArcHeight(30);
        bannerView.setClip(clip);
        AnchorPane.setTopAnchor(bannerView, 0.0);
        AnchorPane.setLeftAnchor(bannerView, 0.0);

        var labelBox = new HBox(5);
        labelBox.setId("hbox");
        labelBox.setAlignment(Pos.CENTER_LEFT);
        labelBox.setPrefSize(224, 48);
        labelBox.setMinSize(224, 48);
        labelBox.setMaxSize(224, 48);
        // HBox style from FXML: #333337cd radius 0 0 15 15
        labelBox.setStyle("-fx-background-color:#333337cd;-fx-background-radius:0 0 15 15;");
        labelBox.setPadding(new Insets(0, 10, 0, 10));
        AnchorPane.setLeftAnchor(labelBox, 0.0);
        AnchorPane.setBottomAnchor(labelBox, 0.0);

        var iconView = new ImageView();
        iconView.setFitWidth(34);
        iconView.setFitHeight(34);
        iconView.setPreserveRatio(true);
        iconView.setSmooth(true);
        // Icon: covers from ~/.config/CorkyTux/icons – matches original PHP
        // PHP: $iconView = new UXImageArea(new UXImage(fs::isFile($icon) ? $icon : 'res://.data/img/noImage.png')); size [34,34] proportional=centered=stretch
        // Java: resolve explicit iconPath, then icons/{gameName}.*, then icons/{fileName}, then noImage.png fallback (both resource roots)
        Image iconImg = resolveImage(iconPath, null);
        if (iconImg == null) {
            String home = com.corkytux.launcher.modules.FilesWorker.getExpectedHome();
            // Try exact icons dir entry without extension (PHP stored without ext)
            for (String ext : new String[]{"", ".png", ".jpg", ".jpeg"}) {
                String tryPath = home + "/.config/CorkyTux/icons/" + gameName + ext;
                iconImg = resolveImage(tryPath, null);
                if (iconImg != null) break;
            }
            if (iconImg == null) {
                // Also try iconPath's filename in icons dir (covers re-added games where icon stored under random name)
                String fileName = iconPath != null ? Path.of(iconPath).getFileName().toString() : null;
                if (fileName != null) {
                    String tryIcon = home + "/.config/CorkyTux/icons/" + fileName;
                    iconImg = resolveImage(tryIcon, null);
                    if (iconImg == null) {
                        // Try stripping extension like PHP – icon stored without ext in some versions
                        String noExt = fileName.contains(".") ? fileName.substring(0, fileName.lastIndexOf('.')) : fileName;
                        iconImg = resolveImage(home + "/.config/CorkyTux/icons/" + noExt, null);
                        if (iconImg == null && noExt != null) {
                            // Also try with extensions appended to noExt
                            for (String ext : new String[]{".png", ".jpg"}) {
                                iconImg = resolveImage(home + "/.config/CorkyTux/icons/" + noExt + ext, null);
                                if (iconImg != null) break;
                            }
                        }
                    }
                    // Also try raw fileName with extension appended if original lacked it
                    if (iconImg == null) {
                        for (String ext : new String[]{".png", ".jpg"}) {
                            if (fileName.endsWith(ext)) continue;
                            iconImg = resolveImage(home + "/.config/CorkyTux/icons/" + fileName + ext, null);
                            if (iconImg != null) break;
                        }
                    }
                }
            }
            // Last resort: scan icons dir for any file starting with gameName (covers legacy)
            if (iconImg == null) {
                var iconsDir = Path.of(home, ".config/CorkyTux/icons");
                if (Files.isDirectory(iconsDir)) {
                    try (var stream = Files.list(iconsDir)) {
                        var match = stream.filter(p -> p.getFileName().toString().toLowerCase().startsWith(gameName.toLowerCase()))
                                .findFirst().orElse(null);
                        if (match != null) iconImg = resolveImage(match.toString(), null);
                    } catch (Exception ignored) {}
                }
            }
        }
        if (iconImg == null) {
            iconImg = resolveImage(null, "/img/noImage.png");
            if (iconImg == null) iconImg = resolveImage(null, "/.data/img/noImage.png");
            if (iconImg == null) iconImg = resolveImage(null, "/img/game.png"); // tertiary fallback
        }
        // Ensure iconView respects proportional=centered=stretch: preserveRatio true + smooth true + size 34x34
        if (iconImg != null && iconImg.isError()) {
            LOG.warn("Icon image failed to load for {}: error={}", gameName, iconImg.getException());
            var fallback = resolveImage(null, "/img/noImage.png");
            if (fallback != null) iconImg = fallback;
        }
        iconView.setImage(iconImg);

        var nameLabel = new Label(gameName);
        nameLabel.setId("tileGameName");
        // Also alias "label" for PHP children[3]->children[0] parity (the label inside HBox)
        nameLabel.getProperties().put("alias", "label");
        nameLabel.setTextFill(Color.WHITE);
        nameLabel.setStyle("-fx-font-weight: bold; -fx-font-size: 12;");
        nameLabel.setWrapText(true);
        nameLabel.setPrefWidth(208);
        nameLabel.setGraphic(iconView);
        nameLabel.setContentDisplay(javafx.scene.control.ContentDisplay.LEFT);
        nameLabel.setGraphicTextGap(4);

        labelBox.getChildren().add(nameLabel);
        tile.getChildren().addAll(bannerView, labelBox);

        // Shadow – mirrors PHP addBasicEffects DropShadow color #0000004d
        var shadow = new DropShadow();
        shadow.setColor(Color.web("#0000004d"));
        shadow.setRadius(10);
        shadow.setOffsetY(4);
        tile.setEffect(shadow);

        LOG.debug("Prototypes.createPanel game={} banner={} icon={}", gameName, imagePath, iconPath);
        return tile;
    }

    /**
     * Lightweight stub tile used during game import – mirrors
     * {@code MainForm.addStubGame()}'s {@code VBox} path but sourced through
     * the prototypes factory for consistency.
     * Structure mirrors prototypes.fxml gameStubBox (224x136, style modern-input-box).
     */
    public static VBox createStubPanel(String placeholderName) {
        var box = new VBox(0);
        box.setId("gameStubBox");
        box.setPrefSize(224, 136);
        box.setMinSize(224, 136);
        box.setMaxSize(224, 136);
        box.setAlignment(Pos.CENTER);
        // Matches prototypes.fxml: no inline bg, but FXML uses Panel #333337; we mirror with inline
        // Use same as PHP stub via prototypes.gameStubBox which inherits modern-input-box; fallback bg #333337
        box.setStyle("-fx-background-color:#333337;-fx-background-radius:15px;");
        box.getStyleClass().add("modern-input-box");
        var nameLabel = new Label(placeholderName != null ? placeholderName : "...");
        nameLabel.setId("label4");
        nameLabel.setAlignment(Pos.CENTER);
        nameLabel.setPrefWidth(192);
        nameLabel.setTextFill(Color.WHITE);
        nameLabel.setStyle("-fx-font-weight: bold; -fx-font-size:13;");
        var statusLabel = new Label("Please wait until game added");
        statusLabel.setId("label5");
        statusLabel.setAlignment(Pos.CENTER);
        statusLabel.setPrefWidth(200);
        statusLabel.setTextFill(Color.web("#e6e6e6"));
        statusLabel.setStyle("-fx-font-size:12;");
        box.getChildren().addAll(nameLabel, statusLabel);
        box.getProperties().put("gameNameLabel", nameLabel);
        box.getProperties().put("statusLabel", statusLabel);
        return box;
    }

    private static Image resolveImage(String path, String fallbackResource) {
        if (path != null && Files.isRegularFile(Path.of(path))) {
            try { return new Image(Path.of(path).toUri().toString()); }
            catch (Exception e) { LOG.debug("resolveImage file failed {}", path, e); }
        }
        // Also try without extension variants if path lacked ext but file exists with ext
        if (path != null && !path.isBlank()) {
            for (String ext : new String[]{".png", ".jpg", ".jpeg"}) {
                String withExt = path + ext;
                if (Files.isRegularFile(Path.of(withExt))) {
                    try { return new Image(Path.of(withExt).toUri().toString()); } catch (Exception ignored) {}
                }
            }
        }
        if (fallbackResource != null) {
            try (var is = Prototypes.class.getResourceAsStream(fallbackResource)) {
                if (is != null) return new Image(is);
            } catch (Exception e) { LOG.debug("resolveImage fallback failed {}", fallbackResource, e); }
        }
        return null;
    }
}

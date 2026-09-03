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

import javafx.scene.layout.StackPane;

/**
 * Minimal stub for DevelNext Data node used in FXML.
 * Holds graphic/image references as properties for MainForm to read.
 */
public class Data extends StackPane {
    private String graphic;
    private String image;

    public Data() {
        setVisible(false);
        setManaged(false);
        setMouseTransparent(true);
        setFocusTraversable(false);
        // Data is a metadata container, not a visible panel – matches DevelNext Data behavior
        setMaxSize(0, 0);
        setPrefSize(0, 0);
    }

    public String getGraphic() { return graphic; }
    public void setGraphic(String graphic) { this.graphic = graphic; }

    public String getImage() { return image; }
    public void setImage(String image) { this.image = image; }
}

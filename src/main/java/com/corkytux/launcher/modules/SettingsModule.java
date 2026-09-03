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

package com.corkytux.launcher.modules;

import javafx.animation.FadeTransition;
import javafx.application.Platform;
import javafx.scene.Node;
import javafx.scene.control.TextInputControl;
import javafx.stage.DirectoryChooser;
import javafx.stage.Window;
import javafx.util.Duration;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * Java 25 port of {@code SettingsModule.php} (51 lines).
 *
 * <p>Mirrors the DevelNext settings navigation and directory-choosing helpers.
 * PHP original used {@code AbstractModule} with dynamic {@code $this->activePage}
 * and {@code $this->{$oldPage->id.'Button'}} toggle-button lookups plus
 * {@code Animation::fadeOut/fadeIn(...,350)}.
 * In Java the same behaviour is provided via a typed {@link Node} graph,
 * {@link FadeTransition} (350 ms) and JavaFX {@link DirectoryChooser}.</p>
 */
public final class SettingsModule {

    private static final Logger LOG = LoggerFactory.getLogger(SettingsModule.class);

    /** Currently visible settings pane – mirrors PHP {@code $activePage}. */
    private Node activePage;
    
    /** Guard against rapid clicking – disables buttons during animation */
    private boolean switching = false;

    // -----------------------------------------------------------------------
    // Instance API – page switching
    // -----------------------------------------------------------------------

    public SettingsModule() {}

    public SettingsModule(Node initialPage) {
        this.activePage = initialPage;
    }

    public Node getActivePage() {
        return activePage;
    }

    public void setActivePage(Node activePage) {
        this.activePage = activePage;
    }

    /**
     * Mirrors PHP {@code switchPage($newPage)}.
     * <p>
     * PHP steps:
     * <pre>
     * $oldPage = $this->activePage;
     * $oldPageButtonID = $oldPage->id.'Button';
     * if ($oldPage == $newPage) { $this->$oldPageButtonID->selected = true; return; }
     * $this->$oldPageButtonID->selected = false;
     * $this->activePage = $newPage;
     * Animation::fadeOut($oldPage,350,function () use ($newPage,$oldPage){
     *     $oldPage->hide(); $newPage->show(); Animation::fadeIn($newPage,350);
     * });
     * </pre>
     * Java maps this to {@link FadeTransition} on FX thread. Button selection is handled
     * via lookup of {@code #&lt;id&gt;Button} in the page's parent if present.
     *
     * @param newPage node to switch to (must have an {@code id} matching PHP's page id)
     */
    public void switchPage(Node newPage) {
        if (newPage == null) {
            LOG.warn("switchPage called with null newPage");
            return;
        }
        
        // Guard against rapid clicking during animation
        if (switching) {
            LOG.debug("switchPage blocked – animation in progress");
            return;
        }
        
        Node oldPage = this.activePage;

        if (oldPage == newPage) {
            setButtonSelected(oldPage, true);
            runOnFx(() -> {
                newPage.setVisible(true);
                newPage.setManaged(true);
                newPage.setOpacity(1.0);
            });
            return;
        }

        if (oldPage != null) {
            setButtonSelected(oldPage, false);
        }

        this.activePage = newPage;
        switching = true;

        if (oldPage == null) {
            runOnFx(() -> {
                newPage.setVisible(true);
                newPage.setManaged(true);
                newPage.setOpacity(1.0);
                switching = false;
            });
            return;
        }

        runOnFx(() -> {
            // Ensure new page is prepared but invisible until fadeIn
            newPage.setVisible(true);
            newPage.setManaged(true);
            newPage.setOpacity(0.0);
            
            // Hide old page immediately to prevent overlap
            oldPage.setVisible(false);
            oldPage.setManaged(false);
            
            FadeTransition fadeIn = new FadeTransition(Duration.millis(200), newPage);
            fadeIn.setFromValue(0.0);
            fadeIn.setToValue(1.0);
            fadeIn.setOnFinished(ev -> {
                newPage.setVisible(true);
                newPage.setManaged(true);
                newPage.setOpacity(1.0);
                switching = false;
            });
            fadeIn.play();
        });
    }

    /**
     * Helper mirroring {@code $this->{$pageId.'Button'}->selected = value}.
     * Looks up {@code #<pageId>Button} in parent; logs if not found.
     */
    private static void setButtonSelected(Node page, boolean selected) {
        if (page == null || page.getId() == null) return;
        String buttonId = page.getId() + "Button";
        try {
            runOnFx(() -> {
                var parent = page.getParent();
                if (parent == null) {
                    LOG.debug("No parent to lookup button {}", buttonId);
                    return;
                }
                Node btn = parent.lookup("#" + buttonId);
                if (btn instanceof javafx.scene.control.ToggleButton tb) {
                    tb.setSelected(selected);
                } else if (btn instanceof javafx.scene.control.ButtonBase bb) {
                    // fallback: disable/enable visual hint
                    bb.setDisable(!selected);
                } else if (btn != null) {
                    LOG.debug("Button {} found but unsupported type {}", buttonId, btn.getClass());
                } else {
                    LOG.debug("Button {} not found in parent", buttonId);
                }
            });
        } catch (Exception e) {
            LOG.debug("setButtonSelected failed for {}", buttonId, e);
        }
    }

    // -----------------------------------------------------------------------
    // Static helper – directory chooser (mirrors PHP setWithDirChooser)
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code static function setWithDirChooser($param,$sender=null,$for=['launcher'])}.
     * <p>
     * PHP logic:
     * <pre>
     * $dc = new UXDirectoryChooser;
     * $gameDir = $dc->showDialog($this);
     * if ($gameDir == null) return;
     * if ($sender != null) $sender->text = $gameDir;
     * if ($for[0] == 'launcher') app()->appModule()->launcher->set($param,$gameDir,'User Settings');
     * else app()->appModule()->games->set($param,$gameDir,$for[1]);
     * return true;
     * </pre>
     *
     * @param param  ini key to store
     * @param sender optional FX control whose {@code text} is updated
     * @param owner  window owner for the chooser (may be null)
     * @param forParam varargs: {@code ["launcher"]} or {@code ["games", gameName]}
     * @return true if a directory was chosen and stored, false/null otherwise
     */
    public static boolean setWithDirChooser(String param, TextInputControl sender, Window owner, String... forParam) {
        String for0 = (forParam != null && forParam.length > 0) ? forParam[0] : "launcher";
        String for1 = (forParam != null && forParam.length > 1) ? forParam[1] : null;

        File chosen = showDirectoryChooser(owner);
        if (chosen == null) return false;

        String gameDir = chosen.getAbsolutePath();
        LOG.info("setWithDirChooser: param={} dir={} for={}", param, gameDir, for0);

        if (sender != null) {
            runOnFx(() -> sender.setText(gameDir));
        }

        AppModule app = AppModule.getInstance();
        if ("launcher".equals(for0)) {
            app.setLauncher(param, gameDir, "User Settings");
        } else {
            if (for1 == null) {
                LOG.warn("setWithDirChooser: games target requires game name in for[1]");
                return false;
            }
            app.setGame(param, gameDir, for1);
        }
        return true;
    }

    /**
     * Overload accepting any {@link Node} as sender (e.g. Label, Button).
     */
    public static boolean setWithDirChooserNode(String param, Node sender, Window owner, String... forParam) {
        File chosen = showDirectoryChooser(owner);
        if (chosen == null) return false;
        String gameDir = chosen.getAbsolutePath();
        if (sender instanceof javafx.scene.control.Labeled labeled) {
            runOnFx(() -> labeled.setText(gameDir));
        } else if (sender != null) {
            LOG.debug("setWithDirChooserNode: sender type {} not text-updatable", sender.getClass());
        }
        AppModule app = AppModule.getInstance();
        String for0 = (forParam != null && forParam.length > 0) ? forParam[0] : "launcher";
        String for1 = (forParam != null && forParam.length > 1) ? forParam[1] : null;
        if ("launcher".equals(for0)) {
            app.setLauncher(param, gameDir, "User Settings");
        } else {
            if (for1 == null) {
                LOG.warn("setWithDirChooserNode: missing game name");
                return false;
            }
            app.setGame(param, gameDir, for1);
        }
        return true;
    }

    /**
     * Convenience overload mirroring PHP call with no owner and no sender.
     */
    public static boolean setWithDirChooser(String param, String... forParam) {
        return setWithDirChooser(param, null, null, forParam);
    }

    private static File showDirectoryChooser(Window owner) {
        // Must run on FX thread; if called off FX, dispatch and block
        if (Platform.isFxApplicationThread()) {
            DirectoryChooser dc = new DirectoryChooser();
            dc.setTitle(localize("DIRCHOOSER.GAMEROOT.TITLE", "Select the root folder of the game"));
            return dc.showDialog(owner);
        } else {
            final File[] result = new File[1];
            var latch = new CountDownLatch(1);
            Platform.runLater(() -> {
                try {
                    DirectoryChooser dc = new DirectoryChooser();
                    dc.setTitle(localize("DIRCHOOSER.GAMEROOT.TITLE", "Select the root folder of the game"));
                    result[0] = dc.showDialog(owner);
                } finally {
                    latch.countDown();
                }
            });
            try { latch.await(5, TimeUnit.MINUTES); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
            return result[0];
        }
    }

    private static String localize(String key, String fallback) {
        try {
            return Localization.getInstance().get(key);
        } catch (Exception e) {
            return fallback;
        }
    }

    private static void runOnFx(Runnable r) {
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }
}

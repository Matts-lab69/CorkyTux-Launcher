package com.corkytux.launcher.modules;

import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.InputStream;

/**
 * ThemedIcons – serves _dark variants of white UI icons when the light
 * theme is active, so icons don't vanish on white surfaces.
 * Usage: ThemedIcons.applyTo(imageView, "/img/settings.png") stores the
 * base path and loads the right variant; call refreshThemedIcons(root)
 * after a theme switch.
 */
public final class ThemedIcons {

    private static final Logger LOG = LoggerFactory.getLogger(ThemedIcons.class);
    private static final String PROP = "themedIconBase";

    private ThemedIcons() {}

    /** Returns the resource path for the current theme (dark variant if light). */
    public static String pathFor(String basePath) {
        if (basePath == null) return null;
        boolean light = false;
        try { light = ThemeManager.getInstance().isLight(); } catch (Exception ignored) {}
        if (!light) return basePath;
        int dot = basePath.lastIndexOf('.');
        if (dot < 0) return basePath;
        String dark = basePath.substring(0, dot) + "_dark" + basePath.substring(dot);
        // Verify the variant exists, else keep base
        try (InputStream is = ThemedIcons.class.getResourceAsStream(dark)) {
            if (is != null) return dark;
        } catch (Exception ignored) {}
        return basePath;
    }

    /** Loads the themed image for basePath (null if missing). */
    public static Image load(String basePath) {
        String p = pathFor(basePath);
        if (p == null) return null;
        try (InputStream is = ThemedIcons.class.getResourceAsStream(p)) {
            if (is != null) return new Image(is);
        } catch (Exception e) {
            LOG.debug("ThemedIcons load failed {}", p, e);
        }
        return null;
    }

    /** Sets view image from base path and remembers it for theme refresh. */
    public static void applyTo(ImageView view, String basePath) {
        if (view == null || basePath == null) return;
        view.getProperties().put(PROP, basePath);
        Image img = load(basePath);
        if (img != null) view.setImage(img);
    }

    /** Re-resolves all marked ImageViews under root after a theme change. */
    public static void refreshAll(javafx.scene.Node root) {
        if (root == null) return;
        try {
            if (root instanceof ImageView iv && iv.getProperties().containsKey(PROP)) {
                Object b = iv.getProperties().get(PROP);
                if (b instanceof String s) {
                    Image img = load(s);
                    if (img != null) iv.setImage(img);
                }
            }
            if (root instanceof javafx.scene.Parent par) {
                for (var child : par.getChildrenUnmodifiable()) refreshAll(child);
            }
        } catch (Exception e) {
            LOG.debug("ThemedIcons refresh failed", e);
        }
    }
}

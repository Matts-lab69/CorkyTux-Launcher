package com.corkytux.launcher.modules;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * ThemeManager – dark / light launcher theme.
 * Dark is the default (current Spotify-style UI). Light is an alternate
 * palette loaded from style-v2-light.css. Persisted in Launcher.ini.
 * AccentColorManager override CSS applies on top of either theme.
 */
public class ThemeManager {

    private static final Logger LOG = LoggerFactory.getLogger(ThemeManager.class);
    private static ThemeManager instance;

    public static final String DARK = "dark";
    public static final String LIGHT = "light";

    private String current = DARK;

    private ThemeManager() { load(); }

    public static synchronized ThemeManager getInstance() {
        if (instance == null) instance = new ThemeManager();
        return instance;
    }

    public String getCurrent() { return current; }
    public boolean isLight() { return LIGHT.equals(current); }
    public boolean isDark() { return !isLight(); }

    /** Stylesheet resource for the active theme. */
    public String getStylesheetResource() {
        return isLight() ? "/style-v2-light.css" : "/style-v2.css";
    }

    public void setTheme(String theme) {
        if (!DARK.equals(theme) && !LIGHT.equals(theme)) return;
        this.current = theme;
        save();
        LOG.info("Theme changed to {}", theme);
    }

    private void save() {
        try {
            AppModule.getInstance().setLauncher("theme", current, "User Settings");
        } catch (Exception e) {
            LOG.warn("Failed to save theme", e);
        }
    }

    private void load() {
        try {
            String v = AppModule.getInstance().getLauncher("theme", "User Settings");
            if (LIGHT.equals(v)) current = LIGHT;
        } catch (Exception e) {
            LOG.warn("Failed to load theme", e);
        }
    }
}

package com.corkytux.launcher.modules;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * AccentColorManager – manages the launcher accent color.
 * Generates a CSS override stylesheet that replaces hardcoded #55de1b
 * with the user-selected accent color.
 * The base style.fx.css stays untouched; this adds a second CSS on top.
 */
public class AccentColorManager {

    private static final Logger LOG = LoggerFactory.getLogger(AccentColorManager.class);
    private static AccentColorManager instance;

    // id -> {displayName, primary, hover, pressed}
    private static final Map<String, String[]> ACCENTS = new LinkedHashMap<>();
    static {
        ACCENTS.put("green",  new String[]{"Green",  "#55de1b", "#66e131", "#44b115"});
        ACCENTS.put("blue",   new String[]{"Blue",   "#2196F3", "#42A5F5", "#1976D2"});
        ACCENTS.put("cyan",   new String[]{"Cyan",   "#00BCD4", "#26C6DA", "#0097A7"});
        ACCENTS.put("purple", new String[]{"Purple", "#9C27B0", "#AB47BC", "#7B1FA2"});
        ACCENTS.put("pink",   new String[]{"Pink",   "#E91E63", "#EC407A", "#C2185B"});
        ACCENTS.put("red",    new String[]{"Red",    "#F44336", "#EF5350", "#D32F2F"});
        ACCENTS.put("orange", new String[]{"Orange", "#FF9800", "#FFB74D", "#F57C00"});
        ACCENTS.put("yellow", new String[]{"Yellow", "#FFC107", "#FFD54F", "#FFA000"});
        ACCENTS.put("teal",   new String[]{"Teal",   "#009688", "#26A69A", "#00796B"});
        ACCENTS.put("indigo", new String[]{"Indigo", "#3F51B5", "#5C6BC0", "#303F9F"});
    }

    private String currentId = "green";

    private AccentColorManager() { load(); }

    public static synchronized AccentColorManager getInstance() {
        if (instance == null) instance = new AccentColorManager();
        return instance;
    }

    // ── Public API ────────────────────────────────────────────────────────

    public String getCurrentId() { return currentId; }
    public String getPrimary()   { return val("primary"); }
    public String getHover()     { return val("hover"); }
    public String getPressed()   { return val("pressed"); }

    public void setAccent(String id) {
        if (!ACCENTS.containsKey(id)) return;
        this.currentId = id;
        save();
        LOG.info("Accent color changed to {}", id);
    }

    public static Map<String, String[]> getAllAccents() { return Map.copyOf(ACCENTS); }
    public static String getDisplayName(String id) {
        String[] v = ACCENTS.get(id); return v != null ? v[0] : id;
    }
    public static String getPrimaryFor(String id) {
        String[] v = ACCENTS.get(id); return v != null ? v[1] : "#55de1b";
    }

    // ── CSS generation ────────────────────────────────────────────────────

    public String generateCss() {
        String p  = val("primary");
        String h  = val("hover");
        String pr = val("pressed");
        String grad = "linear-gradient(to right," + p + "," + pr + ")";
        String selBg = darken(p, 0.55);

        StringBuilder sb = new StringBuilder();
        sb.append("/* Accent color override – auto-generated */\n\n");

        // .jfx-button (main action buttons)
        sb.append(".jfx-button { -fx-background-color: ").append(p).append("; }\n");
        sb.append(".jfx-button:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append(".jfx-button:pressed, .jfx-button:armed { -fx-background-color: ").append(pr).append("; }\n");
        sb.append(".jfx-button:focused { -fx-background-color: ").append(p).append("; }\n\n");

        // #playButton
        sb.append("#playButton { -fx-background-color: ").append(p).append("; }\n");
        sb.append("#playButton:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append("#playButton:pressed { -fx-background-color: ").append(pr).append("; }\n\n");

        // .confirm-button (GameRemover yes)
        sb.append(".confirm-button { -fx-background-color: ").append(p).append("; }\n");
        sb.append(".confirm-button:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append(".confirm-button:pressed { -fx-background-color: ").append(pr).append("; }\n\n");

        // scroll-bar thumb
        sb.append(".scroll-bar .thumb { -fx-background-color: ").append(p).append("; }\n");
        sb.append(".scroll-bar .thumb:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append(".scroll-bar .thumb:pressed { -fx-background-color: ").append(pr).append("; }\n\n");

        // check-box mark
        sb.append(".check-box:selected .box .mark { -fx-background-color: ").append(p).append("; }\n\n");

        // text highlight
        sb.append(".text-input, .text-area .content, .text-area, .text-area .scroll-pane {\n");
        sb.append("  -fx-highlight-fill: ").append(grad).append(";\n}\n\n");

        // list-cell / table selection
        sb.append(".list-cell:filled:selected, .list-cell:filled:selected:hover,\n");
        sb.append(".table-row-cell:selected, .table-row-cell:selected .table-cell {\n");
        sb.append("  -fx-background-color: ").append(selBg).append(";\n");
        sb.append("  -fx-text-fill: ").append(p).append(";\n}\n\n");

        // toggle-switch selected
        sb.append(".toggle-switch:selected {\n");
        sb.append("  -fx-background-color: white, ").append(grad).append(";\n");
        sb.append("  -fx-background-insets: 3 3 3 22, 0;\n");
        sb.append("  -fx-background-radius: 9px, 12px;\n}\n");
        sb.append(".toggle-switch:selected .thumb-area { -fx-background-color: ").append(grad).append("; }\n\n");

        // toggle-switch rippler
        sb.append(".toggle-switch .jfx-rippler { -fx-background-color: ").append(p).append("; }\n\n");

        // progress bar
        sb.append(".progress-bar .bar { -fx-background-color: ").append(p).append("; }\n");
        sb.append(".jfx-progress-bar .bar { -fx-background-color: ").append(p).append("; }\n\n");

        // tooltip
        sb.append(".tooltip { -fx-text-fill: ").append(p).append("; -fx-border-color: ").append(p).append("; }\n\n");

        // table column header border
        sb.append(".table-view .column-header {\n");
        sb.append("  -fx-border-color: transparent transparent ").append(pr).append(" transparent;\n}\n\n");

        // #addGame
        sb.append("#addGame { -fx-background-color: ").append(p).append("; }\n");
        sb.append("#addGame:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append("#addGame:pressed { -fx-background-color: ").append(pr).append("; }\n");
        sb.append("#addGame:focused { -fx-background-color: ").append(p).append("; }\n\n");

        // #selectFileButton
        sb.append("#selectFileButton { -fx-background-color: ").append(p).append("; }\n");
        sb.append("#selectFileButton:hover { -fx-background-color: ").append(h).append("; }\n");
        sb.append("#selectFileButton:pressed { -fx-background-color: ").append(pr).append("; }\n");
        sb.append("#selectFileButton:focused { -fx-background-color: ").append(p).append("; }\n\n");

        // .accent-badge (version badge, etc.)
        sb.append(".accent-badge { -fx-background-color: ").append(p).append("; }\n");

        return sb.toString();
    }

    // ── Persistence ───────────────────────────────────────────────────────

    private void save() {
        try {
            AppModule.getInstance().setLauncher("accentColor", currentId, "User Settings");
        } catch (Exception e) {
            LOG.warn("Failed to save accent color", e);
        }
    }

    private void load() {
        try {
            String v = AppModule.getInstance().getLauncher("accentColor", "User Settings");
            if (v != null && ACCENTS.containsKey(v)) currentId = v;
        } catch (Exception e) {
            LOG.warn("Failed to load accent color", e);
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    private String val(String key) {
        String[] v = ACCENTS.get(currentId);
        if (v == null) v = ACCENTS.get("green");
        return switch (key) {
            case "primary" -> v[1];
            case "hover"   -> v[2];
            case "pressed" -> v[3];
            default        -> v[1];
        };
    }

    private static String darken(String hex, double amount) {
        int r = Integer.parseInt(hex.substring(1, 3), 16);
        int g = Integer.parseInt(hex.substring(3, 5), 16);
        int b = Integer.parseInt(hex.substring(5, 7), 16);
        r = (int) (r * (1 - amount));
        g = (int) (g * (1 - amount));
        b = (int) (b * (1 - amount));
        return String.format("#%02X%02X%02X",
                Math.max(0, Math.min(255, r)),
                Math.max(0, Math.min(255, g)),
                Math.max(0, Math.min(255, b)));
    }
}

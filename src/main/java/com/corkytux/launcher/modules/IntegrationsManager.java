package com.corkytux.launcher.modules;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * IntegrationsManager – external gaming platform integrations.
 * <ul>
 *   <li>Steam: native library scan (libraryfolders.vdf + appmanifest_*.acf)</li>
 *   <li>Lutris: local installed games scan (pga.db sqlite or lutris CLI)</li>
 *   <li>ProtonDB: compatibility summaries via public API (no key)</li>
 *   <li>SteamGridDB: cover downloads (requires user API key)</li>
 *   <li>IGDB: game info/ratings (requires Twitch client id + secret)</li>
 * </ul>
 */
public class IntegrationsManager {

    private static final Logger LOG = LoggerFactory.getLogger(IntegrationsManager.class);
    private static IntegrationsManager instance;

    private final HttpClient http;

    /** Bundled SteamGridDB key – automatic cover downloads for all users, no setup. */
    public static final String DEFAULT_STEAMGRIDDB_KEY = "0ab12f62e2d5e6b3717161be0c5e68fa";

    private IntegrationsManager() {
        http = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(10))
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    public static synchronized IntegrationsManager getInstance() {
        if (instance == null) instance = new IntegrationsManager();
        return instance;
    }

    // ── XDG-aware data dir resolution (native + Flatpak) ──────────────────

    /** XDG data home, defaults to ~/.local/share. */
    public static Path xdgDataHome() {
        String xdg = System.getenv("XDG_DATA_HOME");
        if (xdg != null && !xdg.isBlank()) return Path.of(xdg);
        return Path.of(FilesWorker.getExpectedHome(), ".local/share");
    }

    /**
     * Lutris data dir candidates in priority order:
     * native (~/.local/share/lutris) then Flatpak (~/.var/app/net.lutris.Lutris/data/lutris).
     */
    public static List<Path> lutrisDataDirs() {
        var dirs = new ArrayList<Path>();
        var nativeDir = xdgDataHome().resolve("lutris");
        if (Files.isDirectory(nativeDir)) dirs.add(nativeDir);
        var flatpakDir = Path.of(FilesWorker.getExpectedHome(),
                ".var/app/net.lutris.Lutris/data/lutris");
        if (Files.isDirectory(flatpakDir) && !dirs.contains(flatpakDir)) dirs.add(flatpakDir);
        if (dirs.isEmpty()) dirs.add(nativeDir); // default for path construction
        return dirs;
    }

    /** First existing Lutris data dir, or native default. */
    public static Path lutrisDataDir() {
        return lutrisDataDirs().get(0);
    }

    /** System hicolor icons dir (XDG-aware). */
    public static Path hicolorAppsDir() {
        return xdgDataHome().resolve("icons/hicolor/128x128/apps");
    }

    // ── Enable flags (persisted in Launcher.ini) ──────────────────────────

    public boolean isEnabled(String key) {
        try {
            String v = AppModule.getInstance().getLauncher("integration_" + key, "Integrations");
            return "1".equals(v) || "true".equalsIgnoreCase(v);
        } catch (Exception e) {
            return false;
        }
    }

    public void setEnabled(String key, boolean enabled) {
        try {
            AppModule.getInstance().setLauncher("integration_" + key, enabled ? "1" : "0", "Integrations");
        } catch (Exception e) {
            LOG.warn("Failed to save integration flag {}", key, e);
        }
    }

    public String getKey(String key) {
        try {
            String v = AppModule.getInstance().getLauncher("integration_key_" + key, "Integrations");
            if (v != null && !v.isBlank()) return v;
        } catch (Exception e) {
            // fall through to default
        }
        // Bundled default (SteamGridDB works out of the box)
        if ("steamgriddb".equals(key)) return DEFAULT_STEAMGRIDDB_KEY;
        return null;
    }

    public void setKey(String key, String value) {
        try {
            AppModule.getInstance().setLauncher("integration_key_" + key, value != null ? value : "", "Integrations");
        } catch (Exception e) {
            LOG.warn("Failed to save integration key {}", key, e);
        }
    }

    // ── Steam native library ──────────────────────────────────────────────

    /** Result of scanning one Steam game. */
    public record SteamGame(String appId, String name, String installDir, String libraryPath) {}

    /**
     * Finds Steam install roots (native + flatpak).
     */
    public List<Path> findSteamRoots() {
        var roots = new ArrayList<Path>();
        String home = FilesWorker.getExpectedHome();
        for (String candidate : new String[]{
                home + "/.local/share/Steam",
                home + "/.steam/steam",
                home + "/.var/app/com.valvesoftware.Steam/data/Steam"}) {
            var p = Path.of(candidate);
            if (Files.isDirectory(p)) roots.add(p);
        }
        return roots;
    }

    /**
     * Parses libraryfolders.vdf to get all library paths, falling back to the
     * Steam root itself when the file is missing.
     */
    public List<Path> findSteamLibraries(Path steamRoot) {
        var libs = new ArrayList<Path>();
        libs.add(steamRoot);
        var vdf = steamRoot.resolve("steamapps/libraryfolders.vdf");
        if (!Files.isRegularFile(vdf)) return libs;
        try {
            String content = Files.readString(vdf);
            // Very small VDF parser: collect all "path" values
            var matcher = java.util.regex.Pattern.compile("\"path\"\\s+\"([^\"]+)\"").matcher(content);
            while (matcher.find()) {
                String raw = matcher.group(1).replace("\\\\", "/");
                var p = Path.of(raw);
                if (Files.isDirectory(p) && !libs.contains(p)) libs.add(p);
            }
        } catch (Exception e) {
            LOG.debug("Failed to parse {}", vdf, e);
        }
        return libs;
    }

    /**
     * Scans all Steam libraries for installed games (appmanifest_*.acf).
     */
    public List<SteamGame> scanSteamLibrary() {
        var games = new ArrayList<SteamGame>();
        for (Path root : findSteamRoots()) {
            for (Path lib : findSteamLibraries(root)) {
                var steamapps = lib.resolve("steamapps");
                if (!Files.isDirectory(steamapps)) continue;
                try (DirectoryStream<Path> ds = Files.newDirectoryStream(steamapps, "appmanifest_*.acf")) {
                    for (Path acf : ds) {
                        try {
                            SteamGame g = parseAcf(acf, steamapps);
                            if (g != null) games.add(g);
                        } catch (Exception e) {
                            LOG.debug("Failed to parse {}", acf, e);
                        }
                    }
                } catch (Exception e) {
                    LOG.debug("Failed to list {}", steamapps, e);
                }
            }
        }
        LOG.info("Steam scan: {} games found", games.size());
        return games;
    }

    private SteamGame parseAcf(Path acf, Path steamapps) throws IOException {
        String content = Files.readString(acf);
        String appId = extractVdfValue(content, "appid");
        String name = extractVdfValue(content, "name");
        String installDir = extractVdfValue(content, "installdir");
        if (appId == null) return null;
        // Fallback: use installdir when name missing or numeric
        if (name == null || name.isBlank() || name.matches("\\d+")) {
            name = installDir != null ? installDir : ("Steam " + appId);
        }
        String libPath = installDir != null
                ? steamapps.resolve("common/" + installDir).toString() : "";
        return new SteamGame(appId, name, installDir != null ? installDir : "", libPath);
    }

    private static String extractVdfValue(String vdf, String key) {
        var m = java.util.regex.Pattern.compile("\"" + key + "\"\\s+\"([^\"]+)\"").matcher(vdf);
        return m.find() ? m.group(1) : null;
    }

    // ── Steam CDN artwork (free, no key) ──────────────────────────────────

    /**
     * Downloads Steam store artwork (header banner + small capsule icon)
     * via the public CDN into the CorkyTux banners/icons dirs.
     * Falls back to Store API (appdetails) when CDN direct 404s.
     * Returns {bannerPath, iconPath} (either may be null on failure).
     */
    public Map<String, String> fetchSteamArtwork(String appId) {
        var out = new LinkedHashMap<String, String>();
        if (appId == null || appId.isBlank()) return out;
        String home = FilesWorker.getExpectedHome();
        var bannerDest = Path.of(home, ".config", "CorkyTux", "banners", appId.trim() + ".jpg");
        var iconDest = Path.of(home, ".config", "CorkyTux", "icons", appId.trim() + ".jpg");
        if (downloadTo("https://cdn.cloudflare.steamstatic.com/steam/apps/"
                + appId.trim() + "/header.jpg", bannerDest)) {
            out.put("banner", bannerDest.toString());
        }
        if (downloadTo("https://cdn.cloudflare.steamstatic.com/steam/apps/"
                + appId.trim() + "/capsule_184x69.jpg", iconDest)) {
            out.put("icon", iconDest.toString());
        }
        // Fallback: Steam Store API (different CDN host with hash)
        if (!out.containsKey("banner")) {
            try {
                var req = HttpRequest.newBuilder(
                        URI.create("https://store.steampowered.com/api/appdetails?appids=" + appId.trim()))
                        .timeout(Duration.ofSeconds(10)).GET().build();
                var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
                if (resp.statusCode() == 200) {
                    var m = java.util.regex.Pattern.compile("\"header_image\"\\s*:\\s*\"([^\"]+)\"")
                            .matcher(resp.body());
                    if (m.find()) {
                        String imgUrl = m.group(1).replace("\\/", "/");
                        var dest = Path.of(home, ".config", "CorkyTux", "banners", appId.trim() + "-store.jpg");
                        if (downloadTo(imgUrl, dest)) out.put("banner", dest.toString());
                    }
                }
            } catch (Exception e) {
                LOG.debug("Store API art failed for {}", appId, e);
            }
        }
        return out;
    }

    /** Downloads URL to dest if HTTP 200 and body >1KB. Returns true on success. */
    private boolean downloadTo(String url, Path dest) {
        try {
            var req = HttpRequest.newBuilder(URI.create(url))
                    .timeout(Duration.ofSeconds(20)).GET().build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofByteArray());
            if (resp.statusCode() != 200 || resp.body().length <= 1000) return false;
            Files.createDirectories(dest.getParent());
            Files.write(dest, resp.body());
            return true;
        } catch (Exception e) {
            LOG.debug("download failed: {}", url, e);
            return false;
        }
    }

    /**
     * Fetches cover art from Lutris.net public API (free, no key) by game name.
     * Returns {banner} with local path, or empty map. Picks best name match.
     */
    public Map<String, String> fetchLutrisNetArtwork(String gameName) {
        var out = new LinkedHashMap<String, String>();
        if (gameName == null || gameName.isBlank()) return out;
        try {
            String enc = java.net.URLEncoder.encode(gameName.trim(),
                    java.nio.charset.StandardCharsets.UTF_8);
            var req = HttpRequest.newBuilder(
                    URI.create("https://lutris.net/api/games?search=" + enc))
                    .timeout(Duration.ofSeconds(10))
                    .header("User-Agent", "CorkyTux/2.8")
                    .GET().build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return out;
            String body = resp.body();
            // Find all {name, coverart} pairs, pick closest name match with art
            var namePat = java.util.regex.Pattern.compile("\"name\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"");
            var artPat = java.util.regex.Pattern.compile("\"coverart\"\\s*:\\s*\"([^\"]+)\"");
            var names = new java.util.ArrayList<String>();
            var arts = new java.util.ArrayList<String>();
            var nm = namePat.matcher(body);
            while (nm.find()) names.add(nm.group(1));
            var am = artPat.matcher(body);
            while (am.find()) arts.add(am.group(1));
            String want = gameName.trim().toLowerCase(java.util.Locale.ROOT);
            String bestUrl = null;
            // Exact match first
            for (int i = 0; i < Math.min(names.size(), arts.size() + 4); i++) {
                // coverart appears after name in each result object; approximate pairing
            }
            // Simpler robust pairing: split results by "coverart" occurrences
            var results = body.split("\\{\"id\"");
            for (String r : results) {
                var n2 = namePat.matcher(r);
                var a2 = artPat.matcher(r);
                if (n2.find() && a2.find()) {
                    String rn = n2.group(1).trim();
                    String url = a2.group(1);
                    if (url == null || url.isBlank() || url.equals("null")) continue;
                    if (rn.equalsIgnoreCase(gameName.trim())) { bestUrl = url; break; }
                    if (bestUrl == null && rn.toLowerCase(java.util.Locale.ROOT).contains(want)
                            || want.contains(rn.toLowerCase(java.util.Locale.ROOT))) {
                        bestUrl = url;
                    }
                }
            }
            if (bestUrl != null) {
                String home = FilesWorker.getExpectedHome();
                var dest = Path.of(home, ".config", "CorkyTux", "banners",
                        slugify(gameName) + "-lutris.jpg");
                if (downloadTo(bestUrl, dest)) out.put("banner", dest.toString());
            }
        } catch (Exception e) {
            LOG.debug("Lutris.net fetch failed for {}", gameName, e);
        }
        return out;
    }

    // ── Lutris ────────────────────────────────────────────────────────────

    /** Result of scanning one Lutris game. */
    public record LutrisGame(String slug, String name, String runner, String directory,
            String executable, String prefix, double playtimeHours) {}

    /**
     * Scans Lutris installed games via `lutris -l` (text table) or pga.db.
     * Returns empty list when lutris is not installed.
     */
    public List<LutrisGame> scanLutrisLibrary() {
        var games = new ArrayList<LutrisGame>();
        // Primary: pga.db (complete: slug, dir, exe, playtime + YAML resolution)
        try {
            games.addAll(scanLutrisPgaDb());
            if (!games.isEmpty()) {
                LOG.info("Lutris scan via pga.db: {} games", games.size());
                return games;
            }
        } catch (Exception e) {
            LOG.debug("Lutris pga.db scan failed", e);
        }
        // Fallback: lutris CLI (names only, no paths)
        try {
            var pb = new ProcessBuilder("lutris", "-l");
            pb.redirectErrorStream(true);
            var proc = pb.start();
            String out = new String(proc.getInputStream().readAllBytes());
            proc.waitFor();
            for (String line : out.split("\n")) {
                line = line.trim();
                if (line.isEmpty() || line.startsWith("Name") || line.startsWith("-")) continue;
                // Skip Gtk warnings / log noise merged from stderr
                if (line.startsWith("[") || line.startsWith("(") || line.contains("WARNING")
                        || line.contains("Starting Lutris") || line.contains("Shutting down")
                        || line.contains("libgnutls") || line.contains("is AMD")
                        || !line.contains("|")) continue;
                // Format: [Idx |] Name | Runner ... – name is first non-numeric column
                String[] cols = line.split("\\|");
                String name = "";
                String runner = "";
                // Find first non-numeric, non-empty column as name
                int nameIdx = -1;
                for (int i = 0; i < cols.length; i++) {
                    String c = cols[i].trim();
                    if (!c.isEmpty() && !c.matches("\\d+") && !c.equalsIgnoreCase("yes") && !c.equalsIgnoreCase("no")) {
                        name = c;
                        nameIdx = i;
                        break;
                    }
                }
                if (nameIdx >= 0 && nameIdx + 1 < cols.length) runner = cols[nameIdx + 1].trim();
                if (!name.isEmpty() && !name.matches("\\d+")) games.add(new LutrisGame("", name, runner, "", "", "", 0));
            }
            if (!games.isEmpty()) {
                LOG.info("Lutris scan via CLI: {} games (names only)", games.size());
                return games;
            }
        } catch (Exception e) {
            LOG.debug("lutris CLI not available", e);
        }
        return games;
    }

    /**
     * Scans Lutris pga.db (all known data dirs) with YAML exe/prefix resolution.
     */
    private List<LutrisGame> scanLutrisPgaDb() {
        var games = new ArrayList<LutrisGame>();
        // (sqlite – read via sqlite3 CLI)
        try {
            Path db = null;
            for (Path dir : lutrisDataDirs()) {
                var candidate = dir.resolve("pga.db");
                if (Files.isRegularFile(candidate)) { db = candidate; break; }
            }
            if (db == null) return games;
            var pb = new ProcessBuilder("sqlite3", db.toString(),
                    "SELECT slug,name,runner,directory,executable,configpath,playtime FROM games WHERE installed=1;");
            pb.redirectErrorStream(true);
            var proc = pb.start();
            String out = new String(proc.getInputStream().readAllBytes());
            proc.waitFor();
            for (String line : out.split("\n")) {
                String[] cols = line.split("\\|", -1);
                if (cols.length >= 2 && !cols[1].isBlank()) {
                    String slug = cols[0];
                    String nm = cols[1];
                    String runner = cols.length > 2 ? cols[2] : "";
                    String dir = cols.length > 3 ? cols[3] : "";
                    String exe = cols.length > 4 ? cols[4] : "";
                    String cfg = cols.length > 5 ? cols[5] : "";
                    double pt = 0;
                    if (cols.length > 6) try { pt = Double.parseDouble(cols[6].trim()); } catch (Exception ignored) {}
                    // Resolve missing exe/dir/prefix from Lutris game YAML (authoritative)
                    String prefix = "";
                    if ((exe.isBlank() || dir.isBlank()) && !cfg.isBlank()) {
                        var resolved = resolveLutrisYaml(cfg);
                        if (exe.isBlank() && resolved.containsKey("exe")) exe = resolved.get("exe");
                        if (dir.isBlank() && exe != null && !exe.isBlank()) {
                            try {
                                var p = Path.of(exe);
                                if (p.getParent() != null) dir = p.getParent().toString();
                            } catch (Exception ignored) {}
                        }
                        if (resolved.containsKey("prefix")) prefix = resolved.get("prefix");
                    }
                    games.add(new LutrisGame(slug, nm, runner, dir, exe, prefix, pt));
                }
            }
            LOG.info("Lutris scan via pga.db: {} games", games.size());
        } catch (Exception e) {
            LOG.debug("Lutris pga.db scan failed", e);
        }
        return games;
    }

    // ── Unified artwork resolver (any game) ───────────────────────────────

    /**
     * Resolves banner + icon for ANY game using all available sources:
     * <ol>
     *   <li>Steam CDN by steamID (free, no key) → header.jpg + capsule</li>
     *   <li>Lutris local art by slugified name (coverart/banner + hicolor icon)</li>
     *   <li>SteamGridDB search by name (needs API key) → grid cover</li>
     * </ol>
     * Downloads into CorkyTux banners/icons dirs. Returns {banner, icon} paths
     * (keys absent when that source failed). Never returns Lutris-referencing paths.
     */
    public Map<String, String> resolveArtwork(String gameName, String steamId) {
        var out = new LinkedHashMap<String, String>();
        // 1) Steam CDN by AppID
        if (steamId != null && !steamId.isBlank()) {
            try {
                var art = fetchSteamArtwork(steamId.trim());
                out.putAll(art);
            } catch (Exception e) {
                LOG.debug("Steam CDN art failed for {}", steamId, e);
            }
        }
        // 2) Lutris local by slugified name
        if ((!out.containsKey("banner") || !out.containsKey("icon")) && gameName != null) {
            try {
                String slug = slugify(gameName);
                for (Path dir : lutrisDataDirs()) {
                    if (!out.containsKey("banner")) {
                        for (String artDir : new String[]{"coverart", "banners"}) {
                            var src = dir.resolve(artDir + "/" + slug + ".jpg");
                            if (Files.isRegularFile(src)) {
                                var dest = copyArtwork(src, "banners", slug + ".jpg");
                                if (dest != null) { out.put("banner", dest); break; }
                            }
                        }
                    }
                    if (!out.containsKey("icon")) {
                        var hicolor = hicolorAppsDir().resolve("lutris_" + slug + ".png");
                        if (Files.isRegularFile(hicolor)) {
                            var dest = copyArtwork(hicolor, "icons", slug + ".png");
                            if (dest != null) out.put("icon", dest);
                        }
                    }
                    if (out.containsKey("banner") && out.containsKey("icon")) break;
                }
            } catch (Exception e) {
                LOG.debug("Lutris art lookup failed for {}", gameName, e);
            }
        }
        // 3) Lutris.net public API by name (free, no key) – IGDB-sourced covers
        if ((!out.containsKey("banner") || !out.containsKey("icon")) && gameName != null) {
            try {
                var lutrisArt = fetchLutrisNetArtwork(gameName);
                if (!out.containsKey("banner") && lutrisArt.containsKey("banner")) {
                    out.put("banner", lutrisArt.get("banner"));
                }
                out.putIfAbsent("icon", lutrisArt.getOrDefault("icon",
                        lutrisArt.getOrDefault("banner", null)));
                out.values().removeIf(java.util.Objects::isNull);
            } catch (Exception e) {
                LOG.debug("Lutris.net art failed for {}", gameName, e);
            }
        }
        // 4) SteamGridDB by name (needs key)
        if ((!out.containsKey("banner") || !out.containsKey("icon")) && gameName != null) {
            try {
                String gridId = steamGridSearchGame(gameName);
                if (gridId != null) {
                    String home = FilesWorker.getExpectedHome();
                    var dest = Path.of(home, ".config", "CorkyTux", "banners",
                            slugify(gameName) + "-sgdb.jpg");
                    if (!out.containsKey("banner") && steamGridDownloadCover(gridId, dest)) {
                        out.put("banner", dest.toString());
                        // Grid covers are portrait – also usable as fallback icon
                        out.putIfAbsent("icon", dest.toString());
                    }
                }
            } catch (Exception e) {
                LOG.debug("SteamGridDB art failed for {}", gameName, e);
            }
        }
        return out;
    }

    /** URL/filename-safe slug: lowercase, non-alnum → dash. */
    public static String slugify(String name) {
        if (name == null) return "";
        return name.toLowerCase(java.util.Locale.ROOT)
                .replaceAll("[^a-z0-9]+", "-").replaceAll("^-|-$", "");
    }

    /** Copies artwork src into ~/.config/CorkyTux/{subdir}/{name}, returns dest path or null. */
    public static String copyArtwork(Path src, String subdir, String name) {
        try {
            var dest = Path.of(FilesWorker.getExpectedHome(), ".config", "CorkyTux", subdir, name);
            Files.createDirectories(dest.getParent());
            Files.copy(src, dest, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            return dest.toString();
        } catch (Exception e) {
            LOG.debug("copyArtwork failed", e);
            return null;
        }
    }

    /**
     * Parses a Lutris game YAML ({@code <lutris-data>/games/<configpath>.yml})
     * for exe / main_file / prefix. Minimal line parser (no SnakeYAML dep).
     * Returns map possibly containing "exe" and "prefix".
     */
    public static Map<String, String> resolveLutrisYaml(String configpath) {
        var out = new LinkedHashMap<String, String>();
        if (configpath == null || configpath.isBlank()) return out;
        try {
            Path yml = null;
            for (Path dir : lutrisDataDirs()) {
                var candidate = dir.resolve("games/" + configpath.trim() + ".yml");
                if (Files.isRegularFile(candidate)) { yml = candidate; break; }
            }
            if (yml == null) return out;
            boolean inGame = false;
            for (String raw : Files.readAllLines(yml)) {
                String line = raw.strip();
                if (line.equals("game:")) { inGame = true; continue; }
                if (inGame && !raw.startsWith(" ") && !raw.startsWith("\t") && line.endsWith(":")) {
                    inGame = false; // next top-level section
                }
                if (!inGame) continue;
                for (String key : new String[]{"exe", "main_file", "prefix", "game_path"}) {
                    if (line.startsWith(key + ":")) {
                        String val = line.substring(key.length() + 1).strip();
                        // strip quotes
                        if (val.length() >= 2 && ((val.startsWith("\"") && val.endsWith("\""))
                                || (val.startsWith("'") && val.endsWith("'")))) {
                            val = val.substring(1, val.length() - 1);
                        }
                        if (!val.isEmpty()) {
                            if (key.equals("main_file") || key.equals("game_path")) out.putIfAbsent("exe", val);
                            else out.putIfAbsent(key, val);
                        }
                    }
                }
            }
        } catch (Exception e) {
            LOG.debug("Lutris YAML parse failed for {}", configpath, e);
        }
        return out;
    }

    /**
     * Fetches ProtonDB tier + confidence for a Steam app id.
     * Returns map with tier/confidence or null on failure.
     */
    public Map<String, String> fetchProtonDbRating(String appId) {
        if (appId == null || appId.isBlank()) return null;
        try {
            var req = HttpRequest.newBuilder(
                    URI.create("https://www.protondb.com/api/v1/reports/summaries/"
                            + appId.trim() + ".json"))
                    .timeout(Duration.ofSeconds(10))
                    .header("User-Agent", "CorkyTux/2.8")
                    .GET().build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return null;
            String body = resp.body();
            var out = new LinkedHashMap<String, String>();
            var tier = java.util.regex.Pattern.compile("\"tier\"\\s*:\\s*\"([^\"]+)\"").matcher(body);
            if (tier.find()) out.put("tier", tier.group(1));
            var conf = java.util.regex.Pattern.compile("\"confidence\"\\s*:\\s*\"([^\"]+)\"").matcher(body);
            if (conf.find()) out.put("confidence", conf.group(1));
            var total = java.util.regex.Pattern.compile("\"total\"\\s*:\\s*(\\d+)").matcher(body);
            if (total.find()) out.put("total", total.group(1));
            return out.isEmpty() ? null : out;
        } catch (Exception e) {
            LOG.debug("ProtonDB fetch failed for {}", appId, e);
            return null;
        }
    }

    // ── SteamGridDB (requires API key) ────────────────────────────────────

    /**
     * Searches SteamGridDB for a game id by name. Returns grid id or null.
     */
    public String steamGridSearchGame(String name) {
        String key = getKey("steamgriddb");
        if (key == null || key.isBlank() || name == null || name.isBlank()) return null;
        try {
            String enc = java.net.URLEncoder.encode(name, java.nio.charset.StandardCharsets.UTF_8);
            var req = HttpRequest.newBuilder(
                    URI.create("https://www.steamgriddb.com/api/v2/search/autocomplete/" + enc))
                    .timeout(Duration.ofSeconds(10))
                    .header("Authorization", "Bearer " + key.trim())
                    .GET().build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return null;
            var m = java.util.regex.Pattern.compile("\"id\"\\s*:\\s*(\\d+)").matcher(resp.body());
            return m.find() ? m.group(1) : null;
        } catch (Exception e) {
            LOG.debug("SteamGridDB search failed for {}", name, e);
            return null;
        }
    }

    /**
     * Downloads a grid cover (600x900) for a SteamGridDB game id into dest file.
     * Returns true on success.
     */
    public boolean steamGridDownloadCover(String gridId, Path dest) {
        String key = getKey("steamgriddb");
        if (key == null || key.isBlank() || gridId == null) return false;
        try {
            var req = HttpRequest.newBuilder(
                    URI.create("https://www.steamgriddb.com/api/v2/grids/game/" + gridId
                            + "?dimensions=600x900&types=static"))
                    .timeout(Duration.ofSeconds(10))
                    .header("Authorization", "Bearer " + key.trim())
                    .GET().build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return false;
            var m = java.util.regex.Pattern.compile("\"url\"\\s*:\\s*\"([^\"]+)\"").matcher(resp.body());
            if (!m.find()) return false;
            String imgUrl = m.group(1).replace("\\/", "/");
            var imgReq = HttpRequest.newBuilder(URI.create(imgUrl))
                    .timeout(Duration.ofSeconds(30)).GET().build();
            var imgResp = http.send(imgReq, HttpResponse.BodyHandlers.ofByteArray());
            if (imgResp.statusCode() != 200) return false;
            Files.createDirectories(dest.getParent());
            Files.write(dest, imgResp.body());
            LOG.info("SteamGridDB cover saved to {}", dest);
            return true;
        } catch (Exception e) {
            LOG.debug("SteamGridDB download failed for {}", gridId, e);
            return false;
        }
    }

    // ── IGDB (requires Twitch client-id + secret) ─────────────────────────

    private volatile String igdbToken;
    private volatile long igdbTokenExpiry;

    /**
     * Fetches IGDB game summary (rating, summary) by name.
     * Credentials stored as "clientId:clientSecret" under key "igdb".
     */
    public Map<String, String> igdbFetchGame(String name) {
        String creds = getKey("igdb");
        if (creds == null || !creds.contains(":") || name == null || name.isBlank()) return null;
        try {
            String[] parts = creds.split(":", 2);
            String token = ensureIgdbToken(parts[0].trim(), parts[1].trim());
            if (token == null) return null;
            String query = "search \"" + name.replace("\"", "") + "\"; fields name,rating,summary,cover.url; limit 1;";
            var req = HttpRequest.newBuilder(URI.create("https://api.igdb.com/v4/games"))
                    .timeout(Duration.ofSeconds(10))
                    .header("Client-ID", parts[0].trim())
                    .header("Authorization", "Bearer " + token)
                    .POST(HttpRequest.BodyPublishers.ofString(query)).build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return null;
            String body = resp.body();
            var out = new LinkedHashMap<String, String>();
            var r = java.util.regex.Pattern.compile("\"rating\"\\s*:\\s*([\\d.]+)").matcher(body);
            if (r.find()) out.put("rating", r.group(1));
            var s = java.util.regex.Pattern.compile("\"summary\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"").matcher(body);
            if (s.find()) out.put("summary", s.group(1).replace("\\\"", "\""));
            return out.isEmpty() ? null : out;
        } catch (Exception e) {
            LOG.debug("IGDB fetch failed for {}", name, e);
            return null;
        }
    }

    private synchronized String ensureIgdbToken(String clientId, String secret) {
        if (igdbToken != null && System.currentTimeMillis() < igdbTokenExpiry) return igdbToken;
        try {
            var req = HttpRequest.newBuilder(URI.create(
                    "https://id.twitch.tv/oauth2/token?client_id=" + clientId
                            + "&client_secret=" + secret + "&grant_type=client_credentials"))
                    .timeout(Duration.ofSeconds(10))
                    .POST(HttpRequest.BodyPublishers.noBody()).build();
            var resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() != 200) return null;
            var m = java.util.regex.Pattern.compile("\"access_token\"\\s*:\\s*\"([^\"]+)\"").matcher(resp.body());
            if (!m.find()) return null;
            igdbToken = m.group(1);
            igdbTokenExpiry = System.currentTimeMillis() + 3_600_000L; // ~1h
            return igdbToken;
        } catch (Exception e) {
            LOG.debug("IGDB token failed", e);
            return null;
        }
    }
}

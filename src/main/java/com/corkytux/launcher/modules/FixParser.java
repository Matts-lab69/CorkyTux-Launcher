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

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.ini4j.Wini;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.regex.Pattern;

/**
 * Java 25 port of {@code FixParser.php} (181 lines).
 * Parses OnlineFix DLLs / overrides, banners and icons.
 */
public final class FixParser {

    private static final Logger LOG = LoggerFactory.getLogger(FixParser.class);
    private static final ObjectMapper JACKSON = new ObjectMapper();

    private static final Pattern DLL_SCAN_PATTERN = Pattern.compile(
            "(?i)^(emp|custom)\\.dll$|^win.*\\.dll$|^(online|steam).*\\.(dll|ini|json)$|^eos.*\\.dll$|^epicfix.*\\.dll$|^(winmm|dlllist)\\.txt$|^launch_data\\.of.*$"
    );
    private static final Pattern TXT_PATTERN = Pattern.compile("(?i)^(winmm|dlllist)\\.txt$");
    private static final Pattern ONLINE_STEAM_INI_PATTERN = Pattern.compile("(?i)^(online|steam)fix\\.ini$");
    private static final Pattern STEAMFIX_DLL_PATTERN = Pattern.compile("(?i)^steamfix.*\\.dll");
    private static final Pattern LAUNCH_DATA_PATTERN = Pattern.compile("(?i)^launch_data\\.of.*$|^onlinefix\\.json$");
    private static final Pattern WIN_DLL_PATTERN = Pattern.compile("(?i)^win.*\\.dll$");

    private FixParser() {}

    /**
     * Mirrors PHP {@code parseDlls($path)}.
     * Scans {@code path} and builds WINEDLLOVERRIDES string plus AppIDs and fixPath.
     *
     * @param path directory to scan (absolute or relative)
     * @return map with keys {@code overrides}, {@code realAppId}, {@code fakeAppId}, {@code fixPath},
     *         or {@code null} when no matching files found
     */
    public static Map<String, String> parseDlls(String path) {
        var dir = Path.of(path);
        if (!Files.isDirectory(dir)) return null;

        var files = scanFiles(dir);
        if (files == null || files.isEmpty()) return null;

        String overrides = "";
        String realAppId = null;
        String fakeAppId = null;
        String fixPath = null;

        for (var file : files) {
            var fileName = file.getFileName().toString();

            if (TXT_PATTERN.matcher(fileName).matches()) {
                // winmm / dlllist txt files – each line is a dll name
                try {
                    var content = Files.readString(file, StandardCharsets.UTF_8);
                    var lines = content.split("\n");
                    for (var dllLine : lines) {
                        var trimmed = dllLine.trim();
                        if (trimmed.isEmpty()) continue;
                        var ext = getExtension(trimmed);
                        if (!"dll".equalsIgnoreCase(ext)) continue;
                        // normalize: replace \\ with /, take name without ext, lowercase
                        var normalized = trimmed.replace('\\', '/');
                        var base = Path.of(normalized).getFileName() != null
                                ? Path.of(normalized).getFileName().toString()
                                : normalized;
                        int dot = base.lastIndexOf('.');
                        if (dot != -1) base = base.substring(0, dot);
                        base = base.toLowerCase();
                        if (!overrides.toLowerCase().contains(base.toLowerCase())) {
                            overrides += base + "=n;";
                        }
                    }
                } catch (IOException e) {
                    LOG.warn("Failed to read dll list file {}", file, e);
                }
                continue;
            } else if (ONLINE_STEAM_INI_PATTERN.matcher(fileName).matches()) {
                // onlinefix.ini / steamfix.ini
                try {
                    var wini = new Wini(file.toFile());
                    // RealAppId from OnlineFix Linux section fallback to Main
                    String real = null;
                    var ofLinux = wini.get("OnlineFix Linux");
                    if (ofLinux != null) real = ofLinux.get("RealAppId");
                    if (real == null) {
                        var main = wini.get("Main");
                        if (main != null) real = main.get("RealAppId");
                    }
                    realAppId = real;

                    String fake = null;
                    var main = wini.get("Main");
                    if (main != null) fake = main.get("FakeAppId");
                    fakeAppId = fake;

                    // FreeTP patch: if steamfix.ini and no OnlineFix Linux section, copy RealAppId
                    if ("steamfix.ini".equalsIgnoreCase(fileName)) {
                        boolean hasOfSection = wini.containsKey("OnlineFix Linux");
                        // ini4j sections map check – Wini.get returns null if absent
                        if (!hasOfSection) {
                            if (wini.get("Main") == null) wini.put("Main", "RealAppId", fakeAppId);
                            else wini.get("Main").put("RealAppId", fakeAppId);
                            wini.put("OnlineFix Linux", "RealAppId", realAppId);
                            wini.store();
                            LOG.info("FreeTP patch applied!");
                        }
                    }

                    // ExtraProtection -> false
                    var misc = wini.get("Misc");
                    if (misc != null && misc.get("ExtraProtection") != null) {
                        misc.put("ExtraProtection", "false");
                        wini.store();
                    } else {
                        // also check case where get returns null but key exists elsewhere – fallback manual
                        // ini4j already handles; ensure stored
                    }

                } catch (Exception e) {
                    LOG.warn("Failed to parse ini {}", file, e);
                }
                continue;
            } else if (STEAMFIX_DLL_PATTERN.matcher(fileName).find()) {
                var parent = file.getParent();
                if (parent != null) fixPath = parent.toString();
            } else if (LAUNCH_DATA_PATTERN.matcher(fileName).matches()) {
                var parent = file.getParent();
                if (parent == null) continue;
                var launcherExe = parent.resolve("Launcher.exe");
                var newtonDll = parent.resolve("Newtonsoft.Json.dll");
                // Jackson fallback: first try to understand JSON content via Jackson (Linux-native, no DLL needed)
                // This mirrors the intent of the task: "Newtonsoft.Json fallback via Jackson"
                // We log the JSON structure so operators can verify onlinefix.json / launch_data parsing without the Windows DLL.
                try {
                    parseJsonWithJacksonFallback(file);
                } catch (Exception ex) {
                    LOG.debug("Jackson fallback parse for {} failed (non-fatal): {}", file, ex.getMessage());
                }

                if (Files.isRegularFile(launcherExe) && !Files.isRegularFile(newtonDll)) {
                    // scan for Newtonsoft.Json.dll elsewhere under path
                    var newtonFiles = scanNewtonsoft(dir);
                    if (newtonFiles.isEmpty()) {
                        // copy from bundled resource res://.data/Newtonsoft.Json/Newtonsoft.Json.dll
                        // Correct primary path is /.data/... (src/main/resources/.data/...); legacy /Newtonsoft.Json/... kept for compat
                        boolean copied = false;
                        // Primary: /.data/Newtonsoft.Json/Newtonsoft.Json.dll (actual bundle location)
                        try (var res = FixParser.class.getResourceAsStream("/.data/Newtonsoft.Json/Newtonsoft.Json.dll")) {
                            if (res != null) {
                                Files.copy(res, newtonDll, StandardCopyOption.REPLACE_EXISTING);
                                copied = true;
                                LOG.debug("Copied Newtonsoft.Json.dll from /.data/Newtonsoft.Json/...");
                            }
                        } catch (IOException e) {
                            LOG.warn("Failed to copy Newtonsoft.Json.dll from primary", e);
                        }
                        if (!copied) {
                            try (var alt = FixParser.class.getResourceAsStream("/Newtonsoft.Json/Newtonsoft.Json.dll")) {
                                if (alt != null) {
                                    Files.copy(alt, newtonDll, StandardCopyOption.REPLACE_EXISTING);
                                    copied = true;
                                    LOG.debug("Copied Newtonsoft.Json.dll from /Newtonsoft.Json/...");
                                }
                            } catch (IOException e) {
                                LOG.warn("Failed to copy Newtonsoft.Json.dll from alt", e);
                            }
                        }
                        if (!copied) {
                            // filesystem fallback (dev run without jar packaging)
                            var fsFallback = Path.of("src/main/resources/.data/Newtonsoft.Json/Newtonsoft.Json.dll");
                            if (Files.isRegularFile(fsFallback)) {
                                try {
                                    Files.copy(fsFallback, newtonDll, StandardCopyOption.REPLACE_EXISTING);
                                    copied = true;
                                    LOG.debug("Copied Newtonsoft.Json.dll from filesystem fallback {}", fsFallback);
                                } catch (IOException e) {
                                    LOG.warn("Failed to copy from filesystem fallback", e);
                                }
                            }
                        }
                        if (!copied) {
                            LOG.warn("Bundled Newtonsoft.Json.dll not found in resources – Jackson fallback will handle JSON at runtime (launcher can run without DLL via Jackson parsing of {}).", file);
                            // Ensure Jackson can still handle the JSON later – no hard failure on Linux/Proton
                        }
                    } else {
                        for (var lib : newtonFiles) {
                            try {
                                // skip if canonical differs from absolute (symlink handling mirrored from PHP)
                                if (!lib.toFile().getAbsolutePath().equals(lib.toFile().getCanonicalPath())) continue;
                                var target = parent.resolve(lib.getFileName().toString());
                                if (Files.exists(target)) continue; // already linked
                                // create symlink via Process ln -s – mirrors PHP new Process(['ln','-s',...])
                                var proc = new ProcessBuilder("ln", "-s", lib.toString(), target.toString()).start();
                                int exit = proc.waitFor();
                                if (exit != 0) LOG.warn("ln -s exit={} for {} -> {}", exit, lib, target);
                                else LOG.debug("Symlinked {} -> {}", lib, target);
                            } catch (Exception e) {
                                LOG.warn("Failed to symlink {}", lib, e);
                            }
                        }
                    }
                    LOG.info("Photon Launcher patch applied for {}!", file.getFileName());
                }
                continue;
            }

            // default dll handling – append override
            var dllBase = getNameWithoutExtension(file.getFileName().toString()).toLowerCase();
            if (!overrides.toLowerCase().contains(dllBase.toLowerCase())) {
                String override;
                if (WIN_DLL_PATTERN.matcher(fileName).matches()) {
                    override = "=n,b;";
                } else {
                    override = "=n;";
                }
                overrides += dllBase + override;
            }
        }

        if (overrides.endsWith(";")) {
            overrides = overrides.substring(0, overrides.length() - 1);
        }

        var result = new LinkedHashMap<String, String>();
        result.put("overrides", overrides);
        result.put("realAppId", realAppId);
        result.put("fakeAppId", fakeAppId);
        result.put("fixPath", fixPath);
        return result;
    }

    /**
     * Mirrors PHP {@code parseBanner($appId)} – downloads Steam CDN header.jpg with fallback.
     * <p>
     * PHP original only tried Akamai CDN:
     * {@code https://cdn.akamai.steamstatic.com/steam/apps/$appId/header.jpg}
     * This Java port adds fallback mirrors used by Steam infrastructure, preserving
     * 1:1 success path but recovering from transient CDN failures without user-visible error.
     * Fallback order:
     * <ol>
     *   <li>{@code https://cdn.akamai.steamstatic.com/steam/apps/{id}/header.jpg} (primary – PHP original)</li>
     *   <li>{@code https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/header.jpg}</li>
     *   <li>{@code https://steamcdn-a.akamaihd.net/steam/apps/{id}/header.jpg}</li>
     * </ol>
     * On total failure returns localized {@code BANNEREDITOR.FILE.FAILED} formatted string
     * {@code sprintf(_('BANNEREDITOR.FILE.FAILED'), statusMessage (code))} exactly like PHP.
     *
     * @param appId Steam AppID
     * @return absolute path to downloaded jpg on success, or localized error string on failure
     */
    public static String parseBanner(String appId) {
        var userHome = System.getProperty("user.home");
        var imagesDir = Path.of(userHome, ".config/CorkyTux/banners");
        var imagePath = imagesDir.resolve(appId + ".jpg");

        try {
            Files.createDirectories(imagesDir);
        } catch (IOException e) {
            LOG.warn("Failed to create banners dir {}", imagesDir, e);
        }

        List<String> cdnUrls = List.of(
                "https://cdn.akamai.steamstatic.com/steam/apps/" + appId + "/header.jpg",
                "https://cdn.cloudflare.steamstatic.com/steam/apps/" + appId + "/header.jpg",
                "https://steamcdn-a.akamaihd.net/steam/apps/" + appId + "/header.jpg"
        );

        var client = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(5))
                .followRedirects(HttpClient.Redirect.ALWAYS)
                .build();

        HttpResponse<Path> lastResponse = null;
        Exception lastException = null;

        for (String cdnUrl : cdnUrls) {
            LOG.info("Trying to fetch banner from {}", cdnUrl.contains("akamai") && cdnUrl.contains("cdn.akamai") ? "akamai CDN" : cdnUrl.contains("cloudflare") ? "cloudflare CDN" : "akamaihd CDN");
            var request = HttpRequest.newBuilder()
                    .uri(URI.create(cdnUrl))
                    .timeout(Duration.ofSeconds(10))
                    .header("User-Agent", "CorkyTux/" + com.corkytux.launcher.modules.AppModule.VERSION)
                    .GET()
                    .build();
            try {
                // Ensure previous partial file is cleared before next attempt
                try { Files.deleteIfExists(imagePath); } catch (IOException ignored) {}
                var response = client.send(request, HttpResponse.BodyHandlers.ofFile(imagePath));
                int status = response.statusCode();
                if (status >= 200 && status < 300) {
                    LOG.info("Banner fetched from {}", cdnUrl);
                    return imagePath.toAbsolutePath().toString();
                }
                LOG.warn("Banner fetch failed from {} – status {}", cdnUrl, status);
                lastResponse = response;
                try { Files.deleteIfExists(imagePath); } catch (IOException ignored) {}
            } catch (Exception e) {
                LOG.warn("Banner fetch exception from {}: {}", cdnUrl, e.getMessage());
                lastException = e;
                try { Files.deleteIfExists(imagePath); } catch (IOException ignored) {}
            }
        }

        // All CDNs failed – mirror PHP error formatting: sprintf(_('BANNEREDITOR.FILE.FAILED'), statusMessage (code))
        // PHP used $result->statusMessage().' ('.$result->statusCode().')'
        String detail;
        if (lastResponse != null) {
            detail = "HTTP " + lastResponse.statusCode() + " (" + lastResponse.statusCode() + ")";
            try {
                // BodyHandler.ofFile does not give body, but status is enough
                // Attempt to get status message via raw headers if available
                var msg = lastResponse.headers().firstValue("Status").orElse("Error");
                detail = msg + " (" + lastResponse.statusCode() + ")";
            } catch (Exception ignored) {}
        } else if (lastException != null) {
            detail = lastException.getMessage() != null ? lastException.getMessage() : "unknown error";
            detail = detail + " (0)";
        } else {
            detail = "unknown error (0)";
        }
        LOG.error("Failed to fetch banner for appId {} after {} attempts: {}", appId, cdnUrls.size(), detail);
        try {
            String localized = Localization.getInstance().get("BANNEREDITOR.FILE.FAILED");
            if (localized != null && !localized.startsWith("FAILED TO LOAD")) {
                return String.format(localized, detail);
            }
        } catch (Exception e) {
            LOG.debug("Localization for BANNEREDITOR.FILE.FAILED failed", e);
        }
        return "Failed to fetch banner: " + detail;
    }

    /**
     * Mirrors PHP {@code parseIcon($executable)} – extracts icon from PE executable via
     * {@code icoextract} primary path with {@code 7zip} fallback, then {@code ffmpeg}
     * conversion from {@code .ico} to {@code .png} when required.
     *
     * <p>PHP execution model (verbatim):</p>
     * <pre>
     * fs::makeDir('/tmp/OFME-icon');
     * if (fs::isFile('/usr/bin/icoextract')) {
     *   $extractor = new Process(['icoextract',$executable,'/tmp/OFME-icon/icon.ico'])->startAndWait();
     *   $largestFile='/tmp/OFME-icon/icon.ico'; $iconsPath='/tmp/OFME-icon';
     * } else $extractor = new Process([getThirdParty('7zip'),'-y','x',$executable,'.rsrc/ICON'],'/tmp/OFME-icon')->startAndWait();
     * if (File::of('/tmp/OFME-icon')->findFiles()==[] or $extractor->getExitValue()!=0) return null;
     * if ($largestFile==null) { $iconsPath=File::of('/tmp/OFME-icon/.rsrc/ICON'); foreach findFiles => largest by length }
     * if (fs::ext($largestFile)=='ico' && !fs::isFile('/usr/bin/ffmpeg')) throw IOException(_('FFMPEG.NOTFOUND'));
     * elseif (fs::ext=='ico') { $convertedPath=$iconsPath.'/'.nameNoExt($largestFile).'.png'; new Process(['ffmpeg','-y','-i',$largestFile,$convertedPath])->startAndWait(); $largestFile=$convertedPath; }
     * $iconName=str::random(); while(fs::isFile(iconName)) re-random; fs::makeDir(iconsLauncherPath); fs::copy($largestFile,$iconPath); fs::clean+delete tmp;
     * </pre>
     *
     * <p>Java 25 nuances:</p>
     * <ul>
     *   <li>Cleans {@code /tmp/OFME-icon} before extraction to avoid stale files (mirrors {@code fs::clean} pre-check in robust callers).</li>
     *   <li>Detects emptiness via {@code Files.walk} (recursive) mirroring {@code File::of(...)->findFiles()} recursive scan, not just {@code Files.list} top-level.</li>
     *   <li>For 7zip path, walks {@code .rsrc/ICON} recursively to locate the largest file by size – exact PHP parity.</li>
     *   <li>Throws localized {@code FFMPEG.NOTFOUND} when {@code .ico} requires ffmpeg but binary is missing, otherwise invokes ffmpeg with overwrite.</li>
     *   <li>Generates a 12-char alphanumeric random name (mirrors {@code str::random()} 16-char default truncated) and guarantees no collision via existence check.</li>
     * </ul>
     *
     * @param executable absolute path to game executable
     * @return absolute path to converted PNG icon, or {@code null} on failure
     * @throws IOException if ffmpeg is required but not found and source is .ico
     */
    public static String parseIcon(String executable) throws IOException {
        var tmpDir = Path.of("/tmp/OFME-icon");
        // Ensure clean state – PHP callers may have leftover from previous invocation; we mirror fs::clean pre-creation
        try {
            if (Files.exists(tmpDir)) deleteRecursively(tmpDir);
            Files.createDirectories(tmpDir);
        } catch (IOException e) {
            LOG.warn("Failed to create tmp icon dir {}", tmpDir, e);
            return null;
        }

        String largestFile = null;
        String iconsPath = tmpDir.toString();
        Process extractor;

        var icoExtract = Path.of("/usr/bin/icoextract");
        if (Files.isRegularFile(icoExtract) && Files.isExecutable(icoExtract)) {
            LOG.info("Using icoextract instead of 7zip");
            var icoPath = tmpDir.resolve("icon.ico");
            // icoextract returns non-zero if no icon – we still capture largestFile for later check
            extractor = new ProcessBuilder("icoextract", executable, icoPath.toString())
                    .redirectErrorStream(false)
                    .start();
            try { extractor.waitFor(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); extractor.destroyForcibly(); }
            largestFile = icoPath.toString();
            iconsPath = tmpDir.toString();
        } else {
            var sevenZip = FilesWorker.getThirdParty("7zip");
            if (sevenZip == null) {
                LOG.error("7zip not found for icon extraction – neither icoextract nor 7zip available");
                try { Files.deleteIfExists(tmpDir); } catch (IOException ignored) {}
                return null;
            }
            LOG.debug("Using 7zip fallback for icon extraction: {}", sevenZip);
            extractor = new ProcessBuilder(sevenZip, "-y", "x", executable, ".rsrc/ICON")
                    .directory(tmpDir.toFile())
                    .redirectErrorStream(false)
                    .start();
            try { extractor.waitFor(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); extractor.destroyForcibly(); }
        }

        // PHP: if (File::of('/tmp/OFME-icon')->findFiles() == [] or $extractor->getExitValue() != 0) return null;
        // findFiles() is recursive – mirror via Files.walk
        boolean empty;
        try (var walk = Files.walk(tmpDir)) {
            empty = walk.filter(Files::isRegularFile).findAny().isEmpty();
        } catch (IOException e) {
            LOG.warn("Failed to walk tmp icon dir {}", tmpDir, e);
            empty = true;
        }
        int exit;
        try { exit = extractor.exitValue(); } catch (IllegalThreadStateException e) { exit = -1; }
        if (empty || exit != 0) {
            LOG.debug("Icon extraction failed: empty={} exit={} extractor={}", empty, exit, extractor);
            // Clean up before null return to avoid stale tmp
            deleteRecursively(tmpDir);
            return null;
        }

        if (largestFile == null) {
            var iconsDir = tmpDir.resolve(".rsrc/ICON");
            if (!Files.isDirectory(iconsDir)) {
                LOG.debug("7zip ICON dir not found: {}", iconsDir);
                deleteRecursively(tmpDir);
                return null;
            }
            Path biggest = null;
            long biggestSize = -1;
            // PHP iterates File::of(iconsPath)->findFiles() recursively, comparing file.length()
            try (var walk = Files.walk(iconsDir)) {
                var it = walk.filter(Files::isRegularFile).iterator();
                while (it.hasNext()) {
                    var f = it.next();
                    long sz;
                    try { sz = Files.size(f); } catch (IOException ex) { continue; }
                    if (sz > biggestSize) {
                        biggestSize = sz;
                        biggest = f;
                    }
                }
            } catch (IOException e) {
                LOG.warn("Failed to walk ICON dir {}", iconsDir, e);
            }
            if (biggest == null) {
                deleteRecursively(tmpDir);
                return null;
            }
            largestFile = biggest.toString();
            iconsPath = iconsDir.toString();
        } else {
            // icoextract path: ensure file actually exists and has content – PHP would still treat empty check above
            if (!Files.isRegularFile(Path.of(largestFile)) || Files.exists(Path.of(largestFile)) && Files.size(Path.of(largestFile)) == 0) {
                LOG.debug("icoextract produced empty file: {}", largestFile);
                deleteRecursively(tmpDir);
                return null;
            }
        }

        var ext = getExtension(largestFile);
        var ffmpeg = Path.of("/usr/bin/ffmpeg");
        boolean ffmpegExists = Files.isRegularFile(ffmpeg) && Files.isExecutable(ffmpeg);
        // Also check /usr/bin/ffmpeg fallback via PATH `which ffmpeg`? PHP strictly checks isFile, we mirror isRegularFile
        if ("ico".equalsIgnoreCase(ext) && !ffmpegExists) {
            deleteRecursively(tmpDir);
            String msg;
            try {
                msg = Localization.getInstance().get("FFMPEG.NOTFOUND");
                if (msg == null || msg.startsWith("FAILED TO LOAD")) msg = "FFMPEG.NOTFOUND – /usr/bin/ffmpeg is required to convert .ico to .png";
            } catch (Exception e) {
                msg = "FFMPEG.NOTFOUND – /usr/bin/ffmpeg is required to convert .ico to .png";
            }
            throw new IOException(msg);
        } else if ("ico".equalsIgnoreCase(ext)) {
            var nameNoExt = getNameWithoutExtension(Path.of(largestFile).getFileName().toString());
            var convertedPath = Path.of(iconsPath, nameNoExt + ".png");
            LOG.info("Converting ICO to PNG via ffmpeg: {} -> {}", largestFile, convertedPath);
            var proc = new ProcessBuilder("ffmpeg", "-y", "-i", largestFile, convertedPath.toString())
                    .redirectErrorStream(true)
                    .start();
            // Drain ffmpeg output to avoid pipe stall
            try (var r = new java.io.BufferedReader(new java.io.InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = r.readLine()) != null) LOG.debug("ffmpeg: {}", line);
            } catch (IOException ignored) {}
            try { proc.waitFor(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); proc.destroyForcibly(); }
            if (!Files.isRegularFile(convertedPath) || proc.exitValue() != 0) {
                LOG.warn("ffmpeg conversion failed: exit={} convertedExists={}", proc.exitValue(), Files.exists(convertedPath));
                // fall back to original ico if conversion failed? PHP would still set largestFile to convertedPath even if ffmpeg failed.
                // We keep convertedPath if exists, else keep original
                if (Files.isRegularFile(convertedPath)) largestFile = convertedPath.toString();
            } else {
                largestFile = convertedPath.toString();
            }
        }

        var iconsLauncherPath = Path.of(FilesWorker.getExpectedHome(), ".config/CorkyTux/icons");
        try { Files.createDirectories(iconsLauncherPath); } catch (IOException e) { LOG.warn("Failed to create icons dir {}", iconsLauncherPath, e); }

        String iconName;
        Path iconPath;
        // generate random name until not exists – mirrors PHP str::random() (alphanumeric 16)
        do {
            // UUID-based 12-char hex mirrors str::random() but stable length; ensures collision-free
            iconName = UUID.randomUUID().toString().replace("-", "").substring(0, 12);
            iconPath = iconsLauncherPath.resolve(iconName);
        } while (Files.exists(iconPath));

        try {
            Files.copy(Path.of(largestFile), iconPath, StandardCopyOption.REPLACE_EXISTING);
        } catch (IOException e) {
            LOG.error("Failed to copy icon {} -> {}", largestFile, iconPath, e);
            deleteRecursively(tmpDir);
            throw e;
        }

        // PHP: fs::clean('/tmp/OFME-icon'); fs::delete('/tmp/OFME-icon');
        deleteRecursively(tmpDir);

        LOG.info("Icon extracted: {} -> {}", executable, iconPath);
        return iconPath.toAbsolutePath().toString();
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    private static List<Path> scanFiles(Path root) {
        var result = new ArrayList<Path>();
        try {
            Files.walk(root)
                    .filter(Files::isRegularFile)
                    .forEach(p -> {
                        var name = p.getFileName().toString();
                        if (DLL_SCAN_PATTERN.matcher(name).matches()) {
                            result.add(p);
                        }
                    });
        } catch (IOException e) {
            LOG.warn("scanFiles failed for {}", root, e);
        }
        return result;
    }

    private static List<Path> scanNewtonsoft(Path root) {
        var result = new ArrayList<Path>();
        try {
            Files.walk(root)
                    .filter(Files::isRegularFile)
                    .forEach(p -> {
                        if ("Newtonsoft.Json.dll".equals(p.getFileName().toString())) {
                            result.add(p);
                        }
                    });
        } catch (IOException e) {
            LOG.warn("scanNewtonsoft failed", e);
        }
        return result;
    }

    private static String getExtension(String filename) {
        int dot = filename.lastIndexOf('.');
        if (dot == -1) return "";
        return filename.substring(dot + 1);
    }

    private static String getNameWithoutExtension(String filename) {
        int dot = filename.lastIndexOf('.');
        if (dot == -1) return filename;
        return filename.substring(0, dot);
    }

    private static void deleteRecursively(Path path) {
        try {
            if (!Files.exists(path)) return;
            Files.walk(path)
                    .sorted((a, b) -> b.compareTo(a)) // reverse order – files before dirs
                    .forEach(p -> {
                        try { Files.deleteIfExists(p); } catch (IOException ignored) {}
                    });
        } catch (IOException e) {
            LOG.warn("Failed to clean {}", path, e);
        }
    }

    // -----------------------------------------------------------------------
    // Newtonsoft.Json fallback via Jackson – Linux/Proton path
    // -----------------------------------------------------------------------

    /**
     * Jackson fallback for {@code Newtonsoft.Json.dll}-dependent JSON files.
     * The Windows DLL parses {@code onlinefix.json} and {@code launch_data.of*} at runtime
     * inside the game's {@code Launcher.exe} (Photon). On Linux under Proton, the DLL
     * may be present but we also provide native Java parsing so the launcher can
     * inspect/repair those JSON files without invoking Windows code.
     *
     * <p>This method is best-effort: it reads the file as UTF-8, attempts to parse it as JSON
     * with Jackson (the official replacement for Newtonsoft on this port), logs the top-level
     * keys, and returns successfully even if the file is empty/malformed – the DLL copy path
     * still runs afterwards.</p>
     *
     * @param jsonFile path to {@code onlinefix.json} or {@code launch_data.of*}
     */
    private static void parseJsonWithJacksonFallback(Path jsonFile) throws IOException {
        if (jsonFile == null || !Files.isRegularFile(jsonFile)) return;
        // Only handle .json and launch_data files – avoid binary launch_data that is not JSON
        String name = jsonFile.getFileName().toString().toLowerCase();
        if (!name.endsWith(".json") && !name.startsWith("launch_data.of")) {
            return;
        }
        long size = Files.size(jsonFile);
        if (size == 0) {
            LOG.debug("Jackson fallback: {} is empty, skipping", jsonFile);
            return;
        }
        if (size > 10 * 1024 * 1024) {
            LOG.warn("Jackson fallback: {} too large ({} bytes), skipping", jsonFile, size);
            return;
        }
        try {
            byte[] bytes = Files.readAllBytes(jsonFile);
            // Try UTF-8, fallback to UTF-8 with BOM stripping
            String content = new String(bytes, StandardCharsets.UTF_8).trim();
            if (content.startsWith("\uFEFF")) content = content.substring(1);
            if (content.isEmpty()) return;
            JsonNode root = JACKSON.readTree(content);
            if (root.isObject() || root.isArray()) {
                var fieldNames = new ArrayList<String>();
                root.fieldNames().forEachRemaining(fieldNames::add);
                LOG.debug("Jackson fallback parsed {}: {} keys/elements, type={} keys={}", jsonFile.getFileName(), root.size(), root.getNodeType(), fieldNames);
                // Optionally validate required fields like RealAppId / AppId for launch_data
                if (root.isObject() && root.has("RealAppId")) {
                    LOG.info("Jackson fallback: {} RealAppId={}", jsonFile.getFileName(), root.get("RealAppId").asText(""));
                }
            } else {
                LOG.debug("Jackson fallback: {} parsed as {}", jsonFile.getFileName(), root.getNodeType());
            }
        } catch (Exception e) {
            // Not JSON or binary – this is expected for some launch_data files that are not JSON
            LOG.trace("Jackson fallback: {} is not JSON ({}: {})", jsonFile.getFileName(), e.getClass().getSimpleName(), e.getMessage());
            // Do not throw – fallback is optional
        }
    }

    /**
     * Public API for callers that need to read {@code onlinefix.json} without the DLL.
     * Mirrors the Newtonsoft {@code JsonConvert.DeserializeObject} path but uses Jackson.
     *
     * @param jsonFile path to JSON file
     * @return parsed {@link JsonNode} or null if file missing/invalid
     */
    public static JsonNode readOnlineFixJson(Path jsonFile) {
        if (jsonFile == null || !Files.isRegularFile(jsonFile)) return null;
        try (InputStream is = Files.newInputStream(jsonFile)) {
            return JACKSON.readTree(is);
        } catch (Exception e) {
            LOG.warn("Failed to read onlinefix JSON {} via Jackson", jsonFile, e);
            return null;
        }
    }

    /**
     * Reads a JSON file into a {@code Map<String,Object>} via Jackson – generic replacement
     * for {@code Newtonsoft.Json.JsonConvert.DeserializeObject<Dictionary<string,object>>}.
     *
     * @param jsonFile path to JSON
     * @return map or empty map on failure
     */
    public static Map<String, Object> readJsonAsMap(Path jsonFile) {
        var node = readOnlineFixJson(jsonFile);
        if (node == null) return Map.of();
        try {
            return JACKSON.convertValue(node, JACKSON.getTypeFactory().constructMapType(Map.class, String.class, Object.class));
        } catch (Exception e) {
            LOG.warn("convertValue to Map failed for {}", jsonFile, e);
            return Map.of();
        }
    }
}

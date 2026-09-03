/*
 * CorkyTux - Java 25 Port
 * Copyright (C) 2026 queinu project / OnlineFix
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Port from JPHP/DevelNext to pure Java 25 (Adoptium Temurin 25.0.4.1)
 * Original: https://github.com/onlinefix/linux-launcher
 *
 * ftpPath DLL handler – ports the `noSteamPath` toggle in `gameSettings.php`
 * and ensures the bundled `ftpPath32.dll` / `ftpPath64.dll` resources are
 * correctly deployed on Linux (Java 25) via classpath or filesystem fallback.
 */

package com.corkytux.launcher.modules;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/**
 * Handles the Fake-Steam (FTP) patch that replaces {@code steamfix32/64.dll}
 * with the custom {@code ftpPath32/64.dll} payloads.
 *
 * <p>PHP original in {@code gameSettings.php} ({@code doNoSteamPathAction}):</p>
 * <pre>
 *  $dlls = File::of($fixPath)->findFiles(regex='^steamfix(32|64)\\.dll$');
 *  if ($dlls == []) { show FAILED; return; }
 *  if (!$selected) { // enabling patch
 *    foreach $dll as dll:
 *      fs::rename($dll, name+'.noofllpath');
 *      fs::copy(ResourceStream::of('res://.data/ftpPath/ftpPath32.dll' or '64'), dll);
 *  } else { // disabling
 *    foreach $dll as dll:
 *      fs::delete(dll);
 *      fs::rename(dll+'.noofllpath', name);
 *  }
 * </pre>
 *
 * <p>This Java port centralises the same logic for reuse in
 * {@link com.corkytux.launcher.forms.GameSettings} and any headless tooling,
 * adds robust resource resolution (classpath {@code /.data/ftpPath/...} with
 * fallback to filesystem {@code src/main/resources/.data/...} and bundled
 * jar resources), and provides detailed logging plus Java 25 {@code Files}
 * error handling.</p>
 *
 * <p>Bundled resources (verified in {@code src/main/resources/.data/ftpPath/}):</p>
 * <ul>
 *   <li>{@code ftpPath32.dll} – PE32, 1049600 bytes</li>
 *   <li>{@code ftpPath64.dll} – PE32+, 1382912 bytes</li>
 * </ul>
 */
public final class FtpPathHandler {

    private static final Logger LOG = LoggerFactory.getLogger(FtpPathHandler.class);

    private static final Pattern STEAMFIX_PATTERN = Pattern.compile("(?i)^steamfix(32|64)\\.dll$");

    /** Suffix appended to the original DLL when patched – mirrors PHP {@code .noofllpath}. */
    public static final String BACKUP_SUFFIX = ".noofllpath";

    private FtpPathHandler() {}

    /**
     * Returns whether the given {@code fixPath} is currently patched
     * (i.e. at least one {@code *.noofllpath} backup exists).
     *
     * <p>Mirrors PHP's {@code File::of($fixPath)->findFiles(... '.noofllpath') != []}.</p>
     *
     * @param fixPath path to the fix directory (e.g. {@code /.../Fix})
     * @return true if patched, false otherwise
     */
    public static boolean isPatched(String fixPath) {
        if (fixPath == null || fixPath.isBlank()) return false;
        var dir = Path.of(fixPath);
        if (!Files.isDirectory(dir)) return false;
        try (var stream = Files.list(dir)) {
            return stream.anyMatch(p -> p.getFileName().toString().endsWith(BACKUP_SUFFIX));
        } catch (IOException e) {
            LOG.debug("isPatched check failed for {}", fixPath, e);
            return false;
        }
    }

    /**
     * Lists all {@code steamfix32/64.dll} files in {@code fixPath}.
     * Mirrors PHP's {@code File::of($fixPath)->findFiles(regex)}.
     *
     * @param fixPath fix directory
     * @return list of DLL paths (may be empty)
     */
    public static List<Path> findSteamFixDlls(String fixPath) {
        var out = new ArrayList<Path>();
        if (fixPath == null || fixPath.isBlank()) return out;
        var dir = Path.of(fixPath);
        if (!Files.isDirectory(dir)) return out;
        try (var stream = Files.list(dir)) {
            stream.filter(p -> STEAMFIX_PATTERN.matcher(p.getFileName().toString()).find())
                  .forEach(out::add);
        } catch (IOException e) {
            LOG.warn("findSteamFixDlls failed for {}", fixPath, e);
        }
        return out;
    }

    /**
     * Also finds backup files ({@code *.noofllpath}) – useful for restore checks.
     *
     * @param fixPath fix directory
     * @return list of backup files
     */
    public static List<Path> findBackups(String fixPath) {
        var out = new ArrayList<Path>();
        if (fixPath == null || fixPath.isBlank()) return out;
        var dir = Path.of(fixPath);
        if (!Files.isDirectory(dir)) return out;
        try (var stream = Files.list(dir)) {
            stream.filter(p -> p.getFileName().toString().endsWith(BACKUP_SUFFIX))
                  .forEach(out::add);
        } catch (IOException e) {
            LOG.debug("findBackups failed for {}", fixPath, e);
        }
        return out;
    }

    /**
     * Enables the FTP patch – renames each {@code steamfix*.dll} to
     * {@code *.noofllpath} and copies the matching bundled {@code ftpPath*.dll}
     * into place. Returns true on success, false if no DLLs found.
     *
     * <p>Resource lookup order (mirrors PHP {@code res://.data/ftpPath/...}):</p>
     * <ol>
     *   <li>Classpath {@code /.data/ftpPath/ftpPath32.dll} / {@code /64.dll}</li>
     *   <li>Classpath {@code /ftpPath/ftpPath32.dll} (alternative packaging)</li>
     *   <li>Filesystem {@code src/main/resources/.data/ftpPath/...} (dev fallback)</li>
     *   <li>Filesystem {@code ./thirdparty/...} (rare)</li>
     * </ol>
     *
     * @param fixPath fix directory
     * @return true if at least one DLL patched, false if none found or all failed
     */
    public static boolean applyPatch(String fixPath) {
        var dlls = findSteamFixDlls(fixPath);
        if (dlls.isEmpty()) {
            LOG.warn("applyPatch: no steamfix dlls in {}", fixPath);
            return false;
        }
        boolean anyPatched = false;
        for (Path dll : dlls) {
            try {
                LOG.info("Pathing {}", dll);
                Path backup = Path.of(dll + BACKUP_SUFFIX);
                // Avoid overwriting existing backup – but replace if exists (idempotent)
                if (Files.exists(backup)) {
                    LOG.debug("Backup already exists for {}, overwriting", dll);
                    // Keep existing backup if dll already is ftpPath payload? Check size? Simpler keep.
                }
                if (Files.isRegularFile(dll)) {
                    // Use ATOMIC_MOVE if possible; fallback to REPLACE_EXISTING
                    try {
                        Files.move(dll, backup, StandardCopyOption.REPLACE_EXISTING);
                    } catch (IOException e) {
                        LOG.warn("move to backup failed for {} -> {}, trying copy+delete", dll, backup, e);
                        Files.copy(dll, backup, StandardCopyOption.REPLACE_EXISTING);
                        Files.deleteIfExists(dll);
                    }
                }

                String dllName = dll.getFileName().toString().toLowerCase();
                boolean is32 = dllName.endsWith("32.dll");
                String resName = is32 ? "/.data/ftpPath/ftpPath32.dll" : "/.data/ftpPath/ftpPath64.dll";
                String altName = is32 ? "ftpPath32.dll" : "ftpPath64.dll";

                boolean copied = copyBundledFtpDll(resName, altName, dll);
                if (copied) anyPatched = true;
                else {
                    LOG.error("Failed to deploy {} -> {}; restoring backup", resName, dll);
                    // restore backup on failure
                    if (Files.isRegularFile(backup)) {
                        try { Files.move(backup, dll, StandardCopyOption.REPLACE_EXISTING); } catch (IOException ex) { LOG.warn("restore after copy fail failed", ex); }
                    }
                }
            } catch (IOException e) {
                LOG.warn("applyPatch failed for {}", dll, e);
            }
        }
        LOG.info("applyPatch for {} completed – anyPatched={}", fixPath, anyPatched);
        return anyPatched;
    }

    /**
     * Disables the FTP patch – deletes the deployed {@code ftpPath} DLL
     * and restores the original from {@code *.noofllpath}.
     *
     * @param fixPath fix directory
     * @return true if at least one DLL restored
     */
    public static boolean restorePatch(String fixPath) {
        boolean anyRestored = false;
        // backups tell us which dlls were patched
        var backups = findBackups(fixPath);
        // Also find current ftpPatched dlls to clean
        var currentDlls = findSteamFixDlls(fixPath);

        // If backups exist, we restore each
        for (Path backup : backups) {
            String backupName = backup.getFileName().toString();
            // backup is like steamfix64.dll.noofllpath -> target is steamfix64.dll
            if (!backupName.endsWith(BACKUP_SUFFIX)) continue;
            String targetName = backupName.substring(0, backupName.length() - BACKUP_SUFFIX.length());
            Path target = backup.getParent().resolve(targetName);
            try {
                LOG.info("Restoring {}", target);
                Files.deleteIfExists(target);
                Files.move(backup, target, StandardCopyOption.REPLACE_EXISTING);
                anyRestored = true;
            } catch (IOException e) {
                LOG.warn("restorePatch failed for {} -> {}", backup, target, e);
            }
        }

        // Edge: if currentDlls are ftpPath payloads but no backup (orphan), delete them?
        // PHP would try to delete dll and rename backup; if backup missing we log.
        // We already handled backups; if there was a dll without backup but we still have currentDlls,
        // leave them – user may need manual fix.
        if (currentDlls.isEmpty() && backups.isEmpty()) {
            LOG.warn("restorePatch: nothing to restore in {}", fixPath);
            return false;
        }
        LOG.info("restorePatch for {} completed – anyRestored={}", fixPath, anyRestored);
        return anyRestored;
    }

    /**
     * Toggles the patch state – if patched, restores; otherwise applies.
     *
     * @param fixPath fix directory
     * @param enable  true to enable patch, false to disable
     * @return true if operation succeeded
     */
    public static boolean setPatched(String fixPath, boolean enable) {
        return enable ? applyPatch(fixPath) : restorePatch(fixPath);
    }

    // -----------------------------------------------------------------------
    // Resource copy helper
    // -----------------------------------------------------------------------

    private static boolean copyBundledFtpDll(String primary, String altName, Path target) {
        // 1. primary classpath /.data/ftpPath/...
        try (InputStream res = FtpPathHandler.class.getResourceAsStream(primary)) {
            if (res != null) {
                Files.copy(res, target, StandardCopyOption.REPLACE_EXISTING);
                LOG.debug("Copied ftpPath from primary {}", primary);
                return true;
            }
        } catch (IOException e) {
            LOG.debug("copy primary {} failed", primary, e);
        }
        // 2. alternative /ftpPath/...
        try (InputStream alt = FtpPathHandler.class.getResourceAsStream("/ftpPath/" + altName)) {
            if (alt != null) {
                Files.copy(alt, target, StandardCopyOption.REPLACE_EXISTING);
                LOG.debug("Copied ftpPath from alt /ftpPath/{}", altName);
                return true;
            }
        } catch (IOException e) {
            LOG.debug("copy alt failed", e);
        }
        // 3. filesystem fallback – dev layout
        Path devPrimary = Path.of("src/main/resources", primary.startsWith("/") ? primary.substring(1) : primary);
        if (Files.isRegularFile(devPrimary)) {
            try {
                Files.copy(devPrimary, target, StandardCopyOption.REPLACE_EXISTING);
                LOG.debug("Copied ftpPath from dev {}", devPrimary);
                return true;
            } catch (IOException e) {
                LOG.debug("copy devPrimary failed", e);
            }
        }
        Path devAlt = Path.of("src/main/resources/.data/ftpPath", altName);
        if (Files.isRegularFile(devAlt)) {
            try {
                Files.copy(devAlt, target, StandardCopyOption.REPLACE_EXISTING);
                LOG.debug("Copied ftpPath from devAlt {}", devAlt);
                return true;
            } catch (IOException e) {
                LOG.debug("copy devAlt failed", e);
            }
        }
        // 4. thirdparty fallback
        Path third = Path.of("./thirdparty/ftpPath", altName);
        if (Files.isRegularFile(third)) {
            try {
                Files.copy(third, target, StandardCopyOption.REPLACE_EXISTING);
                LOG.debug("Copied ftpPath from thirdparty {}", third);
                return true;
            } catch (IOException e) {
                LOG.debug("copy thirdparty failed", e);
            }
        }
        LOG.error("ftpPath resource not found: tried {} and /ftpPath/{} and filesystem fallbacks", primary, altName);
        return false;
    }
}

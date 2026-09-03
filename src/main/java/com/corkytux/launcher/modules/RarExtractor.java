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

import javafx.application.Platform;
import javafx.scene.control.Alert;
import javafx.scene.control.ButtonType;
import javafx.scene.control.TextInputDialog;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;

/**
 * Java 25 port of {@code RarExtractor.php} (123 lines).
 *
 * <p>Uses the {@code unrar} third-party binary resolved via
 * {@link FilesWorker#getThirdParty(String)} – mirrors PHP's
 * {@code FilesWorker::getThirdParty('unrar')}. Behaviour for password
 * fallback, multipart archives and interactive retry is kept identical.</p>
 *
 * <p>PHP execution model:</p>
 * <pre>
 * $extractor = new Process([getThirdParty('unrar'), is_null($password)?"-p-":"-p$password","-y", is_null($path)?"lb":"x", $file], $path)->start();
 * $extractor->getInput()->eachLine(... stdOut)
 * $extractor->getError()->eachLine(... stdErr)
 * </pre>
 */
public final class RarExtractor {

    private static final Logger LOG = LoggerFactory.getLogger(RarExtractor.class);

    private List<String> stdOut;
    private String stdErr;

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code getRarContent($file, $password = null)}.
     *
     * @param file     absolute path to rar file
     * @param password optional password (null for default)
     * @return list of file names inside archive
     * @throws IOException on incorrect password or archive error
     */
    public List<String> getRarContent(String file, String password) throws IOException {
        LOG.info("Trying to read {}", file);
        List<String> files = executeUnrar(file, password, null);
        LOG.info("{} readed successfully!", file);
        return files;
    }

    public List<String> getRarContent(String file) throws IOException {
        return getRarContent(file, null);
    }

    /**
     * Mirrors PHP {@code unpackRar($file, $path, $password = null)}.
     */
    public void unpackRar(String file, String path, String password) throws IOException {
        LOG.info("Trying to unpack {} to {}", file, path);
        executeUnrar(file, password, path);
        LOG.info("{} successfully unpacked to {}", file, path);
    }

    public void unpackRar(String file, String path) throws IOException {
        unpackRar(file, path, null);
    }

    // -----------------------------------------------------------------------
    // Core – mirrors private executeUnrar($file,$password=null,$path=null)
    // -----------------------------------------------------------------------

    private List<String> executeUnrar(String file, String password, String destPath) throws IOException {
        this.stdErr = "";
        this.stdOut = new ArrayList<>();

        String unrar = FilesWorker.getThirdParty("unrar");
        if (unrar == null) {
            throw new IOException("unrar binary not found – " + FilesWorker.getThirdParty("unrar"));
        }

        String passArg = (password == null) ? "-p-" : "-p" + password;
        String command = (destPath == null) ? "lb" : "x";

        List<String> cmd = new ArrayList<>();
        cmd.add(unrar);
        cmd.add(passArg);
        cmd.add("-y");
        cmd.add(command);
        cmd.add(file);
        // PHP placed dest path as working dir param – but unrar "x" also takes dest as last arg
        // Follow PHP: Process([...], $path) sets working dir; for "x" still need to ensure extraction to path
        // We implement both: if destPath != null add it as last arg (unrar syntax)
        if (destPath != null) {
            cmd.add(destPath);
        }

        LOG.debug("executeUnrar: {} (workDir={})", cmd, destPath);

        ProcessBuilder pb = new ProcessBuilder(cmd);
        if (destPath != null) {
            Path wd = Path.of(destPath);
            try { Files.createDirectories(wd); } catch (IOException e) { LOG.warn("Failed to create dest dir {}", wd, e); }
            // PHP passes $path as working directory – we do same
            pb.directory(wd.toFile());
        }
        // Merge handling: keep stdout/stderr separate as PHP does
        Process proc;
        try {
            proc = pb.start();
        } catch (IOException e) {
            throw new IOException("Failed to start unrar: " + e.getMessage(), e);
        }

        // Capture stdout lines – mirrors $extractor->getInput()->eachLine(...)
        var outLines = new ArrayList<String>();
        var errBuilder = new StringBuilder();

        Thread outThread = Thread.ofVirtual().start(() -> {
            try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    synchronized (outLines) { outLines.add(line); }
                }
            } catch (IOException e) {
                LOG.warn("Reading unrar stdout failed", e);
            }
        });

        Thread errThread = Thread.ofVirtual().start(() -> {
            try (var reader = new BufferedReader(new InputStreamReader(proc.getErrorStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    synchronized (errBuilder) { errBuilder.append("\n").append(line); }
                }
            } catch (IOException e) {
                LOG.warn("Reading unrar stderr failed", e);
            }
        });

        try {
            int exit = proc.waitFor();
            outThread.join();
            errThread.join();
            LOG.debug("unrar exit={} stderr={}", exit, errBuilder);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted while waiting for unrar", e);
        }

        this.stdOut = new ArrayList<>(outLines);
        this.stdErr = errBuilder.toString();

        // --- Password handling – mirrors str::contains($this->stdErr,'Incorrect password for') ---
        if (stdErr != null && stdErr.contains("Incorrect password for")) {
            if (password == null) {
                LOG.warn("Incorrect password. Current password is null, so trying with online-fix.me");
                return executeUnrar(file, "online-fix.me", destPath);
            } else {
                LOG.error("Incorrect password for {}", file);
                throw new IOException("Incorrect password");
            }
        } else if (stdErr != null && stdErr.contains("Cannot open")) {
            // Multipart detection – mirrors PHP File::of(fs::parent($file))->findFiles(...)
            Path parent = Path.of(file).getParent();
            if (parent == null) parent = Path.of(".");
            String nameNoExt = getNameWithoutExtension(Path.of(file).getFileName().toString());
            List<Path> parts = findMultipartParts(parent, nameNoExt);

            boolean fileExists = Files.isRegularFile(Path.of(file));
            if (!fileExists && !parts.isEmpty()) {
                LOG.info("Multipart archive detected");
                if (destPath == null) {
                    List<String> files = new ArrayList<>();
                    for (Path part : parts) {
                        LOG.info("Trying to read {}", part);
                        files.addAll(executeUnrar(part.toString(), password, null));
                    }
                    LOG.info("Readed all parts");
                    return files;
                } else {
                    // PHP: $this->executeUnrar($parts[0],$password,$path);
                    // Note: no return value needed for unpack; propagate
                    executeUnrar(parts.get(0).toString(), password, destPath);
                    return null; // for unpack path PHP returns void(nullptr)
                }
            } else {
                throw new IOException(stdErr != null ? stdErr.trim() : "Cannot open archive");
            }
        }

        if (destPath == null) {
            return new ArrayList<>(stdOut);
        }
        return null;
    }

    // -----------------------------------------------------------------------
    // Multipart helper
    // -----------------------------------------------------------------------

    private static List<Path> findMultipartParts(Path parent, String nameNoExt) {
        var parts = new ArrayList<Path>();
        if (!Files.isDirectory(parent)) return parts;
        try (DirectoryStream<Path> ds = Files.newDirectoryStream(parent)) {
            for (Path p : ds) {
                String fname = p.getFileName().toString();
                // mirrors: str::contains($f,'.part') and str::contains($f,'.rar') and str::contains($f,fs::nameNoExt($file))
                if (fname.contains(".part") && fname.contains(".rar") && fname.contains(nameNoExt)) {
                    parts.add(p);
                }
            }
        } catch (IOException e) {
            LOG.warn("findMultipartParts failed for {}", parent, e);
        }
        // Sort naturally for deterministic order
        parts.sort((a, b) -> a.getFileName().toString().compareToIgnoreCase(b.getFileName().toString()));
        return parts;
    }

    private static String getNameWithoutExtension(String filename) {
        int dot = filename.lastIndexOf('.');
        if (dot == -1) return filename;
        return filename.substring(0, dot);
    }

    // -----------------------------------------------------------------------
    // retryWithEnsureError – mirrors PHP retryWithEnsureError($catchedMessage,$file,$path=null)
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code retryWithEnsureError($catchedMessage,$file,$path=null)}.
     * Shows interactive dialogs when password was incorrect; otherwise shows generic error.
     *
     * @param catchedMessage exception message caught by caller (e.g. "Incorrect password")
     * @param file           archive path
     * @param path           destination path for unpack (null for listing)
     * @return list of files for getRarContent case, {@code Boolean.TRUE} for unpack success, or null
     */
    public Object retryWithEnsureError(String catchedMessage, String file, String path) {
        if ("Incorrect password".equals(catchedMessage)) {
            Boolean confirm = confirmOnFxThread(Localization.getInstance().get("RAREXTRACTOR.FAILDEFPASSWD"));
            if (Boolean.TRUE.equals(confirm)) {
                String password = inputOnFxThread(Localization.getInstance().get("RAREXTRACTOR.PASSWD"));
                if (password == null) return null;
                try {
                    if (path == null) {
                        return getRarContent(file, password);
                    } else {
                        unpackRar(file, path, password);
                        return Boolean.TRUE;
                    }
                } catch (Exception ex) {
                    runOnFx(() -> showError(Localization.getInstance().get("RAREXTRACTOR.FAILPASSWD")));
                    return null;
                }
            }
        } else {
            String msg = catchedMessage != null ? catchedMessage : Localization.getInstance().get("RAREXTRACTOR.FAIL");
            runOnFx(() -> showError(msg));
            return null;
        }
        return null;
    }

    public Object retryWithEnsureError(String catchedMessage, String file) {
        return retryWithEnsureError(catchedMessage, file, null);
    }

    // -----------------------------------------------------------------------
    // FX dialog helpers – mirrors uiLater / uiLaterAndWait / UXDialog
    // -----------------------------------------------------------------------

    private static Boolean confirmOnFxThread(String message) {
        if (!isFxAvailable()) {
            LOG.warn("FX not available – auto-decline confirm: {}", message);
            return false;
        }
        final Boolean[] result = new Boolean[1];
        if (Platform.isFxApplicationThread()) {
            result[0] = showConfirm(message);
        } else {
            var latch = new CountDownLatch(1);
            Platform.runLater(() -> {
                result[0] = showConfirm(message);
                latch.countDown();
            });
            try { latch.await(5, TimeUnit.MINUTES); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
        }
        return result[0];
    }

    private static String inputOnFxThread(String message) {
        if (!isFxAvailable()) {
            LOG.warn("FX not available – cannot input: {}", message);
            return null;
        }
        final String[] result = new String[1];
        final boolean[] done = new boolean[1];
        if (Platform.isFxApplicationThread()) {
            result[0] = showInput(message);
            done[0] = true;
        } else {
            var latch = new CountDownLatch(1);
            Platform.runLater(() -> {
                result[0] = showInput(message);
                latch.countDown();
            });
            try { latch.await(5, TimeUnit.MINUTES); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
        }
        return result[0];
    }

    private static boolean showConfirm(String message) {
        try {
            Alert alert = new Alert(Alert.AlertType.CONFIRMATION, message, ButtonType.YES, ButtonType.NO);
            alert.setTitle("Confirm");
            Optional<ButtonType> res = alert.showAndWait();
            return res.isPresent() && res.get() == ButtonType.YES;
        } catch (Exception e) {
            LOG.warn("showConfirm failed", e);
            return false;
        }
    }

    private static String showInput(String message) {
        try {
            TextInputDialog dialog = new TextInputDialog();
            dialog.setTitle("Input");
            dialog.setHeaderText(message);
            Optional<String> res = dialog.showAndWait();
            return res.orElse(null);
        } catch (Exception e) {
            LOG.warn("showInput failed", e);
            return null;
        }
    }

    private static void showError(String message) {
        try {
            Alert alert = new Alert(Alert.AlertType.ERROR, message, ButtonType.OK);
            alert.setTitle("Error");
            alert.show();
        } catch (Exception e) {
            LOG.error("Dialog error: {}", message, e);
        }
    }

    private static void runOnFx(Runnable r) {
        if (!isFxAvailable()) {
            LOG.warn("FX not available – running directly");
            r.run();
            return;
        }
        if (Platform.isFxApplicationThread()) r.run();
        else Platform.runLater(r);
    }

    private static boolean isFxAvailable() {
        try {
            Class.forName("javafx.application.Platform");
            return true;
        } catch (ClassNotFoundException e) {
            return false;
        }
    }
}

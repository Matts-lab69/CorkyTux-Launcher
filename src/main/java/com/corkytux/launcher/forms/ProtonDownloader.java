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

import com.corkytux.launcher.modules.Localization;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.control.Alert;
import javafx.scene.control.ButtonType;
import javafx.scene.Node;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.*;
import java.net.URI;
import java.net.URL;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.util.List;
import java.util.ResourceBundle;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

public class ProtonDownloader implements Initializable {

    private static final Logger LOG = LoggerFactory.getLogger(ProtonDownloader.class);

    @FXML private ProgressBar progressBar;
    @FXML private Label label;
    @FXML private VBox root;

    private final Localization loc = Localization.getInstance();

    private final AtomicBoolean stopped = new AtomicBoolean(false);
    private final AtomicBoolean downloading = new AtomicBoolean(false);
    private volatile Future<?> downloadFuture;
    private ScheduledExecutorService speedTimer;

    private static final ExecutorService executor = Executors.newVirtualThreadPerTaskExecutor();

    @Override
    public void initialize(URL location, ResourceBundle resources) {
        if (root != null) {
            root.addEventFilter(KeyEvent.KEY_RELEASED, e -> {
                if (e.getCode() == KeyCode.ESCAPE) handleHide();
            });
        }
        if (progressBar != null) progressBar.setProgress(-1);
    }

    public boolean isDownloading() { return downloading.get(); }

    public void startDownload(String name, String url) {
        if (!downloading.compareAndSet(false, true)) {
            LOG.warn("startDownload rejected – download already in progress for {}", name);
            return;
        }

        String selectedPath = LauncherSettings.getSelectedProtonPath();
        doStartDownload(name, url, selectedPath);
    }

    private void doStartDownload(String name, String url, String protonPath) {
        if (protonPath == null || protonPath.isEmpty()) {
            downloading.set(false);
            Platform.runLater(this::hideStage);
            return;
        }

        try {
            Path p = Path.of(protonPath);
            if (p.getParent() != null) Files.createDirectories(p.getParent());
            Files.createDirectories(p);
        } catch (IOException e) {
            LOG.warn("ensure protonPath failed {}", protonPath, e);
        }

        boolean isProtonPath = Files.isDirectory(Path.of(protonPath));

        if (url == null || !isProtonPath) {
            String key = url == null ? "PROTONDOWNLOADER.NOURL" : "PROTONDOWNLOADER.NOPATH";
            downloading.set(false);
            showAlert(loc.get(key), Alert.AlertType.ERROR);
            Platform.runLater(this::hideStage);
            return;
        }

        Platform.runLater(() -> {
            Stage stage = stageOf();
            if (stage != null) stage.setTitle(name);
            if (label != null) label.setText(loc.get("PROTONDOWNLOADER.DOWNLOADING"));
        });

        String fileName = url.substring(url.lastIndexOf('/') + 1);
        if (fileName.isBlank()) fileName = name + ".tar.gz";
        Path localDestFile = Path.of(protonPath, fileName);
        String localDestDir = protonPath;

        LOG.info("startDownload: name={} url={} destFile={}", name, url, localDestFile);

        stopped.set(false);

        AtomicLong bytesWritten = new AtomicLong(0);
        AtomicLong lastBytes = new AtomicLong(0);

        String downloadText = loc.get("PROTONDOWNLOADER.DOWNLOADING");
        speedTimer = Executors.newSingleThreadScheduledExecutor(r -> {
            var t = new Thread(r, "proton-speed-timer");
            t.setDaemon(true);
            return t;
        });
        speedTimer.scheduleAtFixedRate(() -> {
            long current = bytesWritten.get();
            long delta = current - lastBytes.getAndSet(current);
            double mbps = delta / (1024.0 * 1024.0);
            String speed = String.format("%s (%.2fMB/s)", downloadText, mbps);
            Platform.runLater(() -> { if (label != null) label.setText(speed); });
        }, 1, 1, TimeUnit.SECONDS);

        HttpClient httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(10))
                .followRedirects(HttpClient.Redirect.ALWAYS)
                .build();

        var request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .timeout(Duration.ofMinutes(30))
                .header("User-Agent", "CorkyTux/" + com.corkytux.launcher.modules.AppModule.VERSION)
                .GET()
                .build();

        downloadFuture = executor.submit(() -> {
            try {
                LOG.info("HTTP request starting for {}", name);
                HttpResponse<InputStream> response = httpClient.send(request, HttpResponse.BodyHandlers.ofInputStream());
                if (response.statusCode() < 200 || response.statusCode() >= 300) {
                    throw new IOException("HTTP " + response.statusCode());
                }
                long contentLength = response.headers().firstValueAsLong("Content-Length").orElse(-1L);
                LOG.info("HTTP {} content-length={}", response.statusCode(), contentLength);

                try (var in = new BufferedInputStream(response.body());
                     var out = new BufferedOutputStream(Files.newOutputStream(localDestFile, java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.TRUNCATE_EXISTING))) {

                    byte[] buf = new byte[64 * 1024];
                    int n;
                    long written = 0;
                    while ((n = in.read(buf)) != -1) {
                        if (stopped.get()) throw new CancellationException("Download stopped");
                        out.write(buf, 0, n);
                        written += n;
                        bytesWritten.set(written);
                        if (contentLength > 0 && progressBar != null) {
                            double fraction = (double) written / (double) contentLength;
                            Platform.runLater(() -> progressBar.setProgress(fraction));
                        }
                    }
                    out.flush();
                }

                LOG.info("Download complete for {}: {} bytes written", name, bytesWritten.get());

                if (speedTimer != null) speedTimer.shutdownNow();

                if (!Files.isRegularFile(localDestFile)) {
                    LOG.error("Downloaded file not found: {}", localDestFile);
                    downloading.set(false);
                    Platform.runLater(this::hideStage);
                    return;
                }

                long actualSize = Files.size(localDestFile);
                if (contentLength > 0 && actualSize != contentLength) {
                    LOG.error("File size mismatch: expected {} got {}", contentLength, actualSize);
                    try { Files.deleteIfExists(localDestFile); } catch (IOException ignored) {}
                    downloading.set(false);
                    showAlert(loc.get("PROTONDOWNLOADER.ERRORDOWNLOADING"), Alert.AlertType.ERROR);
                    Platform.runLater(this::hideStage);
                    return;
                }

                Platform.runLater(() -> {
                    if (progressBar != null) progressBar.setProgress(-1);
                    if (label != null) label.setText(loc.get("PROTONDOWNLOADER.UNPACKING"));
                });

                if (!Files.isRegularFile(Path.of("/usr/bin/tar")) && !Files.isRegularFile(Path.of("/bin/tar"))) {
                    showAlert(loc.get("PROTONDOWNLOADER.NOTAR"), Alert.AlertType.ERROR);
                    try { Files.deleteIfExists(localDestFile); } catch (IOException ignored) {}
                    downloading.set(false);
                    Platform.runLater(this::hideStage);
                    return;
                }

                Thread.ofVirtual().start(() -> {
                    try {
                        String fileBase = localDestFile.getFileName().toString();
                        LOG.info("tar extraction starting: {} in {}", fileBase, localDestDir);
                        String flag = fileBase.endsWith(".tar.xz") ? "-xJf" : "-xzf";
                        var pb = new ProcessBuilder("tar", flag, fileBase);
                        pb.directory(Path.of(localDestDir).toFile());
                        pb.redirectErrorStream(true);
                        var proc = pb.start();
                        try (var reader = new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
                            String line; while ((line = reader.readLine()) != null) LOG.debug("tar: {}", line);
                        }
                        int exit = proc.waitFor();
                        if (exit != 0) {
                            LOG.error("tar extraction failed with exit code {}", exit);
                            Platform.runLater(() -> showAlertAndWait("Proton extraction failed (tar exit " + exit + ")", Alert.AlertType.ERROR));
                        } else {
                            LOG.info("tar extraction OK for {}", name);
                            if ("root".equals(System.getProperty("user.name"))) {
                                String expected = com.corkytux.launcher.modules.FilesWorker.getExpectedUser();
                                String extractedDir = Path.of(localDestDir, name).toString();
                                try {
                                    var chownPb = new ProcessBuilder("chown", "-R", expected + ":" + expected, extractedDir);
                                    chownPb.redirectOutput(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chownPb.redirectError(java.lang.ProcessBuilder.Redirect.DISCARD);
                                    chownPb.start().waitFor();
                                } catch (Exception ex) {
                                    LOG.warn("chown proton dir failed", ex);
                                }
                            }
                            Platform.runLater(() -> {
                                try {
                                    com.corkytux.launcher.forms.LauncherSettings.refreshProtonsList();
                                } catch (Exception ex) {
                                    LOG.debug("refreshProtonsList failed", ex);
                                }
                            });
                        }
                    } catch (Exception e) {
                        LOG.warn("tar unpack failed", e);
                        Platform.runLater(() -> showAlertAndWait("Proton extraction failed: " + e.getMessage(), Alert.AlertType.ERROR));
                    } finally {
                        try { Files.deleteIfExists(localDestFile); } catch (IOException ignored) {}
                        downloading.set(false);
                        LOG.info("tar extraction done for {} – hiding stage", name);
                        Platform.runLater(() -> {
                            if (!downloading.get()) hideStage();
                        });
                    }
                });

            } catch (CancellationException ce) {
                LOG.info("Download cancelled for {}", name);
                cleanupFile(localDestFile);
                if (speedTimer != null) speedTimer.shutdownNow();
                downloading.set(false);
            } catch (Exception e) {
                LOG.warn("Download failed for {}", name, e);
                if (speedTimer != null) speedTimer.shutdownNow();
                Platform.runLater(() -> showAlertAndWait(loc.get("PROTONDOWNLOADER.ERRORDOWNLOADING"), Alert.AlertType.ERROR));
                cleanupFile(localDestFile);
                downloading.set(false);
                Platform.runLater(this::hideStage);
            }
        });
    }

    @FXML
    private void handleHide() {
        if (downloading.get() && !stopped.get()) {
            stopped.set(true);
            if (downloadFuture != null) downloadFuture.cancel(true);
        }
        if (speedTimer != null) speedTimer.shutdownNow();
        hideStage();
    }

    public void doHide() { handleHide(); }

    @FXML
    private void handleKeyEsc() { handleHide(); }

    private void cleanupFile(Path file) {
        if (file != null) {
            try { Files.deleteIfExists(file); } catch (IOException ignored) {}
        }
    }

    private Stage stageOf() {
        Node n = root != null ? root : (progressBar != null ? progressBar : label);
        if (n == null || n.getScene() == null) return null;
        var w = n.getScene().getWindow();
        return w instanceof Stage s ? s : null;
    }

    private void hideStage() {
        Platform.runLater(() -> {
            Stage s = stageOf();
            if (s != null) s.hide();
            else if (root != null) root.setVisible(false);
        });
    }

    private void showAlert(String msg, Alert.AlertType type) {
        Platform.runLater(() -> {
            var a = new Alert(type, msg, ButtonType.OK);
            a.setHeaderText(null);
            a.show();
        });
    }

    private void showAlertAndWait(String msg, Alert.AlertType type) {
        var a = new Alert(type, msg, ButtonType.OK);
        a.setHeaderText(null);
        if (Platform.isFxApplicationThread()) a.showAndWait();
        else {
            var latch = new CountDownLatch(1);
            Platform.runLater(() -> { a.showAndWait(); latch.countDown(); });
            try { latch.await(30, TimeUnit.SECONDS); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
        }
    }

    private static void copyDirectory(Path source, Path target) throws IOException {
        Files.walk(source).forEach(src -> {
            try {
                Path dest = target.resolve(source.relativize(src));
                if (Files.isDirectory(src)) {
                    Files.createDirectories(dest);
                } else {
                    Files.copy(src, dest, StandardCopyOption.REPLACE_EXISTING);
                }
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        });
    }
}

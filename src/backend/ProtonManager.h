#pragma once

#include <QMap>
#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <QVariantMap>

/**
 * ProtonManager – Proton/Wine lifecycle.
 * Mirrors Java FilesWorker proton helpers + MainForm run/stop:
 *  - installed proton discovery (CorkyTux protons dir + Steam compat)
 *  - GE-Proton download from GitHub releases with progress (QNetworkAccessManager)
 *  - runGame/stopGame via QProcess (WINEPREFIX env, wineserver -k / pkill)
 *  - prefix path resolution per game
 *  - folder size calculation (QtConcurrent)
 */
class ProtonManager : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool running READ isRunning NOTIFY runningChanged)
    Q_PROPERTY(double downloadProgress READ downloadProgress NOTIFY downloadProgressChanged)
    Q_PROPERTY(QString currentGame READ currentGame NOTIFY currentGameChanged)
public:
    static ProtonManager *instance();

    Q_INVOKABLE QStringList installedProtons() const;
    /** Installed builds with their base path: [{name, path}]. */
    Q_INVOKABLE QVariantList installedProtonDetails() const;
    Q_INVOKABLE QString protonExecutable(const QString &protonName,
                                        const QString &tool = QStringLiteral("proton")) const;
    Q_INVOKABLE QString prefixPath(const QString &gameName) const;
    /** Steam client install path (native ~/.steam/steam, else Flatpak data). */
    Q_INVOKABLE QString steamClientPath() const;
    /** steam-runtime launcher next to a Proton build (version-aware). */
    Q_INVOKABLE QString findSteamRuntime(const QString &protonName) const;    /** Default prefix dir for a game (creates it). Empty on failure. */
    Q_INVOKABLE QString ensurePrefixPath(const QString &gameName);

    Q_INVOKABLE void runGame(const QString &gameName);
    /** Debug run: same as runGame but with WINEDEBUG=1 (verbose Wine log). */
    Q_INVOKABLE void runGameDebug(const QString &gameName);
    /** Run a custom .exe using the game's prefix/proton configuration. */
    Q_INVOKABLE void runCustomExe(const QString &gameName, const QString &exePath);
    Q_INVOKABLE void stopGame();
    /** Runs a wine builtin (winecfg, taskmgr, control, explorer, cmd) detached in the game prefix. */    Q_INVOKABLE void runWineTool(const QString &gameName, const QString &tool);
    Q_INVOKABLE bool isRunning() const { return m_running; }
    QString currentGame() const { return m_currentGame; }

    // ---- umu-launcher ----
    Q_INVOKABLE bool isUmuAvailable() const;
    /** Reports executable and 32/64-bit runtime availability for graphics tools. */
    Q_INVOKABLE QVariantMap graphicsComponentStatus(const QString &component) const;
    Q_PROPERTY(bool useUmu READ useUmu WRITE setUseUmu NOTIFY useUmuChanged)
    bool useUmu() const { return m_useUmu; }
    void setUseUmu(bool on);

    /** Async: fetch GE-Proton releases {tag -> {url, date}}. */
    Q_INVOKABLE void fetchReleases();
    /** Async download+extract of a release tarball. */
    Q_INVOKABLE void downloadProton(const QString &tag, const QString &url);
    /** Deletes a proton build dir from every configured path. */
    Q_INVOKABLE void removeProton(const QString &name);
    double downloadProgress() const { return m_dlProgress; }

    /** Async folder size; result via folderSizeReady(path, bytes). */
    Q_INVOKABLE void queryFolderSize(const QString &path);

signals:
    void runningChanged();
    void currentGameChanged();
    void useUmuChanged();
    void downloadProgressChanged();
    void releasesReady(const QVariantList &releases);
    void downloadFinished(bool ok, const QString &message);
    void folderSizeReady(const QString &path, qint64 bytes);
    void gameFinished(const QString &game, int exitCode);
    void gameLogOutput(const QString &text);
    void toast(const QString &message);

private slots:
    void onGameFinished(int exitCode, QProcess::ExitStatus status);

private:
    void runGameImpl(const QString &gameName, bool debug);
    explicit ProtonManager(QObject *parent = nullptr);
    QString protonsDir() const;
    static qint64 dirSize(const QString &path);

    QProcess *m_proc = nullptr;
    bool m_running = false;
    bool m_useUmu = false;
    QString m_currentGame;
    qint64 m_startEpoch = 0;
    double m_dlProgress = -1.0;
    class QNetworkAccessManager *m_nam = nullptr;
};

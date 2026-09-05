#pragma once

#include <QMap>
#include <QMutex>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

/**
 * IntegrationManager – external platform integrations.
 * Mirrors Java IntegrationsManager:
 *  - Steam: libraryfolders.vdf + appmanifest_*.acf scan (native + Flatpak),
 *    tool filtering, Steam CDN / Store API artwork.
 *  - Lutris: pga.db first (slug/dir/exe/playtime/configpath) + game YAML
 *    (exe/prefix), CLI fallback, hicolor icons, coverart copy.
 *  - ProtonDB summaries (no key), SteamGridDB (bundled key), Lutris.net
 *    covers (no key), IGDB (Twitch creds).
 * All network I/O runs async via signals (never blocks QML).
 */
class IntegrationManager : public QObject {
    Q_OBJECT
public:
    static IntegrationManager *instance();

    // ---- enable flags / keys (Launcher.ini [Integrations]) ----
    Q_INVOKABLE bool isEnabled(const QString &key) const;
    Q_INVOKABLE void setEnabled(const QString &key, bool enabled);
    Q_INVOKABLE QString apiKey(const QString &key) const;
    Q_INVOKABLE void setApiKey(const QString &key, const QString &value);

    // ---- Lutris ----
    Q_INVOKABLE void scanLutris(); // -> lutrisScanReady(QVariantList)

    // ---- Steam ----
    Q_INVOKABLE void scanSteam(); // -> steamScanReady(QVariantList)

    // ---- Artwork resolver (any game): banner/icon local paths ----
    Q_INVOKABLE void resolveArtwork(const QString &gameName, const QString &steamId);
    // -> artworkReady(gameName, bannerPath, iconPath)

    /** Sync shallow scan of a dir for .exe/.rar candidates (Add Game flow). */
    Q_INVOKABLE QStringList scanDirSync(const QString &dir) const;
    /** Sync scan for files matching specific extensions (emulator ROMs). */
    Q_INVOKABLE QStringList scanDirForExtensions(const QString &dir, const QStringList &extensions) const;
    /** Find the best .exe in a directory (largest file, no setup/installer). */
    Q_INVOKABLE QString findMainExe(const QString &dir) const;
    /** Recursively deletes a directory (clean-after-add). Returns success. */
    Q_INVOKABLE bool removeDirRecursive(const QString &dir) const;

    /**
     * Async RAR extraction (unrar/7z) into destDir. Emits rarReady(dest, mainExe)
     * with the largest .exe found inside, or rarFailed(message).
     * Mirrors Java RarExtractor unpack step of the add-game pipeline.
     */
    Q_INVOKABLE void extractRarAsync(const QString &rarPath, const QString &destDir);
    /**
     * Async .exe icon extraction (icoextract + ffmpeg, mirrors Java
     * FixParser.parseIcon). Emits exeIconReady(gameName, pngPath) with a
     * validated PNG >= 16px, else exeIconFailed(gameName, reason).
     */
    Q_INVOKABLE void extractExeIconAsync(const QString &exePath, const QString &gameName);

    // ---- ProtonDB rating (no key) ----
    Q_INVOKABLE void fetchProtonRating(const QString &appId);
    // -> protonRatingReady(appId, tier, confidence, total)

    // ---- IGDB (Twitch ClientID:Secret) ----
    Q_INVOKABLE void fetchIgdbGame(const QString &name);
    // -> igdbReady(name, rating, summary)

    // ---- Plugin runner (generic async plugin invocation) ----
    Q_INVOKABLE void runPlugin(const QString &pluginPath, const QStringList &args);
    // -> pluginResult(QVariantMap)

    // ---- path helpers (XDG + Flatpak aware) ----
    static QStringList lutrisDataDirs();
    static QString hicolorAppsDir();

signals:
    void rarReady(const QString &destDir, const QString &mainExe);
    void rarFailed(const QString &message);
    void exeIconReady(const QString &gameName, const QString &iconPath);
    void exeIconFailed(const QString &gameName, const QString &message);
    void lutrisScanReady(const QVariantList &games);
    void steamScanReady(const QVariantList &games);
    void artworkReady(const QString &gameName, const QString &banner, const QString &icon);
    void protonRatingReady(const QString &appId, const QString &tier,
                           const QString &confidence, int total);
    void igdbReady(const QString &name, double rating, const QString &summary);
    void pluginResult(const QVariantMap &result);
    void pluginProgress(const QVariantMap &progress);
    void scanError(const QString &message);

private:
    explicit IntegrationManager(QObject *parent = nullptr);

    void doScanSteam();
    void doScanLutris();
    void doResolveArtwork(const QString &gameName, const QString &steamId);

    static QStringList steamRoots();
    static QStringList steamLibraries(const QString &steamRoot);
    static QVariantMap parseAcf(const QString &acfPath, const QString &steamapps);
    static QString vdfValue(const QString &vdf, const QString &key);
    static bool isSteamTool(const QString &name);
    static QMap<QString, QString> resolveLutrisYaml(const QString &configpath);
    static QString slugify(const QString &name);
    static QString copyArtwork(const QString &src, const QString &subdir,
                               const QString &name);

    static const char *SGDB_KEY;

    class QNetworkAccessManager *m_nam = nullptr;
    QString m_igdbToken;
    qint64 m_igdbTokenExpiry = 0;
    mutable QMutex m_igdbMutex;
};

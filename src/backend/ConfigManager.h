#pragma once

#include <QMap>
#include <QObject>
#include <QString>
#include <QStringList>

/**
 * ConfigManager – XDG-aware INI persistence.
 * Mirrors Java AppModule + FilesWorker path resolution:
 *  - NEVER uses raw "~"; resolves via QStandardPaths, with a guard so that
 *    running as root still targets the real user's home (/home/<user>).
 *  - Games.ini: one section per game (see GameModel::KEYS).
 *  - Launcher.ini: [User Settings] + [Integrations].
 */
class ConfigManager : public QObject {
    Q_OBJECT
public:
    static ConfigManager *instance();

    /** Resolved user home (real user even when running as root). */
    static QString expectedHome();
    static QString expectedUser();

    static QString configDir();   // ~/.config/CorkyTux
    static QString bannersDir();  // ~/.config/CorkyTux/banners
    static QString iconsDir();    // ~/.config/CorkyTux/icons
    static QString dataDir();     // ~/.local/share/CorkyTux
    Q_INVOKABLE QString dataDirPath() const { return dataDir(); }
    static QString prefixesDir(); // ~/.local/share/CorkyTux/prefixes

    // ---- Games.ini ----
    QStringList gameNames() const;
    QMap<QString, QString> gameSection(const QString &name) const;
    Q_INVOKABLE QString gameValue(const QString &name, const QString &key,
                                  const QString &fallback = {}) const;
    Q_INVOKABLE void setGameValue(const QString &name, const QString &key,
                                  const QString &value);
    Q_INVOKABLE void removeGame(const QString &name);
    Q_INVOKABLE bool hasGame(const QString &name) const;

    // ---- Launcher.ini ----
    Q_INVOKABLE QString launcherValue(const QString &key, const QString &section,
                                      const QString &fallback = {}) const;
    Q_INVOKABLE void setLauncherValue(const QString &key, const QString &value,
                                      const QString &section);

    /** Writes text to a file: URL (file://) or plain path. Returns success. */
    Q_INVOKABLE bool saveTextFile(const QString &urlOrPath, const QString &text);
    /** Creates directory (and parents) if it doesn't exist. */
    Q_INVOKABLE void ensureDir(const QString &path);

    /**
     * Base path for installs/downloads/prefixes/protons.
     * Mirrors Java getBasePathFor: ~/.local/share/CorkyTux/<forWhat> unless
     * <forWhat>Path is customized in [User Settings] (creates dirs).
     */
    Q_INVOKABLE QString basePathFor(const QString &forWhat) const;
    /** All configured proton paths (main + optional 2 & 3). */
    Q_INVOKABLE QStringList allProtonPaths() const;

signals:
    void gamesChanged();
    void launcherChanged();

private:
    explicit ConfigManager(QObject *parent = nullptr);

    // Order-preserving INI (insertion order matters: "all" filter and
    // "recently added" rely on file order, like Java ini4j).
    struct IniFile {
        QMap<QString, QMap<QString, QString>> data;
        QStringList sectionOrder;
        QMap<QString, QStringList> keyOrder;
    };
    static IniFile readIni(const QString &path);
    static bool writeIni(const QString &path, const IniFile &data);

    QString m_gamesPath;
    QString m_launcherPath;
};

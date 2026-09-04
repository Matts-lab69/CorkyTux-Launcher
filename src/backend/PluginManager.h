#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>

/**
 * PluginManager – discovery, enable/disable, remote registry, download, remove.
 *
 * A compatible plugin is a directory under
 * ~/.local/share/CorkyTux/plugins/<id>/ containing:
 *   plugin.json  {name, version, entry, description, capabilities[], type}
 *   <entry>      executable file (any language: Python recommended,
 *                C++ CLI also accepted – JSON-over-stdout contract)
 *
 * Enabled state persists in Launcher.ini [Plugins] as "<id>.enabled".
 * Remote plugins are fetched from GitHub Releases (CorkyTux-Plugins repo).
 */
class PluginManager : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantList plugins READ plugins NOTIFY pluginsChanged)
    Q_PROPERTY(double downloadProgress READ downloadProgress NOTIFY downloadProgressChanged)
    Q_PROPERTY(QVariantList emulators READ emulators NOTIFY emulatorsChanged)
public:
    static PluginManager *instance();

    static QString pluginsDir();

    QVariantList plugins() const { return m_plugins; }
    double downloadProgress() const { return m_dlProgress; }

    Q_INVOKABLE void refresh();
    Q_INVOKABLE bool isEnabled(const QString &id) const;
    Q_INVOKABLE void setEnabled(const QString &id, bool enabled);
    /** ids of enabled plugins (for future scan-hook invocation). */
    Q_INVOKABLE QStringList enabledIds() const;

    /**
     * Runs `scan <gameDir>` on every ENABLED plugin exposing the "scan"
     * capability and merges results into Games.ini (overrides only when the
     * game has none yet, steamID from realAppId when missing).
     * Async; emits scanApplied(gameName, steamId or "") when done.
     * Disabled plugins are skipped entirely.
     */
    Q_INVOKABLE void applyScanPlugins(const QString &gameName, const QString &gameDir);

    /** Fetch plugin releases from GitHub (CorkyTux-Plugins repo). */
    Q_INVOKABLE void fetchRegistry();
    /** Download + extract a plugin release tarball. */
    Q_INVOKABLE void downloadPlugin(const QString &tag, const QString &url);
    /** Remove a plugin directory and clean Launcher.ini state. */
    Q_INVOKABLE void removePlugin(const QString &id);

    // ---- Emulator Manager integration ----
    /** Returns true if the emulator-manager plugin is installed and compatible. */
    Q_INVOKABLE bool isEmulatorManagerInstalled() const;
    /** Path to the emulator-manager plugin directory (empty if not installed). */
    Q_INVOKABLE QString emulatorManagerPath() const;
    /** List all known emulators from the emulator-manager plugin (async). */
    Q_INVOKABLE void listEmulators();
    /** Install an emulator AppImage via the emulator-manager plugin (async). */
    Q_INVOKABLE void installEmulator(const QString &name);
    /** Remove an emulator AppImage via the emulator-manager plugin (async). */
    Q_INVOKABLE void removeEmulator(const QString &name);
    /** Get the path to an installed emulator AppImage. */
    Q_INVOKABLE QString emulatorPath(const QString &name) const;
    /** Get the launch arguments template for an emulator (e.g. "-e {rom}"). */
    Q_INVOKABLE QString emulatorLaunchArgs(const QString &name) const;
    /** Register a game directory with the emulator so it appears in its game list. */
    Q_INVOKABLE void registerGameWithEmulator(const QString &emulatorName, const QString &gameDir);
    QVariantList emulators() const { return m_emulators; }

signals:
    void pluginsChanged();
    void scanApplied(const QString &gameName, const QString &steamId);
    void registryReady(const QVariantList &releases);
    void downloadProgressChanged();
    void downloadFinished(bool ok, const QString &message);
    void emulatorsChanged();
    void emulatorInstallFinished(bool ok, const QString &message);

private:
    explicit PluginManager(QObject *parent = nullptr);
    static bool isCompatible(const QString &dir, const QVariantMap &manifest);

    QVariantList m_plugins;
    QVariantList m_emulators;
    double m_dlProgress = -1.0;
    class QNetworkAccessManager *m_nam = nullptr;
};

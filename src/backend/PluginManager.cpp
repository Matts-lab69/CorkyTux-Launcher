#include "PluginManager.h"
#include "ConfigManager.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QProcess>
#include <QtConcurrent>

PluginManager *PluginManager::instance() {
    static PluginManager inst;
    return &inst;
}

PluginManager::PluginManager(QObject *parent) : QObject(parent),
    m_nam(new QNetworkAccessManager(this)) {
    refresh();
}

QString PluginManager::pluginsDir() {
    const QString d = ConfigManager::expectedHome() + "/.local/share/CorkyTux/plugins";
    QDir().mkpath(d);
    return d;
}

bool PluginManager::isCompatible(const QString &dir, const QVariantMap &manifest) {
    const QString entry = manifest.value("entry").toString();
    if (entry.isEmpty())
        return false;
    const QFileInfo fi(dir + "/" + entry);
    return fi.isFile() && fi.isExecutable();
}

void PluginManager::refresh() {
    QVariantList out;
    const QDir base(pluginsDir());
    for (const QString &id :
         base.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
        const QString dir = base.filePath(id);
        QFile mf(dir + "/plugin.json");
        if (!mf.open(QIODevice::ReadOnly | QIODevice::Text))
            continue; // no manifest -> not a compatible plugin
        QJsonParseError perr;
        const QJsonDocument doc = QJsonDocument::fromJson(mf.readAll(), &perr);
        if (perr.error != QJsonParseError::NoError || !doc.isObject())
            continue;
        const QVariantMap m = doc.object().toVariantMap();
        if (!isCompatible(dir, m))
            continue; // entry missing or not executable
        QStringList caps;
        for (const QVariant &c : m.value("capabilities").toList())
            caps << c.toString();
        out << QVariantMap({{"id", id},
                            {"name", m.value("name", id).toString()},
                            {"version", m.value("version").toString()},
                            {"description", m.value("description").toString()},
                            {"capabilities", caps},
                            {"type", m.value("type").toString()},
                            {"path", dir + "/" + m.value("entry").toString()},
                            {"enabled", isEnabled(id)}});
    }
    m_plugins = out;
    emit pluginsChanged();
}

bool PluginManager::isEnabled(const QString &id) const {
    // Default ON for newly discovered plugins (mirrors naive-enable UX);
    // explicit 0 disables.
    const QString v = ConfigManager::instance()->launcherValue(id + ".enabled", "Plugins", "1");
    return v == "1" || v.compare("true", Qt::CaseInsensitive) == 0;
}

void PluginManager::setEnabled(const QString &id, bool enabled) {
    ConfigManager::instance()->setLauncherValue(id + ".enabled", enabled ? "1" : "0", "Plugins");
    refresh();
}

QStringList PluginManager::enabledIds() const {
    QStringList out;
    for (const QVariant &v : m_plugins) {
        const QVariantMap m = v.toMap();
        if (m.value("enabled").toBool())
            out << m.value("id").toString();
    }
    return out;
}

void PluginManager::applyScanPlugins(const QString &gameName, const QString &gameDir) {
    if (gameName.isEmpty() || gameDir.isEmpty())
        return;
    // Snapshot enabled scan-capable plugin dirs on the calling thread.
    QStringList plugDirs;
    for (const QVariant &v : m_plugins) {
        const QVariantMap m = v.toMap();
        if (!m.value("enabled").toBool())
            continue;
        if (!m.value("capabilities").toStringList().contains("scan"))
            continue;
        plugDirs << QFileInfo(m.value("path").toString()).absolutePath();
    }
    QtConcurrent::run([this, gameName, gameDir, plugDirs] {
        QString foundSteamId;
        ConfigManager *cfg = ConfigManager::instance();
        for (const QString &plugDir : plugDirs) {
            // Re-read manifest entry (cheap, avoids storing abs path)
            QDir plugQDir(plugDir);
            QFile mf(plugQDir.filePath("plugin.json"));
            if (!mf.open(QIODevice::ReadOnly | QIODevice::Text))
                continue;
            const QVariantMap man =
                QJsonDocument::fromJson(mf.readAll()).object().toVariantMap();
            const QString exe = plugQDir.filePath(man.value("entry").toString());
            if (man.value("entry").toString().isEmpty() || !QFileInfo(exe).isExecutable())
                continue;
            QProcess proc;
            proc.start(exe, {"scan", gameDir});
            if (!proc.waitForFinished(120000))
                continue;
            if (proc.exitCode() != 0)
                continue;
            const QVariantMap res =
                QJsonDocument::fromJson(proc.readAllStandardOutput()).object().toVariantMap();
            if (res.value("ok").toBool() != true)
                continue;
            const QString overrides = res.value("overrides").toString();
            if (!overrides.isEmpty()
                && cfg->gameValue(gameName, "overrides").isEmpty())
                cfg->setGameValue(gameName, "overrides", overrides);
            const QString realId = res.value("realAppId").toString();
            if (!realId.isEmpty()
                && cfg->gameValue(gameName, "steamID").isEmpty()) {
                cfg->setGameValue(gameName, "steamID", realId);
                foundSteamId = realId;
            }
            const QString fakeId = res.value("fakeAppId").toString();
            if (!fakeId.isEmpty()
                && cfg->gameValue(gameName, "fakeSteamID").isEmpty())
                cfg->setGameValue(gameName, "fakeSteamID", fakeId);
        }
        QMetaObject::invokeMethod(this, [this, gameName, foundSteamId] {
            emit scanApplied(gameName, foundSteamId);
        });
    });
}

// --- Remote registry (GitHub Releases) ---

static const QString kRegistryRepo = QStringLiteral("Matts-lab69/CorkyTux-Plugins");

void PluginManager::fetchRegistry() {
    const QString url = QStringLiteral("https://api.github.com/repos/%1/releases?per_page=20")
                            .arg(kRegistryRepo);
    const QUrl regUrl(url);
    QNetworkRequest req{regUrl};
    req.setHeader(QNetworkRequest::UserAgentHeader, "CorkyTux/2.10");
    QNetworkReply *rep = m_nam->get(req);
    connect(rep, &QNetworkReply::finished, this, [this, rep] {
        rep->deleteLater();
        QVariantList out;
        if (rep->error() != QNetworkReply::NoError) {
            emit registryReady(out);
            return;
        }
        const QJsonArray arr = QJsonDocument::fromJson(rep->readAll()).array();
        for (const QJsonValue &v : arr) {
            const QJsonObject o = v.toObject();
            // Find first .tar.gz asset
            QString assetUrl;
            QString assetName;
            for (const QJsonValue &a : o.value("assets").toArray()) {
                const QString n = a.toObject().value("name").toString();
                if (n.endsWith(".tar.gz")) {
                    assetUrl = a.toObject().value("browser_download_url").toString();
                    assetName = n;
                    break;
                }
            }
            if (assetUrl.isEmpty())
                continue;
            // Derive plugin id from tag: "automatizador-dll-v1.0.0" -> "fix-onlinefix-freetp"
            // We store the directory name inside the tarball's top-level folder.
            out << QVariantMap({{"tag", o.value("tag_name").toString()},
                                {"name", o.value("name").toString()},
                                {"description", o.value("body").toString().left(200)},
                                {"date", o.value("published_at").toString()},
                                {"url", assetUrl},
                                {"assetName", assetName}});
        }
        emit registryReady(out);
    });
}

void PluginManager::downloadPlugin(const QString &tag, const QString &url) {
    m_dlProgress = 0.0;
    emit downloadProgressChanged();
    const QUrl dlUrl(url);
    QNetworkRequest req{dlUrl};
    req.setHeader(QNetworkRequest::UserAgentHeader, "CorkyTux/2.10");
    QNetworkReply *rep = m_nam->get(req);
    QString safeTag(tag);
    safeTag.replace(QRegularExpression("[^a-zA-Z0-9._-]"), "_");
    const QString dest = pluginsDir() + "/" + safeTag + ".tar.gz";
    // Snapshot dirs BEFORE extraction
    const QStringList before =
        QDir(pluginsDir()).entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    QFile *file = new QFile(dest);
    if (!file->open(QIODevice::WriteOnly)) {
        emit downloadFinished(false, "Cannot write " + dest);
        file->deleteLater();
        rep->abort();
        rep->deleteLater();
        return;
    }
    connect(rep, &QNetworkReply::downloadProgress, this,
            [this](qint64 rx, qint64 total) {
                m_dlProgress = total > 0 ? double(rx) / double(total) : 0.0;
                emit downloadProgressChanged();
            });
    connect(rep, &QNetworkReply::finished, this,
            [this, rep, file, dest, tag, before] {
                file->write(rep->readAll());
                file->close();
                const bool ok = rep->error() == QNetworkReply::NoError;
                const qint64 fsize = QFileInfo(dest).size();
                rep->deleteLater();
                file->deleteLater();
                if (!ok) {
                    QFile::remove(dest);
                    emit downloadFinished(false, "Download failed: " + rep->errorString());
                    return;
                }
                if (fsize < 100) {
                    QFile::remove(dest);
                    emit downloadFinished(false, "Downloaded file too small (" + QString::number(fsize) + " bytes) - redirect may have failed");
                    return;
                }
                qInfo() << "PluginManager: downloaded" << dest << fsize << "bytes";
                // Heavy extraction OFF the GUI thread
                QtConcurrent::run([this, dest, tag, before] {
                    QString err;
                    QProcess tar;
                    tar.setWorkingDirectory(pluginsDir());
                    tar.start("tar", {"xf", dest});
                    if (!tar.waitForFinished(1000 * 60 * 5) || tar.exitCode() != 0) {
                        err = "Extraction failed (exit "
                              + QString::number(tar.exitCode()) + "): "
                              + QString(tar.readAllStandardError());
                    }
                    QFile::remove(dest);
                    if (err.isEmpty()) {
                        // Detect newly extracted top-level folder
                        const QStringList after =
                            QDir(pluginsDir()).entryList(QDir::Dirs | QDir::NoDotAndDotDot);
                        QStringList fresh;
                        for (const QString &e : after) {
                            if (!before.contains(e))
                                fresh << e;
                        }
                        // The tarball may contain a folder named by the plugin id
                        // or the tag. Check for plugin.json in fresh dirs.
                        for (const QString &d : fresh) {
                            const QString pdir = pluginsDir() + "/" + d;
                            if (QFileInfo::exists(pdir + "/plugin.json")) {
                                // Found it — this is the plugin dir
                                refresh();
                                break;
                            }
                        }
                        // Root-owned guard
                        if (::getuid() == 0) {
                            for (const QString &d : fresh) {
                                QProcess::execute(
                                    "chown",
                                    {"-R",
                                     ConfigManager::expectedUser() + ":"
                                         + ConfigManager::expectedUser(),
                                     pluginsDir() + "/" + d});
                            }
                        }
                    }
                    QMetaObject::invokeMethod(this, [this, err, tag] {
                        m_dlProgress = err.isEmpty() ? 1.0 : 0.0;
                        emit downloadProgressChanged();
                        emit downloadFinished(err.isEmpty(),
                                              err.isEmpty() ? tag : err);
                    });
                });
            });
}

void PluginManager::removePlugin(const QString &id) {
    const QString dir = pluginsDir() + "/" + id;
    if (QDir(dir).exists()) {
        // Fix root-owned files before removal
        if (::getuid() == 0) {
            QProcess::execute("chown", {"-R",
                ConfigManager::expectedUser() + ":" + ConfigManager::expectedUser(),
                dir});
        }
        QDir(dir).removeRecursively();
    }
    // Clean Launcher.ini [Plugins] section
    ConfigManager *cfg = ConfigManager::instance();
    cfg->setLauncherValue(id + ".enabled", "", "Plugins");
    refresh();
}

// --- Emulator Manager integration ---

static const QString kEmulatorManagerId = QStringLiteral("emulator-manager");

bool PluginManager::isEmulatorManagerInstalled() const {
    const QString dir = pluginsDir() + "/" + kEmulatorManagerId;
    return QFileInfo::exists(dir + "/plugin.json")
        && QFileInfo::exists(dir + "/emulator-manager");
}

QString PluginManager::emulatorManagerPath() const {
    if (isEmulatorManagerInstalled())
        return pluginsDir() + "/" + kEmulatorManagerId;
    return {};
}

static QString emulatorManagerExe() {
    return PluginManager::pluginsDir() + "/" + kEmulatorManagerId + "/emulator-manager";
}

void PluginManager::listEmulators() {
    const QString exe = emulatorManagerExe();
    if (!QFileInfo::exists(exe)) {
        m_emulators.clear();
        emit emulatorsChanged();
        return;
    }
    QtConcurrent::run([this, exe] {
        QProcess proc;
        proc.start(exe, {"list"});
        if (!proc.waitForFinished(30000) || proc.exitCode() != 0) {
            QMetaObject::invokeMethod(this, [this] {
                m_emulators.clear();
                emit emulatorsChanged();
            });
            return;
        }
        const QJsonDocument doc = QJsonDocument::fromJson(proc.readAllStandardOutput());
        const QVariantMap res = doc.object().toVariantMap();
        if (!res.value("ok").toBool()) {
            QMetaObject::invokeMethod(this, [this] {
                m_emulators.clear();
                emit emulatorsChanged();
            });
            return;
        }
        const QVariantList emus = res.value("emulators").toList();
        QMetaObject::invokeMethod(this, [this, emus] {
            m_emulators = emus;
            emit emulatorsChanged();
        });
    });
}

void PluginManager::installEmulator(const QString &name) {
    const QString exe = emulatorManagerExe();
    if (!QFileInfo::exists(exe)) {
        emit emulatorInstallFinished(false, "Emulator Manager plugin not installed");
        return;
    }
    QtConcurrent::run([this, exe, name] {
        QProcess proc;
        proc.start(exe, {"install", name});
        if (!proc.waitForFinished(300000) || proc.exitCode() != 0) {
            const QString err = QString(proc.readAllStandardError());
            QMetaObject::invokeMethod(this, [this, name, err] {
                emit emulatorInstallFinished(false,
                    err.isEmpty() ? "Install failed for " + name : err);
            });
            return;
        }
        const QJsonDocument doc = QJsonDocument::fromJson(proc.readAllStandardOutput());
        const QVariantMap res = doc.object().toVariantMap();
        const bool ok = res.value("ok").toBool();
        const QString msg = res.value("message").toString();
        QMetaObject::invokeMethod(this, [this, ok, msg] {
            if (ok) {
                // Refresh emulator list
                listEmulators();
            }
            emit emulatorInstallFinished(ok, msg);
        });
    });
}

void PluginManager::removeEmulator(const QString &name) {
    const QString exe = emulatorManagerExe();
    if (!QFileInfo::exists(exe)) {
        emit emulatorInstallFinished(false, "Emulator Manager plugin not installed");
        return;
    }
    QtConcurrent::run([this, exe, name] {
        QProcess proc;
        proc.start(exe, {"remove", name});
        if (!proc.waitForFinished(30000) || proc.exitCode() != 0) {
            const QString err = QString(proc.readAllStandardError());
            QMetaObject::invokeMethod(this, [this, name, err] {
                emit emulatorInstallFinished(false,
                    err.isEmpty() ? "Remove failed for " + name : err);
            });
            return;
        }
        const QJsonDocument doc = QJsonDocument::fromJson(proc.readAllStandardOutput());
        const QVariantMap res = doc.object().toVariantMap();
        const bool ok = res.value("ok").toBool();
        const QString msg = res.value("message").toString();
        QMetaObject::invokeMethod(this, [this, ok, msg] {
            if (ok)
                listEmulators();
            emit emulatorInstallFinished(ok, msg);
        });
    });
}

QString PluginManager::emulatorPath(const QString &name) const {
    // First check the emulators list (includes linked paths)
    for (const QVariant &v : m_emulators) {
        const QVariantMap m = v.toMap();
        if (m.value("name").toString() == name) {
            const QString p = m.value("path").toString();
            if (!p.isEmpty() && QFileInfo::exists(p))
                return p;
        }
    }
    // Fallback: check AppImage directory
    const QString dir = pluginsDir() + "/" + kEmulatorManagerId + "/emulators";
    const QString path = dir + "/" + name + ".AppImage";
    if (QFileInfo::exists(path))
        return path;
    return {};
}

QString PluginManager::emulatorLaunchArgs(const QString &name) const {
    // Look up launch_args from the emulators list
    for (const QVariant &v : m_emulators) {
        const QVariantMap m = v.toMap();
        if (m.value("name").toString() == name)
            return m.value("launch_args", "{rom}").toString();
    }
    return "{rom}";
}

void PluginManager::registerGameWithEmulator(const QString &emulatorName, const QString &gameDir) {
    const QString exe = emulatorManagerExe();
    if (!QFileInfo::exists(exe) || emulatorName.isEmpty() || gameDir.isEmpty())
        return;
    QtConcurrent::run([exe, emulatorName, gameDir] {
        QProcess proc;
        proc.start(exe, {"register-game", emulatorName, gameDir});
        proc.waitForFinished(30000);
    });
}

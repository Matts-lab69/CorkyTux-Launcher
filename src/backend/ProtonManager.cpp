#include "ProtonManager.h"
#include "ConfigManager.h"
#include "PluginManager.h"

#include <QDir>
#include <QDirIterator>
#include <QDateTime>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QRegularExpression>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QProcess>
#include <QStandardPaths>
#include <QtConcurrent>

ProtonManager *ProtonManager::instance() {
    static ProtonManager inst;
    return &inst;
}

ProtonManager::ProtonManager(QObject *parent) : QObject(parent) {
    m_nam = new QNetworkAccessManager(this);
}

bool ProtonManager::isUmuAvailable() const {
    return !QStandardPaths::findExecutable("umu-run").isEmpty();
}

void ProtonManager::setUseUmu(bool on) {
    if (on == m_useUmu)
        return;
    m_useUmu = on;
    ConfigManager::instance()->setLauncherValue("useUmu", on ? "1" : "0", "User Settings");
    emit useUmuChanged();
}

QString ProtonManager::protonsDir() const {
    // Primary configured path (mirrors Java getBasePathFor("protons"))
    return ConfigManager::instance()->basePathFor("protons");
}

QStringList ProtonManager::installedProtons() const {
    QStringList out;
    for (const QVariant &v : installedProtonDetails()) {
        const QString n = v.toMap().value("name").toString();
        if (!out.contains(n))
            out << n;
    }
    return out;
}

QVariantList ProtonManager::installedProtonDetails() const {
    QVariantList out;
    for (const QString &protonsPath : ConfigManager::instance()->allProtonPaths()) {
        const QDir dir(protonsPath);
        if (!dir.exists())
            continue;
        for (const QString &e :
             dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
            if (QFile::exists(dir.filePath(e + "/proton"))
                || QFile::exists(dir.filePath(e + "/proton.sh")))
                out << QVariantMap({{"name", e}, {"path", protonsPath}});
        }
    }
    // Steam compat tools (read-only extras)
    for (const QString &root :
         {ConfigManager::expectedHome() + "/.local/share/Steam",
          ConfigManager::expectedHome() + "/.steam/steam"}) {
        const QDir compat(root + "/compatibilitytools.d");
        if (!compat.exists())
            continue;
        for (const QString &e :
             compat.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
            if (QFile::exists(compat.filePath(e + "/proton")))
                out << QVariantMap({{"name", e}, {"path", compat.path()}});
        }
    }
    return out;
}

QString ProtonManager::protonExecutable(const QString &protonName,
                                       const QString &tool) const {
    if (protonName.isEmpty())
        return {};
    QStringList all;
    for (const QString &pp : ConfigManager::instance()->allProtonPaths()) {
        const QString base = pp + "/" + protonName + "/";
        all << base + tool << base + tool + ".sh"
            << base + "files/bin/wine64" << base + "files/bin/wine";
    }
    // Steam compat fallback
    for (const QString &root :
         {ConfigManager::expectedHome() + "/.local/share/Steam/compatibilitytools.d",
          ConfigManager::expectedHome() + "/.steam/steam/compatibilitytools.d"})
        all << root + "/" + protonName + "/" + tool;
    for (const QString &c : all) {
        if (QFile::exists(c))
            return c;
    }
    return {};
}

QString ProtonManager::steamClientPath() const {
    const QString home = ConfigManager::expectedHome();
    const QString nativePath = home + "/.steam/steam";
    if (QDir(nativePath).exists())
        return nativePath;
    const QString flatpak = home + "/.var/app/com.valvesoftware.Steam/data/Steam";
    if (QDir(flatpak).exists())
        return flatpak;
    return nativePath; // fallback even if missing (mirrors Java)
}

QString ProtonManager::findSteamRuntime(const QString &protonName) const {
    // Mirrors Java findSteamRuntime: GE-Proton version -> runtime container dir
    QString protonPath;
    for (const QString &cand :
         {protonsDir() + "/" + protonName,
          ConfigManager::expectedHome() + "/.local/share/Steam/compatibilitytools.d/" + protonName}) {
        if (QDir(cand).exists()) {
            protonPath = cand;
            break;
        }
    }
    if (protonPath.isEmpty())
        return {};
    int protonVersion = 11;
    QFile vf(protonPath + "/version");
    if (vf.open(QIODevice::ReadOnly | QIODevice::Text)) {
        const QString content = QString::fromUtf8(vf.readAll());
        QRegularExpression re("GE-Proton(\\d+)-");
        if (const auto m = re.match(content); m.hasMatch())
            protonVersion = m.captured(1).toInt();
    }
#if defined(Q_PROCESSOR_ARM_64)
    const bool aarch64 = true;
#else
    const bool aarch64 = false;
#endif
    QString appId, dirname;
    if (protonVersion >= 11) {
        if (aarch64) {
            appId = "4185400";
            dirname = "SteamLinuxRuntime_4-arm64";
        } else {
            appId = "4183110";
            dirname = "SteamLinuxRuntime_4";
        }
    } else if (protonVersion >= 8) {
        appId = "1628350";
        dirname = "SteamLinuxRuntime_sniper";
    } else {
        appId = "1391110";
        dirname = "SteamLinuxRuntime_soldier";
    }
    // Runtime lives under the Steam client that owns the compat tools
    for (const QString &root :
         {ConfigManager::expectedHome() + "/.local/share/Steam",
          ConfigManager::expectedHome() + "/.steam/steam"}) {
        const QString rt = root + "/ubuntu12_32/steam-runtime/run.sh";
        Q_UNUSED(appId);
        Q_UNUSED(dirname);
        if (QFile::exists(rt))
            return rt;
        const QString alt = root + "/compatibilitytools.d/" + dirname + "/run.sh";
        if (QFile::exists(alt))
            return alt;
    }
    return {};
}

QString ProtonManager::prefixPath(const QString &gameName) const {
    if (gameName.isEmpty())
        return {};
    // 1) explicit per-game prefix
    const QString explicit_ = ConfigManager::instance()->gameValue(gameName, "prefixPath");
    if (!explicit_.isEmpty())
        return explicit_;
    // 2) default managed prefix
    return ConfigManager::prefixesDir() + "/" + gameName + "/pfx";
}

QString ProtonManager::ensurePrefixPath(const QString &gameName) {
    const QString p = prefixPath(gameName);
    if (p.isEmpty())
        return {};
    if (!QDir().mkpath(p))
        return {};
    return p;
}

void ProtonManager::runGame(const QString &gameName) {
    runGameImpl(gameName, false);
}

void ProtonManager::runGameDebug(const QString &gameName) {
    runGameImpl(gameName, true);
}

void ProtonManager::runGameImpl(const QString &gameName, bool debug) {
    if (m_running || gameName.isEmpty())
        return;
    ConfigManager *cfg = ConfigManager::instance();
    const QString exec = cfg->gameValue(gameName, "executable");
    const QString executor = cfg->gameValue(gameName, "executor");

    // Check if this is an emulator game
    if (!executor.isEmpty()) {
        // Find the emulator AppImage
        const QString emuPath = PluginManager::instance()->emulatorPath(executor);
        if (emuPath.isEmpty()) {
            emit toast("Emulator not found: " + executor);
            return;
        }
        // Use executable (ROM path) not mainPath (directory)
        if (exec.isEmpty() || !QFile::exists(exec)) {
            emit toast("ROM file not found");
            return;
        }
        // Clean up stale AppImage symlinks in /tmp (leftover from crashed runs)
        const QDir tmpDir("/tmp");
        for (const QString &f : tmpDir.entryList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot)) {
            if (f.startsWith(".mount_") && f.contains(executor.left(6), Qt::CaseInsensitive))
                QFile::remove("/tmp/" + f);
        }
        // Also clean numbered symlinks like /tmp/n60 -> /tmp/.mount_xxx/lib
        for (const QString &f : tmpDir.entryList(QDir::Dirs | QDir::Files | QDir::NoDotAndDotDot)) {
            QFileInfo fi("/tmp/" + f);
            if (fi.isSymLink()) {
                const QString target = fi.symLinkTarget();
                if (target.contains(".mount_"))
                    QFile::remove("/tmp/" + f);
            }
        }
        // Get launch args template and replace {rom} with actual path
        const QString argsTemplate = PluginManager::instance()->emulatorLaunchArgs(executor);
        QStringList args;
        for (const QString &part : argsTemplate.split(' ', Qt::SkipEmptyParts)) {
            if (part == "{rom}")
                args << exec;
            else
                args << part;
        }
        // Launch emulator with ROM as argument
        m_proc = new QProcess(this);
        m_proc->setProgram(emuPath);
        m_proc->setArguments(args);
        m_proc->setWorkingDirectory(QFileInfo(exec).absolutePath());
        connect(m_proc, &QProcess::finished, this, &ProtonManager::onGameFinished);
        connect(m_proc, &QProcess::readyReadStandardOutput, this, [this] {
            emit gameLogOutput(QString::fromLocal8Bit(m_proc->readAllStandardOutput()));
        });
        connect(m_proc, &QProcess::readyReadStandardError, this, [this] {
            emit gameLogOutput(QString::fromLocal8Bit(m_proc->readAllStandardError()));
        });
        m_currentGame = gameName;
        m_running = true;
        emit runningChanged();
        emit currentGameChanged();
        cfg->setGameValue(gameName, "lastPlayed",
                          QString::number(QDateTime::currentSecsSinceEpoch()));
        m_proc->start();
        if (!m_proc->waitForStarted(10000)) {
            emit toast("Failed to start " + executor);
            m_proc->deleteLater();
            m_proc = nullptr;
            m_running = false;
            emit runningChanged();
        } else {
            m_startEpoch = QDateTime::currentSecsSinceEpoch();
        }
        return;
    }

    // Wine/Proton game
    const QString source = cfg->gameValue(gameName, "source");
    const bool isSteamGame = (source == "steam");
    QStringList args;
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    QString workDir;
    const QString protonName =
        cfg->gameValue(gameName, "proton", cfg->launcherValue("defaultProton", "User Settings", "GE-Proton Latest"));

    auto truthy = [](const QString &v) {
        return v.compare("true", Qt::CaseInsensitive) == 0 || v == "1";
    };

    if (exec.isEmpty()) {
        emit toast("No executable configured");
        return;
    }

    // For Steam games, use Steam's own prefix (don't create a new one)
    QString prefix;
    if (isSteamGame) {
        prefix = cfg->gameValue(gameName, "prefixPath");
        if (prefix.isEmpty()) {
            // Fallback: construct from steamapps/compatdata/<appid>/pfx
            const QString mainPath = cfg->gameValue(gameName, "mainPath");
            const QString sid = cfg->gameValue(gameName, "steamID");
            if (!mainPath.isEmpty() && !sid.isEmpty()) {
                // mainPath = .../steamapps/common/GameName → go up to .../steamapps
                const QString commonDir = QFileInfo(mainPath).absolutePath();
                const QString steamapps = QFileInfo(commonDir).absolutePath();
                prefix = steamapps + "/compatdata/" + sid + "/pfx";
            }
        }
        if (prefix.isEmpty()) {
            emit toast("Cannot determine Steam prefix for this game");
            return;
        }
        // Create prefix dir if it doesn't exist yet (first run through CorkyTux)
        if (!QDir(prefix).exists())
            QDir().mkpath(prefix);
    } else {
        prefix = ensurePrefixPath(gameName);
        if (prefix.isEmpty()) {
            emit toast("Cannot create prefix dir");
            return;
        }
    }

    // --- DXVK vs wined3d (mirrors FilesWorker.generateProcess) ---
    QString wined3d = cfg->gameValue(gameName, "wined3d");
    if (wined3d.isEmpty())
        wined3d = cfg->launcherValue("gamesUsesWined3d", "User Settings");
    const bool useWined3d = truthy(wined3d);
    QString dxOverrides;
    if (!useWined3d)
        dxOverrides = "d3d11=n;d3d10=n;d3d10core=n;dxgi=n;openvr_api_dxvk=n;d3d12=n;d3d12core=n;d3d9=n;d3d8=n;";
    QString overrides = cfg->gameValue(gameName, "overrides");
    env.insert("WINEDLLOVERRIDES", dxOverrides + overrides);
    env.insert("WINEDEBUG", debug ? "1" : "-all");
    env.insert("WINEPREFIX", prefix);
    // Proton script appends "/pfx/" to STEAM_COMPAT_DATA_PATH internally.
    // So STEAM_COMPAT_DATA_PATH must be the parent of the actual pfx dir.
    QString compatDataPath = prefix;
    if (compatDataPath.endsWith("/pfx"))
        compatDataPath.chop(4);
    env.insert("STEAM_COMPAT_DATA_PATH", compatDataPath);
    env.insert("STEAM_COMPAT_CLIENT_INSTALL_PATH", steamClientPath());
    env.insert("STEAM_COMPAT_INSTALL_PATH", cfg->gameValue(gameName, "mainPath"));
    // LIBRARY_PATHS = .../steamapps (where the "common" folder lives)
    const QString mainPath = cfg->gameValue(gameName, "mainPath");
    QString steamappsDir;
    if (!mainPath.isEmpty()) {
        const QString commonDir = QFileInfo(mainPath).absolutePath();
        steamappsDir = QFileInfo(commonDir).absolutePath();
    }
    if (!steamappsDir.isEmpty())
        env.insert("STEAM_COMPAT_LIBRARY_PATHS", steamappsDir);
    env.insert("PROTON_USE_WINED3D", useWined3d ? "1" : "0");
    const QString sid = cfg->gameValue(gameName, "steamID");
    env.insert("SteamAppId", sid.isEmpty() ? "0" : sid);
    // Wayland driver (only when explicitly configured)
    QString wayland = cfg->gameValue(gameName, "nativeWayland");
    if (wayland.isEmpty())
        wayland = cfg->launcherValue("gamesUsesWayland", "User Settings");
    if (!wayland.isEmpty())
        env.insert("PROTON_ENABLE_WAYLAND", truthy(wayland) ? "1" : "0");
    // Steam overlay via LD_PRELOAD (absent key = disabled, mirrors Java)
    const QString steamOverlay = cfg->gameValue(gameName, "steamOverlay");
    if (truthy(steamOverlay)) {
        QString fakeId = cfg->gameValue(gameName, "fakeSteamID", "480");
        const QString scp = steamClientPath();
        QStringList preload;
        for (const QString &rel :
             {"ubuntu12_32/gameoverlayrenderer.so", "ubuntu12_64/gameoverlayrenderer.so"}) {
            if (QFile::exists(scp + "/" + rel))
                preload << scp + "/" + rel;
        }
        if (!preload.isEmpty()) {
            QString existing = env.value("LD_PRELOAD", QString::fromLocal8Bit(qgetenv("LD_PRELOAD")));
            env.insert("LD_PRELOAD", existing.isEmpty()
                                        ? preload.join(':')
                                        : preload.join(':') + ':' + existing);
        }
        env.insert("ENABLE_VK_LAYER_VALVE_steam_overlay_1", "1");
        env.insert("SteamOverlayGameId", fakeId);
        env.insert("SteamGameId", fakeId);
    }
    // Custom environment map ("environment" key, \\ separated, ==== k/v)
    const QString envString = cfg->gameValue(gameName, "environment");
    if (!envString.isEmpty()) {
        QString norm = envString;
        norm.replace("\\\\\\\\", "\\\\");
        const auto putKv = [&](const QString &entry) {
            const int i = entry.indexOf("====");
            if (i < 0) {
                if (!entry.trimmed().isEmpty())
                    env.insert(entry.trimmed(), "");
            } else {
                env.insert(entry.left(i).trimmed(), entry.mid(i + 4));
            }
        };
        if (!norm.contains("\\\\")) {
            putKv(norm);
        } else {
            for (const QString &e : norm.split("\\\\"))
                putKv(e);
        }
    }
    args << "waitforexitandrun" << exec;
    const QString argsAfter = cfg->gameValue(gameName, "argsAfter");
    if (!argsAfter.isEmpty())
        args += argsAfter.split(' ', Qt::SkipEmptyParts);
    // Steam runtime wrapper (findSteamRuntime equivalent)
    const QString rtFlag = cfg->gameValue(gameName, "steamRuntime");
    if (truthy(rtFlag)) {
        const QString rt = findSteamRuntime(protonName);
        if (rt.isEmpty()) {
            cfg->setGameValue(gameName, "steamRuntime", "false");
        } else {
            args.prepend("--");
            args.prepend(rt);
        }
    }
    const QString argsBefore = cfg->gameValue(gameName, "argsBefore");
    if (!argsBefore.isEmpty())
        args = argsBefore.split(' ', Qt::SkipEmptyParts) + args;
    // Working dir = exe parent (mirrors Java)
    const QFileInfo fi(exec);
    workDir = fi.dir().path();

    // Resolve Proton executable
    QString program = protonExecutable(protonName, "proton");
    if (program.isEmpty()) {
        const QStringList all = installedProtons();
        program = all.isEmpty() ? QString() : protonExecutable(all.last(), "proton");
    }

    // Use umu-launcher if enabled and available
    if (m_useUmu && isUmuAvailable()) {
        program = "umu-run";
        args.clear();
        const QString sid = cfg->gameValue(gameName, "steamID");
        if (!sid.isEmpty()) {
            args << "--gameid" << ("umu-" + sid)
                 << "--store" << "steam";
        }
        if (!protonName.isEmpty() && protonName != "GE-Proton Latest") {
            args << "--protonpath" << protonExecutable(protonName, "proton");
        }
        args << exec;
        const QString argsAfter = cfg->gameValue(gameName, "argsAfter");
        if (!argsAfter.isEmpty())
            args += argsAfter.split(' ', Qt::SkipEmptyParts);
        const QString argsBefore = cfg->gameValue(gameName, "argsBefore");
        if (!argsBefore.isEmpty())
            args = argsBefore.split(' ', Qt::SkipEmptyParts) + args;
    } else if (program.isEmpty()) {
        emit toast("No Proton executable found");
        return;
    }

    m_proc = new QProcess(this);
    m_proc->setProgram(program);
    m_proc->setArguments(args);
    // Sanitize: QProcess drops nulls; skip invalid names like Java does.
    {
        QProcessEnvironment clean;
        const QRegularExpression validName("^[a-zA-Z_][a-zA-Z0-9_]*$");
        for (const QString &k : env.keys()) {
            if (validName.match(k).hasMatch())
                clean.insert(k, env.value(k));
        }
        m_proc->setProcessEnvironment(clean);
    }
    if (!workDir.isEmpty())
        m_proc->setWorkingDirectory(workDir);
    connect(m_proc, &QProcess::finished, this, &ProtonManager::onGameFinished);
    connect(m_proc, &QProcess::readyReadStandardOutput, this, [this] {
        emit gameLogOutput(QString::fromLocal8Bit(m_proc->readAllStandardOutput()));
    });
    connect(m_proc, &QProcess::readyReadStandardError, this, [this] {
        emit gameLogOutput(QString::fromLocal8Bit(m_proc->readAllStandardError()));
    });
    m_currentGame = gameName;
    m_running = true;
    emit runningChanged();
    emit currentGameChanged();
    cfg->setGameValue(gameName, "lastPlayed",
                      QString::number(QDateTime::currentSecsSinceEpoch()));
    m_proc->start();
    if (!m_proc->waitForStarted(10000)) {
        emit toast("Failed to start game process");
        m_proc->deleteLater();
        m_proc = nullptr;
        m_running = false;
        emit runningChanged();
    } else {
        m_startEpoch = QDateTime::currentSecsSinceEpoch();
    }
}

void ProtonManager::onGameFinished(int exitCode, QProcess::ExitStatus) {
    const QString game = m_currentGame;
    // Accumulate playtime (mirrors Java updateTimeSpent flow)
    if (!game.isEmpty() && m_startEpoch > 0) {
        const qint64 elapsed = QDateTime::currentSecsSinceEpoch() - m_startEpoch;
        if (elapsed > 0) {
            ConfigManager *cfg = ConfigManager::instance();
            const qint64 prev = cfg->gameValue(game, "timeSpent", "0").toLongLong();
            cfg->setGameValue(game, "timeSpent", QString::number(prev + elapsed));
        }
    }
    m_startEpoch = 0;
    if (m_proc) {
        m_proc->deleteLater();
        m_proc = nullptr;
    }
    m_running = false;
    m_currentGame.clear();
    emit runningChanged();
    emit currentGameChanged();
    emit gameFinished(game, exitCode);
}

void ProtonManager::runWineTool(const QString &gameName, const QString &tool) {
    if (gameName.isEmpty() || tool.isEmpty())
        return;
    ConfigManager *cfg = ConfigManager::instance();
    const QString protonName =
        cfg->gameValue(gameName, "proton", cfg->launcherValue("defaultProton", "User Settings", "GE-Proton Latest"));
    QString wine;
    for (const QString &dir :
         {protonsDir() + "/" + protonName,
          ConfigManager::expectedHome() + "/.local/share/Steam/compatibilitytools.d/" + protonName}) {
        for (const QString &cand :
             {dir + "/files/bin/wine64", dir + "/files/bin/wine", dir + "/dist/bin/wine64"}) {
            if (QFile::exists(cand)) {
                wine = cand;
                break;
            }
        }
        if (!wine.isEmpty())
            break;
    }
    if (wine.isEmpty()) {
        emit toast("No Wine binary found");
        return;
    }
    const QString prefix = ensurePrefixPath(gameName);
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    if (!prefix.isEmpty())
        env.insert("WINEPREFIX", prefix);
    // Detached GUI tool (mirrors utilities menu: winecfg/taskmgr/control/explorer/cmd).
    // Managed QProcess (no static startDetached env overload); auto-cleaned on finish.
    auto *p = new QProcess(this);
    p->setProgram(wine);
    p->setArguments({tool});
    p->setProcessEnvironment(env);
    p->setWorkingDirectory(QDir::homePath());
    connect(p, &QProcess::finished, p, &QObject::deleteLater);
    p->start();
}

void ProtonManager::stopGame() {
    if (!m_running)
        return;
    const QString game = m_currentGame;
    const QString prefix = prefixPath(game);
    if (m_proc) {
        // Try wineserver -k with WINEPREFIX to only kill this prefix's processes
        const QString prog = m_proc->program();
        const QString ws = prog.left(prog.lastIndexOf('/') + 1) + "wineserver";
        if (QFile::exists(ws) && !prefix.isEmpty()) {
            QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
            env.insert("WINEPREFIX", prefix);
            QProcess wsProc;
            wsProc.setProcessEnvironment(env);
            wsProc.start(ws, {"-k"});
            wsProc.waitForFinished(5000);
        }
        // Terminate the main process
        m_proc->terminate();
        if (!m_proc->waitForFinished(3000))
            m_proc->kill();
    }
    // Reset state
    m_running = false;
    m_currentGame.clear();
    emit runningChanged();
    emit currentGameChanged();
}

void ProtonManager::fetchReleases() {
    struct Feed { QString url; QString source; };
    const QList<Feed> feeds = {
        {"https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases?per_page=20", "ge"},
        {"https://api.github.com/repos/CachyOS/proton-cachyos/releases?per_page=10", "cachy"},
    };
    auto pending = std::make_shared<int>(feeds.size());
    auto all = std::make_shared<QVariantList>();
    for (const Feed &feed : feeds) {
        QNetworkRequest req(QUrl(feed.url));
        req.setHeader(QNetworkRequest::UserAgentHeader, "CorkyTux/2.10");
        QNetworkReply *rep = m_nam->get(req);
        connect(rep, &QNetworkReply::finished, this, [this, rep, feed, pending, all] {
            rep->deleteLater();
            if (rep->error() == QNetworkReply::NoError) {
                const QJsonArray arr = QJsonDocument::fromJson(rep->readAll()).array();
                for (const QJsonValue &v : arr) {
                    const QJsonObject o = v.toObject();
                    QString url;
                    for (const QJsonValue &a : o.value("assets").toArray()) {
                        const QString n = a.toObject().value("name").toString();
                        // GE-Proton ships .tar.gz, CachyOS ships .tar.xz (x86_64 preferred)
                        if ((n.endsWith(".tar.gz") || n.endsWith(".tar.xz"))
                            && !n.contains("arm64") && !n.endsWith(".sha512sum")) {
                            url = a.toObject().value("browser_download_url").toString();
                            if (n.contains("x86_64"))
                                break;
                        }
                    }
                    if (!url.isEmpty())
                        all->append(QVariantMap({{"tag", o.value("tag_name").toString()},
                                                {"url", url},
                                                {"date", o.value("published_at").toString()},
                                                {"source", feed.source}}));
                }
            }
            if (--(*pending) == 0)
                emit releasesReady(*all);
        });
    }
}

void ProtonManager::downloadProton(const QString &tag, const QString &url) {
    m_dlProgress = 0.0;
    emit downloadProgressChanged();
    QNetworkRequest req{QUrl(url)};
    req.setHeader(QNetworkRequest::UserAgentHeader, "CorkyTux/2.10");
    QNetworkReply *rep = m_nam->get(req);
    // Use the actual file extension from the URL
    const QString ext = url.endsWith(".tar.xz") ? ".tar.xz" : ".tar.gz";
    const QString dest = protonsDir() + "/" + tag + ext;
    // Snapshot dirs BEFORE download so we can identify the extracted folder later
    // (birthTime is unreliable on ext4 – never guess by date).
    const QStringList before =
        QDir(protonsDir()).entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    QFile *file = new QFile(dest);
    if (!file->open(QIODevice::WriteOnly)) {
        emit downloadFinished(false, "Cannot write " + dest);
        rep->abort();
        rep->deleteLater();
        return;
    }
    connect(rep, &QNetworkReply::downloadProgress, this,
            [this](qint64 rx, qint64 total) {
                m_dlProgress = total > 0 ? double(rx) / double(total) : 0.0;
                emit downloadProgressChanged();
            });
    connect(rep, &QNetworkReply::finished, this, [this, rep, file, dest, tag, before] {
        file->write(rep->readAll());
        file->close();
        const bool ok = rep->error() == QNetworkReply::NoError;
        rep->deleteLater();
        file->deleteLater();
        if (!ok) {
            QFile::remove(dest);
            emit downloadFinished(false, "Download failed");
            return;
        }
        // Heavy extraction OFF the GUI thread (tar can take minutes).
        QtConcurrent::run([this, dest, tag, before] {
            QString err;
            QProcess tar;
            tar.setWorkingDirectory(protonsDir());
            tar.start("tar", {"xf", dest});
            if (!tar.waitForFinished(1000 * 60 * 20) || tar.exitCode() != 0)
                err = "Extraction failed: " + QString::fromLocal8Bit(tar.readAllStandardError()).trimmed();
            else
                QFile::remove(dest);
            if (err.isEmpty()) {
                // Normalize: rename the NEWLY extracted top folder to the tag.
                // New = present now but absent before; fallback to known prefixes.
                QStringList after =
                    QDir(protonsDir()).entryList(QDir::Dirs | QDir::NoDotAndDotDot);
                QStringList fresh;
                for (const QString &e : after) {
                    if (!before.contains(e) && e != tag)
                        fresh << e;
                }
                if (fresh.isEmpty()) {
                    for (const QString &e : after) {
                        if (e != tag && (e.startsWith("GE-Proton") || e.startsWith("proton-")
                                         || e.startsWith("Proton-")
                                         || e.contains("cachy", Qt::CaseInsensitive)))
                            fresh << e;
                    }
                }
                QDir dir(protonsDir());
                if (!QDir(dir.filePath(tag)).exists() && !fresh.isEmpty())
                    QDir().rename(dir.filePath(fresh.first()), dir.filePath(tag));
                // Root-owned guard: hand files back to the real user
                if (::getuid() == 0) {
                    QProcess::execute("chown", {"-R", ConfigManager::expectedUser() + ":" + ConfigManager::expectedUser(),
                                               dir.filePath(tag)});
                }
            }
            QMetaObject::invokeMethod(this, [this, err, tag] {
                m_dlProgress = err.isEmpty() ? 1.0 : 0.0;
                emit downloadProgressChanged();
                emit downloadFinished(err.isEmpty(), err.isEmpty() ? tag : err);
            });
        });
    });
}

void ProtonManager::removeProton(const QString &name) {
    if (name.isEmpty())
        return;
    QtConcurrent::run([name] {
        for (const QString &pp : ConfigManager::instance()->allProtonPaths()) {
            QDir d(pp + "/" + name);
            if (d.exists())
                d.removeRecursively();
        }
    });
}

qint64 ProtonManager::dirSize(const QString &path) {
    qint64 total = 0;
    QDirIterator it(path, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        it.next();
        total += it.fileInfo().size();
    }
    return total;
}

void ProtonManager::queryFolderSize(const QString &path) {
    QtConcurrent::run([this, path] {
        const qint64 bytes = dirSize(path);
        QMetaObject::invokeMethod(this, [this, path, bytes] {
            emit folderSizeReady(path, bytes);
        });
    });
}

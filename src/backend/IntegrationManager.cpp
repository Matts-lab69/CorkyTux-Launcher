#include "IntegrationManager.h"
#include "ConfigManager.h"

#include <QDir>
#include <QDirIterator>
#include <QDateTime>
#include <QDebug>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QImage>
#include <QImageReader>
#include <QTemporaryDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <functional>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QTimer>
#include <QUrl>
#include <QUrlQuery>
#include <QtConcurrent>

const char *IntegrationManager::SGDB_KEY = "0ab12f62e2d5e6b3717161be0c5e68fa";

IntegrationManager *IntegrationManager::instance() {
    static IntegrationManager inst;
    return &inst;
}

IntegrationManager::IntegrationManager(QObject *parent) : QObject(parent) {
    m_nam = new QNetworkAccessManager(this);
}

// ---------------- flags / keys ----------------

bool IntegrationManager::isEnabled(const QString &key) const {
    const QString v = ConfigManager::instance()->launcherValue("integration_" + key, "Integrations");
    return v == "1" || v.compare("true", Qt::CaseInsensitive) == 0;
}

void IntegrationManager::setEnabled(const QString &key, bool enabled) {
    ConfigManager::instance()->setLauncherValue("integration_" + key, enabled ? "1" : "0", "Integrations");
}

QString IntegrationManager::apiKey(const QString &key) const {
    const QString v = ConfigManager::instance()->launcherValue("integration_key_" + key, "Integrations");
    if (!v.isEmpty())
        return v;
    if (key == "steamgriddb")
        return QString::fromLatin1(SGDB_KEY);
    return {};
}

void IntegrationManager::setApiKey(const QString &key, const QString &value) {
    ConfigManager::instance()->setLauncherValue("integration_key_" + key, value, "Integrations");
}

// ---------------- paths ----------------

QStringList IntegrationManager::lutrisDataDirs() {
    QStringList dirs;
    QString xdg = qEnvironmentVariable("XDG_DATA_HOME",
                                       ConfigManager::expectedHome() + "/.local/share");
    const QString native = xdg + "/lutris";
    if (QDir(native).exists())
        dirs << native;
    const QString flatpak =
        ConfigManager::expectedHome() + "/.var/app/net.lutris.Lutris/data/lutris";
    if (QDir(flatpak).exists() && !dirs.contains(flatpak))
        dirs << flatpak;
    if (dirs.isEmpty())
        dirs << native;
    return dirs;
}

QString IntegrationManager::hicolorAppsDir() {
    QString xdg = qEnvironmentVariable("XDG_DATA_HOME",
                                       ConfigManager::expectedHome() + "/.local/share");
    return xdg + "/icons/hicolor/128x128/apps";
}

QStringList IntegrationManager::steamRoots() {
    const QString home = ConfigManager::expectedHome();
    QStringList roots;
    for (const QString &c :
         {home + "/.local/share/Steam", home + "/.steam/steam",
          home + "/.var/app/com.valvesoftware.Steam/data/Steam"}) {
        if (QDir(c).exists())
            roots << c;
    }
    return roots;
}

QStringList IntegrationManager::steamLibraries(const QString &steamRoot) {
    QStringList libs{steamRoot};
    QFile f(steamRoot + "/steamapps/libraryfolders.vdf");
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return libs;
    const QString content = QString::fromUtf8(f.readAll());
    static const QRegularExpression re("\"path\"\\s+\"([^\"]+)\"");
    for (const auto &m : re.globalMatch(content)) {
        QString p = m.captured(1);
        p.replace("\\\\", "/");
        if (QDir(p).exists() && !libs.contains(p))
            libs << p;
    }
    return libs;
}

QString IntegrationManager::vdfValue(const QString &vdf, const QString &key) {
    const QRegularExpression re('"' + QRegularExpression::escape(key) + "\"\\s+\"([^\"]+)\"");
    const auto m = re.match(vdf);
    return m.hasMatch() ? m.captured(1) : QString();
}

QVariantMap IntegrationManager::parseAcf(const QString &acfPath, const QString &steamapps) {
    QFile f(acfPath);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return {};
    const QString content = QString::fromUtf8(f.readAll());
    const QString appId = vdfValue(content, "appid");
    QString name = vdfValue(content, "name");
    const QString installDir = vdfValue(content, "installdir");
    if (appId.isEmpty())
        return {};
    if (name.isEmpty() || name.trimmed().toLongLong() || name == appId)
        name = installDir.isEmpty() ? ("Steam " + appId) : installDir;
    const QString libPath =
        installDir.isEmpty() ? QString() : steamapps + "/common/" + installDir;
    // Find main executable: scan for .exe files, pick largest
    QString executable;
    if (!libPath.isEmpty() && QDir(libPath).exists()) {
        qint64 bestSize = -1;
        QDirIterator it(libPath, {"*.exe"}, QDir::Files, QDirIterator::Subdirectories);
        while (it.hasNext()) {
            it.next();
            if (it.fileInfo().size() > bestSize) {
                bestSize = it.fileInfo().size();
                executable = it.filePath();
            }
        }
    }
    const QString steamPrefix = steamapps + "/compatdata/" + appId + "/pfx";
    return {{"appId", appId}, {"name", name}, {"installDir", installDir},
            {"libraryPath", libPath}, {"executable", executable},
            {"prefixPath", steamPrefix}};
}

bool IntegrationManager::isSteamTool(const QString &name) {
    const QString l = name.toLower();
    for (const QString &t :
         {"proton", "steamworks", "steam linux runtime", "redistributable",
          " runtime", "runtime", " sdk", "sdk", "dedicated server"})
        if (l.contains(t))
            return true;
    return false;
}

// ---------------- scans (async) ----------------

QMap<QString, QString> IntegrationManager::resolveLutrisYaml(const QString &configpath) {
    QMap<QString, QString> out;
    if (configpath.trimmed().isEmpty())
        return out;
    QString yml;
    for (const QString &dir : lutrisDataDirs()) {
        const QString c = dir + "/games/" + configpath.trimmed() + ".yml";
        if (QFile::exists(c)) {
            yml = c;
            break;
        }
    }
    if (yml.isEmpty())
        return out;
    QFile f(yml);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return out;
    bool inGame = false;
    for (const QString &raw : QString::fromUtf8(f.readAll()).split('\n')) {
        const QString line = raw.trimmed();
        if (line == "game:") {
            inGame = true;
            continue;
        }
        if (inGame && !raw.startsWith(' ') && !raw.startsWith('\t') && line.endsWith(':'))
            inGame = false;
        if (!inGame)
            continue;
        for (const QString &key : {"exe", "main_file", "prefix", "game_path"}) {
            if (!line.startsWith(key + ":"))
                continue;
            QString val = line.mid(key.size() + 1).trimmed();
            if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith('\'') && val.endsWith('\'')))
                val = val.mid(1, val.size() - 2);
            if (val.isEmpty())
                continue;
            if (key == "main_file" || key == "game_path") {
                if (!out.contains("exe"))
                    out["exe"] = val;
            } else if (!out.contains(key)) {
                out[key] = val;
            }
        }
    }
    return out;
}

QString IntegrationManager::slugify(const QString &name) {
    QString s = name.toLower();
    s.replace(QRegularExpression("[^a-z0-9]+"), "-");
    s.replace(QRegularExpression("^-|-$"), "");
    return s;
}

/** Normalized name for artwork matching: lowercase alnum only ("R.E.P.O." == "repo"). */
static QString normName(const QString &name) {
    QString s = name.toLower();
    s.remove(QRegularExpression("[^a-z0-9]"));
    return s;
}

QString IntegrationManager::copyArtwork(const QString &src, const QString &subdir,
                                        const QString &name) {
    const QString dest =
        ConfigManager::expectedHome() + "/.config/CorkyTux/" + subdir + "/" + name;
    QDir().mkpath(QFileInfo(dest).path());
    if (!QFile::copy(src, dest) && !QFile::exists(dest))
        return {};
    // QFile::copy fails if dest exists; that's fine, path is usable.
    return QFile::exists(dest) ? dest : QString();
}

static bool looksNumericName(const QString &n) {
    if (n.trimmed().isEmpty())
        return true;
    bool ok = false;
    n.trimmed().toLongLong(&ok);
    return ok;
}

void IntegrationManager::scanLutris() {
    QtConcurrent::run([this] {
        QVariantList games;
        // Primary: pga.db
        QString db;
        for (const QString &dir : lutrisDataDirs()) {
            const QString c = dir + "/pga.db";
            if (QFile::exists(c)) {
                db = c;
                break;
            }
        }
        auto pushGame = [&](const QString &slug, const QString &name, const QString &runner,
                            QString dir, QString exe, const QString &prefix, double playtime) {
            if (looksNumericName(name))
                return;
            if ((exe.isEmpty() || dir.isEmpty())) {
                // YAML is authoritative for exe/prefix
                // (configpath lookup happens in pga.db branch below)
            }
            games << QVariantMap({{"slug", slug}, {"name", name}, {"runner", runner},
                                  {"directory", dir}, {"executable", exe},
                                  {"prefix", prefix}, {"playtimeHours", playtime}});
        };
        if (!db.isEmpty()) {
            QProcess sqlite;
            sqlite.start("sqlite3", {db, "SELECT slug,name,runner,directory,executable,configpath,playtime FROM games WHERE installed=1;"});
            if (sqlite.waitForFinished(15000)) {
                for (const QString &line :
                     QString::fromUtf8(sqlite.readAllStandardOutput()).split('\n')) {
                    const QStringList cols = line.split('|');
                    if (cols.size() < 2 || cols[1].trimmed().isEmpty())
                        continue;
                    const QString slug = cols[0], nm = cols[1].trimmed();
                    const QString runner = cols.size() > 2 ? cols[2] : QString();
                    QString dir = cols.size() > 3 ? cols[3] : QString();
                    QString exe = cols.size() > 4 ? cols[4] : QString();
                    const QString cfg = cols.size() > 5 ? cols[5] : QString();
                    double pt = 0;
                    if (cols.size() > 6)
                        pt = cols[6].trimmed().toDouble();
                    QString prefix;
                    if ((exe.isEmpty() || dir.isEmpty()) && !cfg.trimmed().isEmpty()) {
                        const auto y = resolveLutrisYaml(cfg);
                        if (exe.isEmpty() && y.contains("exe"))
                            exe = y["exe"];
                        if (dir.isEmpty() && !exe.isEmpty()) {
                            const QFileInfo fi(exe);
                            dir = fi.dir().path();
                        }
                        prefix = y.value("prefix");
                    }
                    pushGame(slug, nm, runner, dir, exe, prefix, pt);
                }
            }
        }
        if (!games.isEmpty()) {
            const QVariantList copy = games;
            QMetaObject::invokeMethod(this, [this, copy] { emit lutrisScanReady(copy); });
            return;
        }
        // Fallback: `lutris -l` (names only)
        QProcess lutris;
        lutris.start("lutris", {"-l"});
        if (lutris.waitForFinished(15000)) {
            for (const QString &raw :
                 QString::fromUtf8(lutris.readAllStandardOutput()).split('\n')) {
                const QString line = raw.trimmed();
                if (line.isEmpty() || line.startsWith("Name") || line.startsWith("-")
                    || line.startsWith("[") || line.startsWith("(") || line.contains("WARNING")
                    || line.contains("Starting Lutris") || line.contains("Shutting down")
                    || line.contains("libgnutls") || line.contains("is AMD")
                    || !line.contains('|'))
                    continue;
                const QStringList cols = line.split('|');
                QString name, runner;
                int nameIdx = -1;
                for (int i = 0; i < cols.size(); ++i) {
                    const QString c = cols[i].trimmed();
                    if (c.isEmpty() || c == "yes" || c == "no")
                        continue;
                    bool num = false;
                    c.toLongLong(&num);
                    if (num)
                        continue;
                    name = c;
                    nameIdx = i;
                    break;
                }
                if (nameIdx >= 0 && nameIdx + 1 < cols.size())
                    runner = cols[nameIdx + 1].trimmed();
                if (!looksNumericName(name))
                    games << QVariantMap({{"slug", ""}, {"name", name}, {"runner", runner},
                                          {"directory", ""}, {"executable", ""},
                                          {"prefix", ""}, {"playtimeHours", 0.0}});
            }
        }
        const QVariantList copy = games;
        QMetaObject::invokeMethod(this, [this, copy] { emit lutrisScanReady(copy); });
    });
}

// ---------------- Steam scan ----------------

void IntegrationManager::scanSteam() {
    QtConcurrent::run([this] { doScanSteam(); });
}

void IntegrationManager::doScanSteam() {
    QVariantList games;
    for (const QString &root : steamRoots()) {
        for (const QString &lib : steamLibraries(root)) {
            const QString steamapps = lib + "/steamapps";
            QDir dir(steamapps);
            if (!dir.exists())
                continue;
            for (const QString &acf :
                 dir.entryList({"appmanifest_*.acf"}, QDir::Files, QDir::Name)) {
                QVariantMap game = parseAcf(steamapps + "/" + acf, steamapps);
                if (game.isEmpty())
                    continue;
                if (isSteamTool(game["name"].toString()))
                    continue;
                game["source"] = "steam";
                games << game;
            }
        }
    }
    QMetaObject::invokeMethod(this, [this, games] { emit steamScanReady(games); });
}

// ---------------- network helpers ----------------

static QNetworkRequest artRequest(const QUrl &url, const QString &bearer = {}) {
    QNetworkRequest req(url);
    req.setHeader(QNetworkRequest::UserAgentHeader, "CorkyTux/2.10");
    if (!bearer.isEmpty())
        req.setRawHeader("Authorization", ("Bearer " + bearer).toUtf8());
    return req;
}

static bool downloadTo(QNetworkAccessManager *nam, const QString &url, const QString &dest) {
    // Synchronous helper for worker threads (uses local event loop).
    QNetworkRequest req = artRequest(QUrl(url));
    // cdn2.steamgriddb.com 403s without a browser-like Referer
    if (QUrl(url).host().contains("steamgriddb"))
        req.setRawHeader("Referer", "https://www.steamgriddb.com/");
    QNetworkReply *rep = nam->get(req);
    QEventLoop loop;
    QTimer timer;
    timer.setSingleShot(true);
    QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
    QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
    timer.start(30000);
    loop.exec();
    bool ok = false;
    if (rep->error() == QNetworkReply::NoError) {
        const QByteArray body = rep->readAll();
        if (body.size() > 1000) {
            QDir().mkpath(QFileInfo(dest).path());
            QFile f(dest);
            if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                f.write(body);
                ok = true;
            }
        }
    }
    rep->deleteLater();
    return ok;
}

// ---------------- artwork resolver ----------------

void IntegrationManager::resolveArtwork(const QString &gameName, const QString &steamId) {
    QtConcurrent::run([this, gameName, steamId] {
        QNetworkAccessManager nam; // thread-local: QNAM is NOT thread-safe
        QString banner, icon;
        const QString home = ConfigManager::expectedHome();
        // 1) Steam CDN by AppID
        if (!steamId.trimmed().isEmpty()) {
            const QString id = steamId.trimmed();
            const QString b = home + "/.config/CorkyTux/banners/" + id + ".jpg";
            const QString i = home + "/.config/CorkyTux/icons/" + id + ".jpg";
            if (downloadTo(&nam, "https://cdn.cloudflare.steamstatic.com/steam/apps/" + id + "/header.jpg", b))
                banner = b;
            if (downloadTo(&nam, "https://cdn.cloudflare.steamstatic.com/steam/apps/" + id + "/logo.png", i))
                icon = i;
            // 1b) Store API fallback (header_image + capsule_image, unescaped)
            if (banner.isEmpty() || icon.isEmpty()) {
                QNetworkRequest req = artRequest(
                    QUrl("https://store.steampowered.com/api/appdetails?appids=" + id));
                QNetworkReply *rep = nam.get(req);
                QEventLoop loop;
                QTimer t;
                t.setSingleShot(true);
                QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
                QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
                t.start(15000);
                loop.exec();
                if (rep->error() == QNetworkReply::NoError) {
                    const QString body = QString::fromUtf8(rep->readAll());
                    auto grab = [&](const QString &field) {
                        QRegularExpression re('"' + field + "\"\\s*:\\s*\"([^\"]+)\"");
                        auto m = re.match(body);
                        return m.hasMatch() ? m.captured(1).replace("\\/", "/") : QString();
                    };
                    if (banner.isEmpty()) {
                        const QString u = grab("header_image");
                        if (!u.isEmpty()) {
                            const QString d = home + "/.config/CorkyTux/banners/" + id + "-store.jpg";
                            if (downloadTo(&nam, u, d))
                                banner = d;
                        }
                    }
                    if (icon.isEmpty()) {
                        const QString u = grab("capsule_image");
                        if (!u.isEmpty()) {
                            const QString d = home + "/.config/CorkyTux/icons/" + id + "-store.jpg";
                            if (downloadTo(&nam, u, d))
                                icon = d;
                        }
                    }
                }
                rep->deleteLater();
            }
        }
        // 2) Lutris local by slug
        if ((banner.isEmpty() || icon.isEmpty()) && !gameName.isEmpty()) {
            const QString slug = slugify(gameName);
            for (const QString &dir : lutrisDataDirs()) {
                if (banner.isEmpty()) {
                    for (const QString &ad : {"coverart", "banners"}) {
                        const QString src = dir + "/" + ad + "/" + slug + ".jpg";
                        if (QFile::exists(src)) {
                            const QString d = copyArtwork(src, "banners", slug + ".jpg");
                            if (!d.isEmpty()) {
                                banner = d;
                                break;
                            }
                        }
                    }
                }
                if (icon.isEmpty()) {
                    const QString h = hicolorAppsDir() + "/lutris_" + slug + ".png";
                    if (QFile::exists(h)) {
                        const QString d = copyArtwork(h, "icons", slug + ".png");
                        if (!d.isEmpty())
                            icon = d;
                    }
                }
                if (!banner.isEmpty() && !icon.isEmpty())
                    break;
            }
        }
        // 3) Lutris.net public API (free, no key)
        if (banner.isEmpty() && !gameName.isEmpty()) {
            QUrl url("https://lutris.net/api/games");
            QUrlQuery q;
            q.addQueryItem("search", gameName.trimmed());
            url.setQuery(q);
            QNetworkReply *rep = nam.get(artRequest(url));
            QEventLoop loop;
            QTimer t;
            t.setSingleShot(true);
            QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
            QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
            t.start(15000);
            loop.exec();
            if (rep->error() == QNetworkReply::NoError) {
                const QString body = QString::fromUtf8(rep->readAll());
                QString best;
                // strict pairing: split result objects, accept only
                // normalized-equal names ("R.E.P.O." == "repo").
                const QString wantNorm = normName(gameName);
                for (const QString &chunk : body.split("{\"id\"")) {
                    QRegularExpression nr("\"name\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"");
                    QRegularExpression ar("\"coverart\"\\s*:\\s*\"([^\"]+)\"");
                    auto nm = nr.match(chunk);
                    auto am = ar.match(chunk);
                    if (!nm.hasMatch() || !am.hasMatch())
                        continue;
                    const QString u = am.captured(1);
                    if (u.isEmpty() || u == "null")
                        continue;
                    if (normName(nm.captured(1)) == wantNorm) {
                        best = u;
                        break;
                    }
                }
                if (!best.isEmpty()) {
                    const QString d = home + "/.config/CorkyTux/banners/" + slugify(gameName) + "-lutris.jpg";
                    if (downloadTo(&nam, best, d)) {
                        banner = d;
                        if (icon.isEmpty())
                            icon = d;
                    }
                }
            }
            rep->deleteLater();
        }
        // 4) SteamGridDB (bundled key): grids for banner, icons endpoint for icon
        if ((banner.isEmpty() || icon.isEmpty()) && !gameName.isEmpty()) {
            const QString key = apiKey("steamgriddb");
            if (!key.isEmpty()) {
                auto sgdbGet = [&](const QString &url) {
                    QNetworkReply *rep = nam.get(artRequest(QUrl(url), key));
                    QEventLoop loop;
                    QTimer t;
                    t.setSingleShot(true);
                    QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
                    QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
                    t.start(15000);
                    loop.exec();
                    QByteArray body;
                    bool ok = rep->error() == QNetworkReply::NoError;
                    if (ok)
                        body = rep->readAll();
                    rep->deleteLater();
                    return body;
                };
                const QByteArray searchBody = sgdbGet(
                    "https://www.steamgriddb.com/api/v2/search/autocomplete/"
                    + QUrl::toPercentEncoding(gameName.trimmed()));
                // Strict name verification: accept only the entry whose name
                // matches the query ignoring case/punctuation/spaces.
                // (Autocomplete ranks by popularity; first hit is often wrong,
                // e.g. "REPO" -> "Repository" games.)
                const QString wantNorm = normName(gameName);
                QString gridId;
                {
                    QRegularExpression pairRe(
                        "\"id\"\\s*:\\s*(\\d+)[^}]*?\"name\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"");
                    for (const auto &m : pairRe.globalMatch(QString::fromUtf8(searchBody))) {
                        if (normName(m.captured(2)) == wantNorm) {
                            gridId = m.captured(1);
                            break;
                        }
                    }
                }
                if (!gridId.isEmpty()) {
                    if (banner.isEmpty()) {
                        const QByteArray gb = sgdbGet("https://www.steamgriddb.com/api/v2/grids/game/"
                                                      + gridId + "?dimensions=600x900&types=static");
                        QRegularExpression urlRe("\"url\"\\s*:\\s*\"([^\"]+)\"");
                        const auto um = urlRe.match(QString::fromUtf8(gb));
                        if (um.hasMatch()) {
                            const QString d = home + "/.config/CorkyTux/banners/"
                                + slugify(gameName) + "-sgdb.jpg";
                            if (downloadTo(&nam, um.captured(1).replace("\\/", "/"), d))
                                banner = d;
                        }
                    }
                    if (icon.isEmpty()) {
                        const QByteArray ib =
                            sgdbGet("https://www.steamgriddb.com/api/v2/icons/game/" + gridId + "?limit=20");
                        QRegularExpression urlRe("\"url\"\\s*:\\s*\"([^\"]+)\"");
                        QStringList urls;
                        for (const auto &mm :
                             urlRe.globalMatch(QString::fromUtf8(ib)))
                            urls << mm.captured(1).replace("\\/", "/");
                        std::sort(urls.begin(), urls.end(), [](const QString &a, const QString &b) {
                            return a.endsWith(".png") && !b.endsWith(".png");
                        });
                        for (const QString &u : urls) {
                            const QString d = home + "/.config/CorkyTux/icons/"
                                + slugify(gameName) + "-sgdb.png";
                            if (downloadTo(&nam, u, d)) {
                                icon = d;
                                break;
                            }
                        }
                    }
                }
            }
        }
        const QString b = banner, i = icon;
        QMetaObject::invokeMethod(this, [this, gameName, b, i] {
            emit artworkReady(gameName, b, i);
        });
    });
}

QStringList IntegrationManager::scanDirSync(const QString &dir) const {
    qInfo() << "[scanDirSync] in:" << dir;
    QStringList exes, other;
    const QDir root(dir);
    if (!root.exists())
        return {};
    // Recursive walk: ALL executables under the folder (mirrors old behavior).
    // Skips prefix dirs, hidden dirs and absurd depth; caps at 200 entries.
    std::function<void(const QDir &, int)> walk = [&](const QDir &d, int depth) {
        if (depth > 6 || (int)(exes.size() + other.size()) >= 200)
            return;
        const QString dirname = d.dirName();
        if (dirname.startsWith('.'))
            return;
        if (dirname.compare("prefix", Qt::CaseInsensitive) == 0
            || dirname.compare("pfx", Qt::CaseInsensitive) == 0
            || dirname.endsWith(".prefix", Qt::CaseInsensitive))
            return;
        for (const QString &f : d.entryList({"*.exe"}, QDir::Files))
            exes << d.filePath(f);
        for (const QString &pat : {"*.msi", "*.rar", "*.zip"}) {
            for (const QString &f : d.entryList({pat}, QDir::Files))
                other << d.filePath(f);
        }
        for (const QString &sub : d.entryList(QDir::Dirs | QDir::NoDotAndDotDot)) {
            walk(QDir(d.filePath(sub)), depth + 1);
            if ((int)(exes.size() + other.size()) >= 200)
                return;
        }
    };
    walk(root, 0);
    // .exe first (runnable), then installers/archives
    qInfo() << "[scanDirSync] found:" << (exes.size() + other.size());
    return exes + other;
}

QStringList IntegrationManager::scanDirForExtensions(const QString &dir,
                                                      const QStringList &extensions) const {
    qInfo() << "[scanDirForExtensions] in:" << dir << "exts:" << extensions;
    QStringList results;
    const QDir root(dir);
    if (!root.exists() || extensions.isEmpty())
        return {};
    // Build glob patterns: ["*.nds", "*.iso", ...]
    QStringList patterns;
    for (const QString &ext : extensions)
        patterns << "*." + ext;
    // Recursive walk
    std::function<void(const QDir &, int)> walk = [&](const QDir &d, int depth) {
        if (depth > 6 || results.size() >= 200)
            return;
        const QString dirname = d.dirName();
        if (dirname.startsWith('.'))
            return;
        for (const QString &f : d.entryList(patterns, QDir::Files))
            results << d.filePath(f);
        for (const QString &sub : d.entryList(QDir::Dirs | QDir::NoDotAndDotDot))
            walk(QDir(d.filePath(sub)), depth + 1);
    };
    walk(root, 0);
    qInfo() << "[scanDirForExtensions] found:" << results.size();
    return results;
}

QString IntegrationManager::findMainExe(const QString &dir) const {
    const QDir root(dir);
    if (!root.exists())
        return {};
    QStringList exes;
    // Shallow scan first (top-level .exe files)
    for (const QString &f : root.entryList({"*.exe"}, QDir::Files)) {
        const QString name = f.toLower();
        // Skip known installers/setup/launchers
        if (name.contains("setup") || name.contains("install") || name.contains("unins"))
            continue;
        if (name.contains("redist") || name.contains("vcredist") || name.contains("directx"))
            continue;
        exes << root.filePath(f);
    }
    if (exes.isEmpty()) {
        // Fallback: any .exe at top level
        exes = root.entryList({"*.exe"}, QDir::Files);
        if (exes.isEmpty())
            return {};
        return root.filePath(exes.first());
    }
    // Pick the largest .exe (most likely the game binary)
    QString best;
    qint64 bestSize = 0;
    for (const QString &e : exes) {
        const qint64 sz = QFileInfo(e).size();
        if (sz > bestSize) {
            bestSize = sz;
            best = e;
        }
    }
    return best;
}

bool IntegrationManager::removeDirRecursive(const QString &dir) const {
    if (dir.isEmpty())
        return false;
    QDir d(dir);
    if (!d.exists())
        return false;
    // Safety: never wipe home, root or config roots themselves
    const QString clean = QDir::cleanPath(d.absolutePath());
    const QString home = QDir::cleanPath(ConfigManager::expectedHome());
    if (clean == home || clean == "/" || clean == QDir::cleanPath(home + "/.config/CorkyTux"))
        return false;
    return d.removeRecursively();
}

void IntegrationManager::extractRarAsync(const QString &rarPath, const QString &destDir) {    QtConcurrent::run([this, rarPath, destDir] {
        QString err;
        QString mainExe;
        // tool: unrar preferred, 7z fallback (mirrors RarExtractor behavior)
        QString tool;
        for (const QString &t : {"unrar", "7z", "7zz", "7zr"}) {
            if (!QStandardPaths::findExecutable(t).isEmpty()) {
                tool = t;
                break;
            }
        }
        if (tool.isEmpty()) {
            err = "No unrar/7z tool found";
        } else {
            QDir().mkpath(destDir);
            QProcess proc;
            if (tool == "unrar")
                proc.start(tool, {"x", "-o+", rarPath, destDir + "/"});
            else
                proc.start(tool, {"x", "-y", "-o" + destDir, rarPath});
            if (!proc.waitForFinished(1000 * 60 * 30) || proc.exitCode() != 0) {
                err = "Extraction failed (exit " + QString::number(proc.exitCode()) + ")";
            } else {
                // largest .exe inside wins (mirrors largest-file pick)
                qint64 best = -1;
                QDirIterator it(destDir, {"*.exe"}, QDir::Files,
                                QDirIterator::Subdirectories);
                while (it.hasNext()) {
                    it.next();
                    if (it.fileInfo().size() > best) {
                        best = it.fileInfo().size();
                        mainExe = it.filePath();
                    }
                }
                if (mainExe.isEmpty())
                    err = "No .exe found after extraction";
            }
        }
        const QString e = err, m = mainExe, d = destDir;
        QMetaObject::invokeMethod(this, [this, e, m, d] {
            if (e.isEmpty())
                emit rarReady(d, m);
            else
                emit rarFailed(e);
        });
    });
}

void IntegrationManager::extractExeIconAsync(const QString &exePath, const QString &gameName) {
    QtConcurrent::run([this, exePath, gameName] {
        QString err, out;
        do {
            if (!exePath.toLower().endsWith(".exe")
                || !QFile::exists(exePath)) {
                err = "not a local .exe";
                break;
            }
            if (QStandardPaths::findExecutable("icoextract").isEmpty()
                || QStandardPaths::findExecutable("ffmpeg").isEmpty()) {
                err = "icoextract/ffmpeg missing";
                break;
            }
            QTemporaryDir tmp;
            if (!tmp.isValid()) {
                err = "no temp dir";
                break;
            }
            QProcess x;
            x.start("icoextract", {exePath, tmp.filePath("icon.ico")});
            if (!x.waitForFinished(60000) || x.exitCode() != 0) {
                err = "icoextract failed";
                break;
            }
            // pick largest icon file the extractor produced
            QString ico;
            qint64 best = -1;
            QDirIterator it(tmp.path(), {"*.ico", "*.png"}, QDir::Files,
                            QDirIterator::Subdirectories);
            while (it.hasNext()) {
                it.next();
                if (it.fileInfo().size() > best) {
                    best = it.fileInfo().size();
                    ico = it.filePath();
                }
            }
            if (ico.isEmpty()) {
                err = "no icon in exe";
                break;
            }
            QString png = ico;
            if (ico.toLower().endsWith(".ico")) {
                png = tmp.filePath("icon.png");
                QProcess ff;
                ff.start("ffmpeg", {"-y", "-i", ico, png});
                if (!ff.waitForFinished(60000) || ff.exitCode() != 0
                    || !QFile::exists(png)) {
                    err = "ffmpeg conversion failed";
                    break;
                }
            }
            // validate: loadable PNG >= 16px (else JavaFX/Qt chokes like the .ico case)
            QImageReader reader(png);
            if (!reader.canRead()) {
                err = "unreadable image";
                break;
            }
            const QSize sz = reader.size();
            if (!sz.isValid() || sz.width() < 16 || sz.height() < 16) {
                // try QImage load as final verdict (size() can be -1 for some PNGs)
                QImage img(png);
                if (img.isNull() || img.width() < 16 || img.height() < 16) {
                    err = "icon too small/unreadable";
                    break;
                }
            }
            const QString dest = ConfigManager::expectedHome()
                + "/.config/CorkyTux/icons/" + slugify(gameName) + "-exe.png";
            QDir().mkpath(QFileInfo(dest).path());
            QFile::remove(dest);
            if (!QFile::copy(png, dest)) {
                err = "cannot store icon";
                break;
            }
            out = dest;
        } while (false);
        const QString e = err, o = out, g = gameName;
        QMetaObject::invokeMethod(this, [this, e, o, g] {
            if (e.isEmpty())
                emit exeIconReady(g, o);
            else
                emit exeIconFailed(g, e);
        });
    });
}

// ---------------- ProtonDB / IGDB ----------------
void IntegrationManager::fetchProtonRating(const QString &appId) {
    QtConcurrent::run([this, appId] {
        QNetworkAccessManager nam; // thread-local: QNAM is NOT thread-safe
        QString tier, conf;
        int total = 0;
        QNetworkRequest req = artRequest(
            QUrl("https://www.protondb.com/api/v1/reports/summaries/" + appId.trimmed() + ".json"));
        QNetworkReply *rep = nam.get(req);
        QEventLoop loop;
        QTimer t;
        t.setSingleShot(true);
        QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
        QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
        t.start(15000);
        loop.exec();
        if (rep->error() == QNetworkReply::NoError) {
            const QString body = QString::fromUtf8(rep->readAll());
            auto grab = [&](const QString &f) {
                QRegularExpression re('"' + f + "\"\\s*:\\s*\"([^\"]+)\"");
                auto m = re.match(body);
                return m.hasMatch() ? m.captured(1) : QString();
            };
            tier = grab("tier");
            conf = grab("confidence");
            QRegularExpression tr("\"total\"\\s*:\\s*(\\d+)");
            auto tm = tr.match(body);
            if (tm.hasMatch())
                total = tm.captured(1).toInt();
        }
        rep->deleteLater();
        const QString a = appId, ti = tier, co = conf;
        QMetaObject::invokeMethod(this, [this, a, ti, co, total] {
            emit protonRatingReady(a, ti, co, total);
        });
    });
}

void IntegrationManager::fetchIgdbGame(const QString &name) {
    QtConcurrent::run([this, name] {
        QNetworkAccessManager nam; // thread-local: QNAM is NOT thread-safe
        const QString creds = apiKey("igdb");
        double rating = -1;
        QString summary;
        const int colon = creds.indexOf(':');
        if (!creds.isEmpty() && colon > 0 && !name.trimmed().isEmpty()) {
            const QString cid = creds.left(colon).trimmed();
            const QString secret = creds.mid(colon + 1).trimmed();
            const qint64 now = QDateTime::currentSecsSinceEpoch();
            QString token;
            {
                QMutexLocker lock(&m_igdbMutex);
                if (!m_igdbToken.isEmpty() && now <= m_igdbTokenExpiry)
                    token = m_igdbToken;
            }
            if (token.isEmpty()) {
                QUrl url("https://id.twitch.tv/oauth2/token");
                QUrlQuery q;
                q.addQueryItem("client_id", cid);
                q.addQueryItem("client_secret", secret);
                q.addQueryItem("grant_type", "client_credentials");
                url.setQuery(q);
                QNetworkRequest req(url);
                req.setHeader(QNetworkRequest::ContentTypeHeader, "application/x-www-form-urlencoded");
                QNetworkReply *rep = nam.post(req, QByteArray());
                QEventLoop loop;
                QTimer t;
                t.setSingleShot(true);
                QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
                QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
                t.start(15000);
                loop.exec();
                if (rep->error() == QNetworkReply::NoError) {
                    QRegularExpression tr("\"access_token\"\\s*:\\s*\"([^\"]+)\"");
                    auto m = tr.match(QString::fromUtf8(rep->readAll()));
                    if (m.hasMatch()) {
                        QMutexLocker lock(&m_igdbMutex);
                        m_igdbToken = m.captured(1);
                        m_igdbTokenExpiry = QDateTime::currentSecsSinceEpoch() + 3600;
                        token = m_igdbToken;
                    }
                }
                rep->deleteLater();
            }
            if (!token.isEmpty()) {
                QNetworkRequest req(QUrl("https://api.igdb.com/v4/games"));
                req.setRawHeader("Client-ID", cid.toUtf8());
                req.setRawHeader("Authorization", ("Bearer " + token).toUtf8());
                const QString clean = QString(name).replace('"', "");
                QNetworkReply *rep = nam.post(
                    req, ("search \"" + clean.trimmed() + "\"; fields name,rating,summary; limit 1;").toUtf8());
                QEventLoop loop;
                QTimer t;
                t.setSingleShot(true);
                QObject::connect(rep, &QNetworkReply::finished, &loop, &QEventLoop::quit);
                QObject::connect(&t, &QTimer::timeout, &loop, &QEventLoop::quit);
                t.start(15000);
                loop.exec();
                if (rep->error() == QNetworkReply::NoError) {
                    const QString body = QString::fromUtf8(rep->readAll());
                    QRegularExpression rr("\"rating\"\\s*:\\s*([\\d.]+)");
                    auto rm = rr.match(body);
                    if (rm.hasMatch())
                        rating = rm.captured(1).toDouble();
                    QRegularExpression sr("\"summary\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"");
                    auto sm = sr.match(body);
                    if (sm.hasMatch())
                        summary = sm.captured(1).replace("\\\"", "\"");
                }
                rep->deleteLater();
            }
        }
        QMetaObject::invokeMethod(this, [this, name, rating, summary] {
            emit igdbReady(name, rating, summary);
        });
    });
}

// ---------------- plugin runner ----------------

void IntegrationManager::runPlugin(const QString &pluginPath, const QStringList &args) {
    const bool isInstall = (args.size() > 0 && args[0] == "install");
    QtConcurrent::run([this, pluginPath, args, isInstall] {
        QProcess proc;
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert("PYTHONUNBUFFERED", "1");
        proc.setProcessEnvironment(env);
        proc.setWorkingDirectory(QFileInfo(pluginPath).absolutePath());

        const QString python = "/usr/bin/python3";
        QStringList fullArgs;
        fullArgs << pluginPath << args;

        qDebug() << "[PluginRunner] Starting:" << python << fullArgs;
        proc.start(python, fullArgs);

        const int timeout = isInstall ? 600000 : 30000;

        if (isInstall) {
            // For installs, read line-by-line for real-time progress
            while (proc.state() != QProcess::NotRunning) {
                if (!proc.waitForReadyRead(1000)) {
                    if (proc.state() == QProcess::NotRunning) break;
                    if (!proc.waitForFinished(1000)) {
                        // Check for timeout
                        qDebug() << "[PluginRunner] Install still running...";
                    }
                    continue;
                }
                QByteArray line = proc.readLine().trimmed();
                if (line.isEmpty()) continue;

                QJsonDocument doc = QJsonDocument::fromJson(line);
                QVariantMap msg = doc.object().toVariantMap();
                QString type = msg.value("type").toString();
                qDebug() << "[PluginRunner] Line:" << line.left(200);

                if (type == "progress") {
                    QMetaObject::invokeMethod(this, [this, msg] { emit pluginProgress(msg); });
                } else if (type == "done") {
                    // Final result
                    QMetaObject::invokeMethod(this, [this, msg] { emit pluginResult(msg); });
                    return;
                }
            }
            // Process finished without a "done" line
            proc.waitForFinished(5000);
            QByteArray out = proc.readAllStandardOutput();
            QByteArray err = proc.readAllStandardError();
            qDebug() << "[PluginRunner] Install done, exit:" << proc.exitCode();
            if (!out.isEmpty()) {
                QJsonDocument doc = QJsonDocument::fromJson(out.trimmed());
                QVariantMap res = doc.object().toVariantMap();
                if (!res.isEmpty() && res.value("type").toString() == "done") {
                    QMetaObject::invokeMethod(this, [this, res] { emit pluginResult(res); });
                    return;
                }
            }
            // Fallback
            QVariantMap res;
            res["ok"] = proc.exitCode() == 0;
            res["type"] = "done";
            res["installed"] = QStringList();
            res["failed"] = QStringList();
            res["error"] = QString::fromUtf8(err).left(200);
            QMetaObject::invokeMethod(this, [this, res] { emit pluginResult(res); });
        } else {
            // For scans, single JSON output
            if (!proc.waitForFinished(timeout)) {
                proc.kill();
                proc.waitForFinished(2000);
                QVariantMap res;
                res["ok"] = false;
                res["error"] = "No dependencies needed";
                QMetaObject::invokeMethod(this, [this, res] { emit pluginResult(res); });
                return;
            }
            const QByteArray out = proc.readAllStandardOutput();
            const QByteArray err = proc.readAllStandardError();
            qDebug() << "[PluginRunner] Exit:" << proc.exitCode() << "stdout:" << out.left(500);
            if (!err.isEmpty()) qDebug() << "[PluginRunner] stderr:" << err.left(500);
            const QJsonDocument doc = QJsonDocument::fromJson(out);
            QVariantMap res = doc.object().toVariantMap();
            if (res.isEmpty()) {
                res["ok"] = false;
                res["error"] = err.isEmpty() ? "No output from plugin" : QString::fromUtf8(err).left(200);
            }
            QMetaObject::invokeMethod(this, [this, res] { emit pluginResult(res); });
        }
    });
}

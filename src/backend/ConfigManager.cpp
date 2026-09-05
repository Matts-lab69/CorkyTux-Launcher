#include "ConfigManager.h"

#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QTextStream>
#include <QUrl>
#include <unistd.h>

ConfigManager *ConfigManager::instance() {
    static ConfigManager inst;
    return &inst;
}

QString ConfigManager::expectedUser() {
    // SUDO_USER wins; else first /home/* owner; else $USER.
    const QByteArray sudo = qgetenv("SUDO_USER");
    if (!sudo.isEmpty() && sudo != "root")
        return QString::fromLocal8Bit(sudo);
    QDir home("/home");
    const auto entries = home.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    for (const QString &e : entries) {
        if (e == "lost+found")
            continue;
        if (QDir("/home/" + e).exists())
            return e;
    }
    const QByteArray user = qgetenv("USER");
    if (!user.isEmpty() && user != "root")
        return QString::fromLocal8Bit(user);
    return QStringLiteral("root");
}

QString ConfigManager::expectedHome() {
    if (::getuid() == 0)
        return QStringLiteral("/home/") + expectedUser();
    QString h = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    return h.isEmpty() ? QStringLiteral("/home/") + expectedUser() : h;
}

QString ConfigManager::configDir() {
    static const QString d = expectedHome() + "/.config/CorkyTux";
    QDir().mkpath(d);
    return d;
}

QString ConfigManager::bannersDir() {
    static const QString d = configDir() + "/banners";
    QDir().mkpath(d);
    return d;
}

QString ConfigManager::iconsDir() {
    static const QString d = configDir() + "/icons";
    QDir().mkpath(d);
    return d;
}

QString ConfigManager::dataDir() {
    static const QString d = expectedHome() + "/.local/share/CorkyTux";
    QDir().mkpath(d);
    return d;
}

QString ConfigManager::prefixesDir() {
    static const QString d = dataDir() + "/prefixes";
    QDir().mkpath(d);
    return d;
}

ConfigManager::ConfigManager(QObject *parent) : QObject(parent) {
    m_gamesPath = configDir() + "/Games.ini";
    m_launcherPath = configDir() + "/Launcher.ini";
}

ConfigManager::IniFile ConfigManager::readIni(const QString &path) {
    IniFile data;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return data;
    QTextStream in(&f);
    in.setEncoding(QStringConverter::Utf8);
    QString section;
    while (!in.atEnd()) {
        QString line = in.readLine().trimmed();
        if (line.isEmpty() || line.startsWith(';') || line.startsWith('#'))
            continue;
        if (line.startsWith('[') && line.endsWith(']')) {
            section = line.mid(1, line.size() - 2).trimmed();
            if (!data.data.contains(section)) {
                data.data[section] = {};
                data.sectionOrder << section;
            }
            continue;
        }
        const int eq = line.indexOf('=');
        if (eq < 0)
            continue;
        const QString key = line.left(eq).trimmed();
        // a key before any section goes to the "" section (tolerated)
        if (!data.data.contains(section)) {
            data.data[section] = {};
            data.sectionOrder << section;
        }
        if (!data.keyOrder[section].contains(key))
            data.keyOrder[section] << key;
        data.data[section][key] = line.mid(eq + 1).trimmed();
    }
    return data;
}

bool ConfigManager::writeIni(const QString &path, const IniFile &data) {
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Text | QIODevice::Truncate))
        return false;
    QTextStream out(&f);
    out.setEncoding(QStringConverter::Utf8);
    for (const QString &section : data.sectionOrder) {
        if (!data.data.contains(section))
            continue;
        out << '[' << section << "]\n";
        for (const QString &key : data.keyOrder.value(section)) {
            if (!data.data.value(section).contains(key))
                continue;
            out << key << '=' << data.data.value(section).value(key) << '\n';
        }
        out << '\n';
    }
    return true;
}

QStringList ConfigManager::gameNames() const {
    return readIni(m_gamesPath).sectionOrder;
}

QMap<QString, QString> ConfigManager::gameSection(const QString &name) const {
    return readIni(m_gamesPath).data.value(name);
}

QString ConfigManager::gameValue(const QString &name, const QString &key,
                                 const QString &fallback) const {
    const auto data = readIni(m_gamesPath);
    return data.data.value(name).value(key, fallback);
}

void ConfigManager::setGameValue(const QString &name, const QString &key,
                                 const QString &value) {
    if (name.isEmpty() || key.isEmpty())
        return;
    auto data = readIni(m_gamesPath);
    if (!data.data.contains(name)) {
        data.data[name] = {};
        data.sectionOrder << name;
    }
    if (!data.keyOrder[name].contains(key))
        data.keyOrder[name] << key;
    data.data[name][key] = value;
    if (writeIni(m_gamesPath, data))
        emit gamesChanged();
}

void ConfigManager::removeGame(const QString &name) {
    auto data = readIni(m_gamesPath);
    if (!data.data.contains(name))
        return;
    data.data.remove(name);
    data.sectionOrder.removeAll(name);
    data.keyOrder.remove(name);
    if (writeIni(m_gamesPath, data))
        emit gamesChanged();
}

bool ConfigManager::hasGame(const QString &name) const {
    return readIni(m_gamesPath).data.contains(name);
}

QString ConfigManager::launcherValue(const QString &key, const QString &section,
                                     const QString &fallback) const {
    const auto data = readIni(m_launcherPath);
    return data.data.value(section).value(key, fallback);
}

void ConfigManager::setLauncherValue(const QString &key, const QString &value,
                                     const QString &section) {
    if (key.isEmpty())
        return;
    auto data = readIni(m_launcherPath);
    const QString sec = section.isEmpty() ? QStringLiteral("User Settings") : section;
    if (!data.data.contains(sec)) {
        data.data[sec] = {};
        data.sectionOrder << sec;
    }
    if (!data.keyOrder[sec].contains(key))
        data.keyOrder[sec] << key;
    data.data[sec][key] = value;
    if (writeIni(m_launcherPath, data))
        emit launcherChanged();
}

bool ConfigManager::saveTextFile(const QString &urlOrPath, const QString &text) {
    QString path = urlOrPath;
    if (path.startsWith("file://"))
        path = QUrl(path).toLocalFile();
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Text | QIODevice::Truncate))
        return false;
    QTextStream out(&f);
    out.setEncoding(QStringConverter::Utf8);
    out << text;
    return true;
}

void ConfigManager::ensureDir(const QString &path) {
    QDir().mkpath(path);
}

QString ConfigManager::basePathFor(const QString &forWhat) const {
    const QString def = expectedHome() + "/.local/share/CorkyTux/" + forWhat;
    const QString custom = launcherValue(forWhat + "Path", "User Settings");
    if (custom.trimmed().isEmpty()) {
        QDir().mkpath(def);
        return def;
    }
    return custom;
}

QStringList ConfigManager::allProtonPaths() const {
    QStringList paths;
    const QString main = basePathFor("protons");
    if (!main.isEmpty())
        paths << main;
    for (const QString &extra :
         {launcherValue("protonsPath2", "User Settings"),
          launcherValue("protonsPath3", "User Settings")}) {
        if (!extra.trimmed().isEmpty() && !paths.contains(extra))
            paths << extra;
    }
    return paths;
}

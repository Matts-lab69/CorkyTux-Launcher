#include "GameModel.h"
#include "ConfigManager.h"
#include "PluginManager.h"

#include <QDateTime>
#include <QDir>
#include <QFile>

GameModel::GameModel(QObject *parent)
    : QAbstractListModel(parent), m_cfg(ConfigManager::instance()) {
    loadFromDisk();
    connect(m_cfg, &ConfigManager::gamesChanged, this, &GameModel::reload);
}

int GameModel::rowCount(const QModelIndex &parent) const {
    return parent.isValid() ? 0 : m_games.size();
}

QVariant GameModel::data(const QModelIndex &index, int role) const {
    if (!index.isValid() || index.row() < 0 || index.row() >= m_games.size())
        return {};
    const GameEntry &g = m_games.at(index.row());
    switch (role) {
    case NameRole: return g.name;
    case ExecutableRole: return g.executable;
    case MainPathRole: return g.mainPath;
    case PrefixPathRole: return g.prefixPath;
    case ProtonRole: return g.proton;
    case SteamIdRole: return g.steamID;
    case BannerRole: return g.banner;
    case IconRole: return g.icon;
    case TimeSpentRole: return g.timeSpent;
    case LastPlayedRole: return g.lastPlayed;
    case FavoriteRole: return g.favorite;
    case SourceRole: return g.source;
    case ExecutorRole: return g.executor;
    case Qt::DisplayRole: return g.name;
    default: return {};
    }
}

QHash<int, QByteArray> GameModel::roleNames() const {
    return {
        {NameRole, "name"}, {ExecutableRole, "executable"},
        {MainPathRole, "mainPath"}, {PrefixPathRole, "prefixPath"},
        {ProtonRole, "proton"}, {SteamIdRole, "steamID"},
        {BannerRole, "banner"}, {IconRole, "icon"},
        {TimeSpentRole, "timeSpent"}, {LastPlayedRole, "lastPlayed"},
        {FavoriteRole, "favorite"}, {SourceRole, "source"},
        {ExecutorRole, "executor"},
    };
}

void GameModel::loadFromDisk() {
    beginResetModel();
    m_games.clear();
    for (const QString &name : m_cfg->gameNames())
        m_games.append(entryFromMap(name, m_cfg->gameSection(name)));
    endResetModel();
    emit countChanged();
}

GameEntry GameModel::entryFromMap(const QString &name,
                                  const QMap<QString, QString> &m) const {
    GameEntry g;
    g.name = name;
    g.executable = m.value("executable");
    g.mainPath = m.value("mainPath");
    g.prefixPath = m.value("prefixPath");
    g.proton = m.value("proton");
    g.overrides = m.value("overrides");
    g.steamID = m.value("steamID");
    g.banner = m.value("banner");
    g.icon = m.value("icon");
    g.timeSpent = m.value("timeSpent", "0").toLongLong();
    g.lastPlayed = m.value("lastPlayed", "0").toLongLong();
    const QString fav = m.value("favorite");
    g.favorite = (fav == "1" || fav.compare("true", Qt::CaseInsensitive) == 0);
    g.source = m.value("source");
    g.lutrisRunner = m.value("lutrisRunner");
    g.executor = m.value("executor");
    return g;
}

void GameModel::reload() {
    loadFromDisk();
}

bool GameModel::addGame(const QString &name, const QVariantMap &fields) {
    if (name.trimmed().isEmpty())
        return false;
    const QString executor = fields.value("executor").toString();
    const bool isEmulator = !executor.isEmpty();
    const QString defProton =
        m_cfg->launcherValue("defaultProton", "User Settings", "GE-Proton Latest");
    if (!m_cfg->hasGame(name)) {
        if (!isEmulator)
            m_cfg->setGameValue(name, "proton", defProton);
        m_cfg->setGameValue(name, "executor", executor);
    }
    for (auto it = fields.constBegin(); it != fields.constEnd(); ++it) {
        if (!it.value().toString().isEmpty())
            m_cfg->setGameValue(name, it.key(), it.value().toString());
    }
    // Only auto-set prefixPath for Wine/Proton games (emulators don't need it)
    if (!isEmulator && m_cfg->gameValue(name, "prefixPath").isEmpty())
        m_cfg->setGameValue(name, "prefixPath",
                            ConfigManager::prefixesDir() + "/" + name + "/pfx");
    loadFromDisk();
    return true;
}

bool GameModel::importExternalGame(const QString &name, const QVariantMap &fields) {
    if (name.trimmed().isEmpty())
        return false;
    if (m_cfg->hasGame(name))
        return false; // already exists, skip
    return addGame(name, fields);
}

void GameModel::removeGame(const QString &name) {
    m_cfg->removeGame(name); // emits gamesChanged -> reload
}

void GameModel::removeGameFull(const QString &name, bool removePrefix, bool removeFiles) {
    if (name.isEmpty())
        return;
    const QMap<QString, QString> section = m_cfg->gameSection(name);
    auto rmDir = [](const QString &p) {
        if (p.isEmpty())
            return;
        QDir d(p);
        if (d.exists())
            d.removeRecursively();
    };
    auto rmFile = [](const QString &p) {
        if (!p.isEmpty())
            QFile::remove(p);
    };
    if (removePrefix) {
        QString pp = section.value("prefixPath");
        if (pp.isEmpty())
            pp = ConfigManager::prefixesDir() + "/" + name;
        rmDir(pp);
    }
    if (removeFiles) {
        const QString mp = section.value("mainPath");
        if (!mp.isEmpty() && QDir(mp).exists())
            rmDir(mp);
    }
    rmFile(section.value("banner"));
    rmFile(section.value("icon"));
    rmFile(ConfigManager::expectedHome() + "/Desktop/" + name + ".desktop");
    rmFile(ConfigManager::expectedHome() + "/.local/share/applications/" + name + ".desktop");
    m_cfg->removeGame(name);
}

QVariantMap GameModel::getGame(const QString &name) const {
    for (const GameEntry &g : m_games) {
        if (g.name != name)
            continue;
        QVariantMap out = {
            {"name", g.name}, {"executable", g.executable},
            {"mainPath", g.mainPath}, {"prefixPath", g.prefixPath},
            {"proton", g.proton}, {"overrides", g.overrides},
            {"steamID", g.steamID}, {"banner", g.banner}, {"icon", g.icon},
            {"timeSpent", g.timeSpent}, {"lastPlayed", g.lastPlayed},
            {"favorite", g.favorite}, {"source", g.source},
            {"executor", g.executor},
        };
        // pass through any extra INI keys (env, argsBefore/After, flags...)
        const QMap<QString, QString> section = m_cfg->gameSection(name);
        for (auto it = section.constBegin(); it != section.constEnd(); ++it) {
            if (!out.contains(it.key()))
                out[it.key()] = it.value();
        }
        return out;
    }
    return {};
}

void GameModel::setFavorite(const QString &name, bool fav) {
    m_cfg->setGameValue(name, "favorite", fav ? "1" : "0");
}

void GameModel::touchLastPlayed(const QString &name) {
    m_cfg->setGameValue(name, "lastPlayed",
                        QString::number(QDateTime::currentSecsSinceEpoch()));
}

// ---------------- GameFilterProxy ----------------

GameFilterProxy::GameFilterProxy(QObject *parent)
    : QSortFilterProxyModel(parent) {
    setSortRole(GameModel::NameRole);
    setSortCaseSensitivity(Qt::CaseInsensitive);
    setFilterCaseSensitivity(Qt::CaseInsensitive);
}

void GameFilterProxy::setMode(const QString &mode) {
    static const QStringList valid = {QStringLiteral("all"),
                                      QStringLiteral("favorites"), QStringLiteral("az"),
                                      QStringLiteral("mostplayed"), QStringLiteral("recent")};
    const QString m = valid.contains(mode) ? mode : QStringLiteral("all");
    if (m == m_mode)
        return;
    m_mode = m;
    invalidateFilter();
    sort(0, Qt::AscendingOrder);
    emit modeChanged();
}

void GameFilterProxy::setSearch(const QString &search) {
    if (search == m_search)
        return;
    m_search = search;
    invalidateFilter();
    emit searchChanged();
}

bool GameFilterProxy::filterAcceptsRow(int row, const QModelIndex &parent) const {
    const QModelIndex idx = sourceModel()->index(row, 0, parent);
    if (!m_search.trimmed().isEmpty()) {
        const QString name = sourceModel()->data(idx, GameModel::NameRole).toString();
        if (!name.contains(m_search.trimmed(), Qt::CaseInsensitive))
            return false;
    }
    if (m_mode == "favorites")
        return sourceModel()->data(idx, GameModel::FavoriteRole).toBool();
    return true;
}

bool GameFilterProxy::lessThan(const QModelIndex &l, const QModelIndex &r) const {
    if (m_mode == "all" || m_mode == "favorites")
        return false; // stable sort => source (insertion) order preserved
    if (m_mode == "mostplayed") {
        const qint64 a = sourceModel()->data(l, GameModel::TimeSpentRole).toLongLong();
        const qint64 b = sourceModel()->data(r, GameModel::TimeSpentRole).toLongLong();
        return a > b; // desc; with sort() this yields most-played first
    }
    if (m_mode == "recent") {
        // Recently Added = reverse insertion (file) order, NOT lastPlayed
        // (lastPlayed drives the separate Recently Played section).
        return l.row() > r.row();
    }
    const QString a = sourceModel()->data(l, GameModel::NameRole).toString();
    const QString b = sourceModel()->data(r, GameModel::NameRole).toString();
    return QString::compare(a, b, Qt::CaseInsensitive) < 0;
}

QStringList GameFilterProxy::orderedNames() const {
    QStringList out;
    for (int r = 0; r < rowCount(); ++r)
        out << data(index(r, 0), GameModel::NameRole).toString();
    return out;
}

// ---------------- RecentModel (snapshot) ----------------

RecentModel::RecentModel(QObject *parent) : QAbstractListModel(parent) {}

int RecentModel::rowCount(const QModelIndex &parent) const {
    return parent.isValid() ? 0 : m_games.size();
}

QVariant RecentModel::data(const QModelIndex &index, int role) const {
    if (!index.isValid() || index.row() < 0 || index.row() >= m_games.size())
        return {};
    const GameEntry &g = m_games.at(index.row());
    switch (role) {
    case GameModel::NameRole: return g.name;
    case GameModel::BannerRole: return g.banner;
    case GameModel::IconRole: return g.icon;
    case GameModel::TimeSpentRole: return g.timeSpent;
    case GameModel::LastPlayedRole: return g.lastPlayed;
    case GameModel::FavoriteRole: return g.favorite;
    case Qt::DisplayRole: return g.name;
    default: return {};
    }
}

QHash<int, QByteArray> RecentModel::roleNames() const {
    return {
        {GameModel::NameRole, "name"}, {GameModel::BannerRole, "banner"},
        {GameModel::IconRole, "icon"}, {GameModel::TimeSpentRole, "timeSpent"},
        {GameModel::LastPlayedRole, "lastPlayed"}, {GameModel::FavoriteRole, "favorite"},
    };
}

void RecentModel::setLimit(int n) {
    if (n == m_limit)
        return;
    m_limit = n;
    emit limitChanged();
}

void RecentModel::refresh(GameModel *source) {
    if (!source)
        return;
    struct Scored { GameEntry e; qint64 lp; };
    QVector<Scored> all;
    for (int r = 0; r < source->rowCount(); ++r) {
        const QModelIndex idx = source->index(r, 0);
        const qint64 lp = source->data(idx, GameModel::LastPlayedRole).toLongLong();
        if (lp <= 0)
            continue;
        GameEntry g;
        g.name = source->data(idx, GameModel::NameRole).toString();
        g.banner = source->data(idx, GameModel::BannerRole).toString();
        g.icon = source->data(idx, GameModel::IconRole).toString();
        g.timeSpent = source->data(idx, GameModel::TimeSpentRole).toLongLong();
        g.lastPlayed = lp;
        g.favorite = source->data(idx, GameModel::FavoriteRole).toBool();
        all.append({g, lp});
    }
    std::sort(all.begin(), all.end(),
              [](const Scored &a, const Scored &b) { return a.lp > b.lp; });
    beginResetModel();
    m_games.clear();
    for (int i = 0; i < qMin(m_limit, all.size()); ++i)
        m_games.append(all[i].e);
    endResetModel();
    emit countChanged();
}

#pragma once

#include <QAbstractListModel>
#include <QMap>
#include <QSortFilterProxyModel>
#include <QStringList>
#include <QVector>

class ConfigManager;

/** One game entry (mirrors a Games.ini section). */
struct GameEntry {
    QString name;
    QString executable;
    QString mainPath;
    QString prefixPath;
    QString proton;
    QString overrides;
    QString steamID;
    QString banner;
    QString icon;
    qint64 timeSpent = 0;
    qint64 lastPlayed = 0;
    bool favorite = false;
    QString source;
    QString lutrisRunner;
    QString executor;     // empty = Wine/Proton, or emulator name (e.g. "melonDS")
};

/**
 * GameModel – master game list (never destroyed by filtering).
 * Mirrors Java masterGameList + addGame/importExternalGame/removeGameFromUI.
 */
class GameModel : public QAbstractListModel {
    Q_OBJECT
public:
    enum Roles {
        NameRole = Qt::UserRole + 1,
        ExecutableRole,
        MainPathRole,
        PrefixPathRole,
        ProtonRole,
        SteamIdRole,
        BannerRole,
        IconRole,
        TimeSpentRole,
        LastPlayedRole,
        FavoriteRole,
        SourceRole,
        ExecutorRole,
    };
    Q_ENUM(Roles)

    explicit GameModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    Q_INVOKABLE void reload();
    /** Adds or refreshes a game. Returns false if name is empty. */
    Q_INVOKABLE bool addGame(const QString &name, const QVariantMap &fields);
    Q_INVOKABLE bool importExternalGame(const QString &name, const QVariantMap &fields);
    Q_INVOKABLE void removeGame(const QString &name);
    /** Full removal: INI section + optional prefix dir + game files + art + .desktop. */
    Q_INVOKABLE void removeGameFull(const QString &name, bool removePrefix, bool removeFiles);
    Q_INVOKABLE QVariantMap getGame(const QString &name) const;
    Q_INVOKABLE void setFavorite(const QString &name, bool fav);
    Q_INVOKABLE void touchLastPlayed(const QString &name);

signals:
    void countChanged();

private:
    void loadFromDisk();
    GameEntry entryFromMap(const QString &name, const QMap<QString, QString> &m) const;

    QVector<GameEntry> m_games;
    ConfigManager *m_cfg = nullptr;
};

/**
 * GameFilterProxy – single-selection category filter + live search.
 * Modes: all, favorites, az, mostplayed, recent. Search matches name substring.
 * Mirrors Java applyFilter()/applySearchFilter().
 */
class GameFilterProxy : public QSortFilterProxyModel {
    Q_OBJECT
    Q_PROPERTY(QString mode READ mode WRITE setMode NOTIFY modeChanged)
    Q_PROPERTY(QString search READ search WRITE setSearch NOTIFY searchChanged)
public:
    explicit GameFilterProxy(QObject *parent = nullptr);

    QString mode() const { return m_mode; }
    void setMode(const QString &mode);
    QString search() const { return m_search; }
    void setSearch(const QString &search);

    Q_INVOKABLE QStringList orderedNames() const;

signals:
    void modeChanged();
    void searchChanged();

protected:
    bool filterAcceptsRow(int row, const QModelIndex &parent) const override;
    bool lessThan(const QModelIndex &left, const QModelIndex &right) const override;

private:
    QString m_mode = QStringLiteral("all");
    QString m_search;
};

/** RecentModel – snapshot of top-N by lastPlayed desc (Recently Played). */
class RecentModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int limit READ limit WRITE setLimit NOTIFY limitChanged)
    Q_PROPERTY(int count READ count NOTIFY countChanged)
public:
    explicit RecentModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    int limit() const { return m_limit; }
    void setLimit(int n);
    int count() const { return m_games.size(); }
    Q_INVOKABLE void refresh(GameModel *source);

signals:
    void limitChanged();
    void countChanged();

private:
    int m_limit = 5;
    QVector<GameEntry> m_games;
};

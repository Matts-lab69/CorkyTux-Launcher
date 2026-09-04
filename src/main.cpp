// CorkyTux Qt – entry point. Mirrors Java Launcher.start(): shows Main window,
// no secondary forms auto-opened. Singletons exposed to QML as context props.

#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QStringLiteral>

using namespace Qt::Literals::StringLiterals;

#include "backend/ConfigManager.h"
#include "backend/GameModel.h"
#include "backend/IntegrationManager.h"
#include "backend/PluginManager.h"
#include "backend/ProtonManager.h"
#include "backend/ThemeManager.h"

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    app.setOrganizationName("CorkyTux");
    app.setApplicationName("corkytux");
    app.setApplicationVersion("2.10.0");
    app.setWindowIcon(QIcon(":/CorkyTux/qml/assets/corkytux.png"));

    // Touch singletons (loads INIs, resolves XDG home with root guard).
    ConfigManager *cfg = ConfigManager::instance();
    GameModel *games = new GameModel(&app);
    GameFilterProxy *library = new GameFilterProxy(&app);
    library->setSourceModel(games);
    RecentModel *recent = new RecentModel(&app);
    ThemeManager *theme = ThemeManager::instance();
    ProtonManager *proton = ProtonManager::instance();
    IntegrationManager *integrations = IntegrationManager::instance();
    PluginManager *plugins = PluginManager::instance();
    Q_UNUSED(cfg);

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("config", cfg);
    engine.rootContext()->setContextProperty("games", games);
    engine.rootContext()->setContextProperty("library", library);
    engine.rootContext()->setContextProperty("recent", recent);
    engine.rootContext()->setContextProperty("Theme", theme);
    engine.rootContext()->setContextProperty("proton", proton);
    engine.rootContext()->setContextProperty("integrations", integrations);
    engine.rootContext()->setContextProperty("plugins", plugins);

    // Re-runnable helpers for QML (import flows need MainForm-like access).
    QObject::connect(games, &GameModel::countChanged, recent,
                     [recent, games] { recent->refresh(games); });
    recent->refresh(games);

    engine.load(u"qrc:/CorkyTux/qml/Main.qml"_s);
    if (engine.rootObjects().isEmpty())
        return -1;
    return app.exec();
}

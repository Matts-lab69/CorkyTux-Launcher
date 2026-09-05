import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "components"

// Main – ApplicationWindow 1200x700: top bar, sidebar, center, details overlay,
// modal overlay. Mirrors Java MainForm (StackPane + BorderPane + modals).
ApplicationWindow {
    id: root
    visible: true

    readonly property int sidebarWidth: 320
    readonly property int topBarHeight: 52
    width: 1200
    height: 700
    // Minima por debajo de media pantalla 1080p (960x540) para que el
    // tiling por bordes de Cinnamon (snap) pueda encajar la ventana.
    minimumWidth: 860
    minimumHeight: 500
    title: "CorkyTux"
    color: Theme.bg

    property var currentGame: ({})
    property string currentGameName: ""

    function openGame(name) {
        var g = games.getGame(name);
        if (!g.prefixPath)
            g.prefixPath = proton.prefixPath(name);
        root.currentGame = g;
        root.currentGameName = name;
        details.game = root.currentGame;
        details.gameRunning = proton.running && proton.currentGame === name;
        details.show();
        var mp = root.currentGame.mainPath || "";
        var ex = root.currentGame.executable || "";
        var dir = mp;
        if (!dir && ex)
            dir = ex.substring(0, ex.lastIndexOf("/"));
        if (dir)
            proton.queryFolderSize(dir);
    }
    function refreshAll() {
        recent.refresh(games);
        if (root.currentGameName !== "")
            root.currentGame = games.getGame(root.currentGameName);
    }

    Connections {
        target: proton
        function onRunningChanged() {
            details.gameRunning = proton.running && proton.currentGame === root.currentGameName;
        }
        function onGameLogOutput(text) {
            logModal.logText += text;
        }
        function onToast(msg) { toastLabel.text = msg; toastPopup.open(); }
        function onFolderSizeReady(path, bytes) {
            var txt = bytes > 1073741824 ? (bytes / 1073741824).toFixed(1) + " GB"
                                         : Math.round(bytes / 1048576) + " MB";
            var g = root.currentGame;
            g.sizeText = txt;
            root.currentGame = g;
            details.game = g;
        }
    }
    Connections {
        target: games
        function onCountChanged() { refreshAll(); }
    }
    Connections {
        target: plugins
        function onScanApplied(name, steamId) {
            // Plugin scan may have discovered the Steam AppID: run artwork
            // again so Steam CDN/Store covers get fetched for it.
            if (steamId !== "")
                integrations.resolveArtwork(name, steamId);
            refreshAll();
        }
    }
    Connections {
        target: integrations
        function onArtworkReady(name, banner, icon) {
            var fields = {};
            if (banner)
                fields.banner = banner;
            if (icon) {
                // Never clobber a real exe-extracted icon with cover art
                var cur = (games.getGame(name).icon || "").toString();
                if (cur.indexOf("-exe.png") < 0)
                    fields.icon = icon;
            }
            if (Object.keys(fields).length > 0)
                games.addGame(name, fields);
        }
        function onPluginResult(result) {
            if (result && result.ok && result.installed) {
                toastLabel.text = "Dependencies installed: " + (result.installed || []).join(", ");
                toastPopup.open();
            }
        }
        function onExeIconReady(name, path) {
            // Real exe icon beats cover-as-icon fallbacks (lutris banners,
            // capsules, store images); keeps hicolor/SGDB/exe icons.
            var cur = (games.getGame(name).icon || "").toString();
            var looksCover = cur === "" || cur.indexOf("/coverart/") >= 0
                || cur.indexOf("/banners/") >= 0
                || /capsule|store\.jpg$|-lutris\.jpg$/.test(cur);
            if (looksCover)
                games.addGame(name, { "icon": path });
        }
        function onLutrisScanReady(scanGames) {
            var imported = 0;
            for (var i = 0; i < scanGames.length; i++) {
                var g = scanGames[i];
                if (!g.name || /^\d+$/.test(g.name))
                    continue;
                if (games.importExternalGame(g.name, {
                        "lutrisRunner": g.runner,
                        "mainPath": g.directory,
                        "executable": g.executable || g.directory,
                        "prefixPath": g.prefix || "",
                        "timeSpent": g.playtimeHours > 0 ? Math.round(g.playtimeHours * 3600) : "",
                        "source": "lutris"
                    })) {
                    imported++;
                    if (g.directory)
                        plugins.applyScanPlugins(g.name, g.directory);
                }
            }
            toastLabel.text = "Lutris scan: " + scanGames.length + " found, " + imported + " new.";
            toastPopup.open();
        }
        function onSteamScanReady(scanGames) {
            var imported = 0;
            for (var i = 0; i < scanGames.length; i++) {
                var g = scanGames[i];
                if (!g.name || /^\d+$/.test(g.name))
                    continue;
                if (games.importExternalGame(g.name, {
                        "mainPath": g.libraryPath,
                        "executable": g.executable || "",
                        "prefixPath": g.prefixPath || "",
                        "steamID": g.appId,
                        "source": "steam"
                    })) {
                    imported++;
                    if (g.libraryPath)
                        integrations.resolveArtwork(g.name, g.appId);
                }
            }
            toastLabel.text = "Steam scan: " + scanGames.length + " found, " + imported + " new.";
            toastPopup.open();
        }
    }

    Column {
        anchors.fill: parent
        // ---- top bar ----
        Rectangle {
            width: parent.width
            height: root.topBarHeight
            color: Theme.bg
            RowLayout {
                id: topRow
                anchors.fill: parent
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                spacing: 12
                Image {
                    source: "qrc:/assets/qml/assets/corkytux.png"
                    sourceSize.width: 32
                    sourceSize.height: 32
                    Layout.preferredWidth: 32
                    Layout.preferredHeight: 32
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    Layout.alignment: Qt.AlignVCenter
                }
                Text {
                    text: "CorkyTux"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 16
                    Layout.alignment: Qt.AlignVCenter
                }
                CButton {
                    text: "＋  Add game"
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: addModal.open()
                }
                Item { Layout.fillWidth: true; Layout.fillHeight: true }
                CButton {
                    text: "Settings"
                    kind: "outline"
                    iconSource: "settings-hires"
                    iconSize: 18
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: settingsModal.open()
                }
            }
        }
        // ---- body ----
        Row {
            width: parent.width
            height: parent.height - root.topBarHeight
            Sidebar {
                id: sidebar
                width: root.sidebarWidth
                height: parent.height
                onGameClicked: function(name) { root.openGame(name); }
            }
            // ---- center ----
            Flickable {
                width: parent.width - root.sidebarWidth
                height: parent.height
                contentHeight: centerCol.height + 48
                clip: true
                Column {
                    id: centerCol
                    width: parent.width - 48
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 24
                    spacing: 24
                    Column {
                        width: parent.width
                        spacing: 12
                        Text {
                            text: "RECENTLY PLAYED"
                            color: Theme.textMain
                            font.bold: true
                            font.pixelSize: 24
                            horizontalAlignment: Text.AlignHCenter
                            width: parent.width
                        }
                        Rectangle {
                            width: parent.width
                            height: 1
                            color: Theme.border
                        }
                        Text {
                            text: recent.count > 0 ? "" : "Play a game and it will show up here"
                            color: Theme.textSec
                            font.pixelSize: 12
                            horizontalAlignment: Text.AlignHCenter
                            width: parent.width
                            visible: recent.count === 0
                        }
                        Flow {
                            id: recentFlow
                            width: parent.width
                            spacing: 16
                            Repeater {
                                model: recent
                                GameCard {
                                    name: model.name || ""
                                    banner: model.banner || ""
                                    icon: model.icon || ""
                                    onClicked: root.openGame(model.name)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Capacity from the SCREEN resolution (not the window): how many 224px
    // cards fit fullscreen. Extra oldest entries drop out of view (data kept).
    function updateRecentLimit() {
        var scrW = Screen.width || 1920;
        var scrH = Screen.height || 1080;
        var availW = Math.max(240, scrW - root.sidebarWidth - 48);
        var cols = Math.max(1, Math.floor((availW + 16) / 240));
        var availH = Math.max(200, scrH - root.topBarHeight - 140);
        var rows = Math.max(1, Math.floor((availH + 16) / 156));
        recent.limit = Math.min(30, Math.max(1, cols * rows));
    }
    onWidthChanged: updateRecentLimit()
    onHeightChanged: updateRecentLimit()

    // ---- details overlay (right, floats over content) ----
    DetailsPanel {
        id: details
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: 60
        anchors.rightMargin: 12
        anchors.bottomMargin: 12
        onPlayClicked: proton.runGame(root.currentGameName)
        onStopClicked: proton.stopGame()
        onFavoriteToggled: {
            games.setFavorite(root.currentGameName, !(root.currentGame.favorite || false));
            root.currentGame = games.getGame(root.currentGameName);
            details.game = root.currentGame;
        }
        onClosed: details.hide()
        onActionClicked: function(action) {
            var sid = (root.currentGame.steamID || "").toString();
            if (action === "Settings") gameSettingsModal.openFor(root.currentGameName);
            else if (action === "Remove") removeModal.openFor(root.currentGameName, !!(root.currentGame.executor || ""));
            else if (action === "Debug") {
                logModal.logText = "";
                logModal.open();
                proton.runGameDebug(root.currentGameName);
            }
            else if (action === "Wine") wineMenu.open();
            else if (action === "Run exe") proton.runGame(root.currentGameName);
            else if (action === "Folders") {
                if (root.currentGame.mainPath)
                    Qt.openUrlExternally("file://" + root.currentGame.mainPath);
            } else if (action === "SteamDB" && sid) Qt.openUrlExternally("https://steamdb.info/app/" + sid + "/");
            else if (action === "ProtonDB" && sid) Qt.openUrlExternally("https://protondb.com/app/" + sid);
            else if (action === "Steam" && sid) Qt.openUrlExternally("steam://store/" + sid);
        }
    }

    Menu {
        id: wineMenu
        MenuItem {
            text: "winecfg"
            onTriggered: proton.runWineTool(root.currentGameName, "winecfg")
        }
        MenuItem {
            text: "taskmgr"
            onTriggered: proton.runWineTool(root.currentGameName, "taskmgr")
        }
        MenuItem {
            text: "control"
            onTriggered: proton.runWineTool(root.currentGameName, "control")
        }
        MenuItem {
            text: "explorer"
            onTriggered: proton.runWineTool(root.currentGameName, "explorer")
        }
        MenuItem {
            text: "cmd"
            onTriggered: proton.runWineTool(root.currentGameName, "cmd")
        }
    }

    // ---- modal overlay host ----
    AddGameModal { id: addModal; objectName: "addModal" }
    SettingsModal {
        id: settingsModal
        onNotice: function(msg) { toastLabel.text = msg; toastPopup.open(); }
        onOpenProton: protonModal.open()
    }
    GameSettingsModal { id: gameSettingsModal }
    RemoveModal {
        id: removeModal
        onRemoved: function(name) {
            if (root.currentGameName === name) {
                root.currentGameName = "";
                root.currentGame = {};
            }
            details.hide();
        }
    }
    LogModal { id: logModal }
    ProtonModal { id: protonModal }

    Popup {
        id: toastPopup
        anchors.centerIn: Overlay.overlay
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        background: Rectangle { color: Theme.isLight ? "#212529" : "#282828"; radius: 8 }
        contentItem: Text { id: toastLabel; color: "#FFFFFF"; font.pixelSize: 13; padding: 12 }
    }

    Component.onCompleted: {
        updateRecentLimit();
        refreshAll();
    }
}

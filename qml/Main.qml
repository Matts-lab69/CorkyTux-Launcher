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
        if (root.currentGameName !== "") {
            root.currentGame = games.getGame(root.currentGameName);
            details.game = root.currentGame;
        }
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
            var runnerMap = {
                "mupen64plus": "Mupen64Plus",
                "pcsx2": "PCSX2",
                "ppsspp": "PPSSPP",
                "rpcs3": "RPCS3",
                "ryujinx": "Ryujinx",
                "dolphin": "Dolphin",
                "cemu": "Cemu",
                "melonds": "melonDS",
                "desmume": "Desmume",
                "duckstation": "DuckStation",
                "vita3k": "Vita3K",
                "azahar": "Azahar"
            };
            for (var i = 0; i < scanGames.length; i++) {
                var g = scanGames[i];
                if (!g.name || /^\d+$/.test(g.name))
                    continue;
                // Skip games already in Steam library
                var existing = games.getGame(g.name);
                if (existing && existing.source === "steam")
                    continue;
                var executor = runnerMap[(g.runner || "").toLowerCase()] || "";
                var mainDir = g.directory || "";
                var exePath = g.executable || "";
                if (!mainDir && exePath)
                    mainDir = exePath.substring(0, exePath.lastIndexOf("/"));
                else if (mainDir && !integrations.pathExists(mainDir) && exePath)
                    mainDir = exePath.substring(0, exePath.lastIndexOf("/"));
                var fields = {
                    "lutrisSlug": g.slug || "",
                    "lutrisRunner": g.runner,
                    "mainPath": mainDir,
                    "executable": exePath || mainDir,
                    "prefixPath": g.prefix || "",
                    "timeSpent": g.playtimeHours > 0 ? Math.round(g.playtimeHours * 3600) : "",
                    "source": "lutris"
                };
                if (executor)
                    fields["executor"] = executor;
                games.importExternalGame(g.name, fields);
                imported++;
                if (g.directory)
                    plugins.applyScanPlugins(g.name, g.directory);
                var gData = games.getGame(g.name);
                if (gData && ((!gData.banner || gData.banner === "")
                              || (!gData.icon || gData.icon === "")))
                    integrations.resolveArtwork(g.name, g.steamID || "");
            }
            toastLabel.text = "Lutris scan: " + scanGames.length + " found, " + imported + " added.";
            toastPopup.open();
        }
        function onSteamScanReady(scanGames) {
            var imported = 0;
            for (var i = 0; i < scanGames.length; i++) {
                var g = scanGames[i];
                if (!g.name || /^\d+$/.test(g.name))
                    continue;
                games.importExternalGame(g.name, {
                        "mainPath": g.libraryPath,
                        "executable": g.executable || "",
                        "prefixPath": g.prefixPath || "",
                        "steamID": g.appId,
                        "source": "steam"
                    });
                imported++;
                // Resolve artwork for all Steam games (new and existing)
                if (g.libraryPath)
                    integrations.resolveArtwork(g.name, g.appId);
            }
            toastLabel.text = "Steam scan: " + scanGames.length + " found, " + imported + " games updated.";
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
                    Rectangle {
                        id: recentFrame
                        width: parent.width
                        height: Math.max(360, root.height - root.topBarHeight - 48)
                        radius: 10
                        color: "transparent"
                        border.color: Theme.accent
                        border.width: 1
                        Column {
                            id: recentContent
                            anchors.top: parent.top
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.topMargin: 14
                            width: parent.width - 28
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
                                width: {
                                    var columns = Math.max(1, Math.min(recent.count || 1,
                                        Math.floor((parent.width + 16) / 240)));
                                    return Math.min(parent.width, columns * 224 + (columns - 1) * 16);
                                }
                                anchors.horizontalCenter: parent.horizontalCenter
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
            else if (action === "Run exe") {
                var g = games.getGame(root.currentGameName);
                var dir = g ? (g.mainPath || "") : "";
                if (!dir) { toastLabel.text = "No game directory"; toastPopup.open(); return; }
                var all = integrations.scanDirSync(dir);
                var exes = [];
                for (var i = 0; i < all.length; i++) {
                    var lower = all[i].toLowerCase();
                    if (lower.endsWith(".exe") || lower.endsWith(".bat") || lower.endsWith(".msi"))
                        exes.push(all[i]);
                }
                root.runExeCandidates = exes;
                runExeModal.open();
            }
            else if (action === "Folders") {
                if (root.currentGame.mainPath)
                    Qt.openUrlExternally(Qt.resolvedUrl("file://" + root.currentGame.mainPath));
            } else if (action === "SteamDB") {
                if (sid) Qt.openUrlExternally("https://steamdb.info/app/" + sid + "/");
                else Qt.openUrlExternally("https://steamdb.info/search/?q=" + encodeURIComponent(root.currentGameName));
            } else if (action === "ProtonDB") {
                if (sid) Qt.openUrlExternally("https://www.protondb.com/app/" + sid);
                else Qt.openUrlExternally("https://www.protondb.com/search?q=" + encodeURIComponent(root.currentGameName));
            } else if (action === "Steam") {
                if (sid) Qt.openUrlExternally("steam://store/" + sid);
                else Qt.openUrlExternally("https://store.steampowered.com/search/?term=" + encodeURIComponent(root.currentGameName));
            }
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

    // ---- Run Exe picker (scans game dir for .exe files) ----
    property var runExeCandidates: []
    CModal {
        id: runExeModal
        title: "Run Executable"
        boxWidth: 440
        Column {
            width: parent.width
            spacing: 8
            Text {
                text: "Select an executable from the game directory"
                color: Theme.textSec
                font.pixelSize: 12
                wrapMode: Text.Wrap
                width: parent.width
            }
            Repeater {
                model: root.runExeCandidates
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    width: parent.width
                    height: 44
                    radius: 6
                    color: exeMouse.containsMouse ? Theme.hover : Theme.well
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 8
                        Text {
                            text: modelData.split("/").pop()
                            color: Theme.textMain
                            font.bold: true
                            font.pixelSize: 13
                            elide: Text.ElideMiddle
                            width: parent.width - 24
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                    MouseArea {
                        id: exeMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            runExeModal.close();
                            proton.runCustomExe(root.currentGameName, modelData);
                        }
                    }
                }
            }
            Text {
                text: root.runExeCandidates.length === 0 ? "No executables found in game directory" : ""
                color: Theme.textSec
                font.pixelSize: 12
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
            }
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
    Timer {
        id: emuReloadTimer
        interval: 300
        repeat: false
        onTriggered: {
            gameSettingsModal.reloadEmuFields();
            // Retry if emulator settings still empty (listEmulators may still be running)
            if (gameSettingsModal.isEmulatorGame && !gameSettingsModal.hasEmuSettings
                    && plugins.emulators.length === 0) {
                retryCount++;
                if (retryCount < 10) emuReloadTimer.start();
            } else {
                retryCount = 0;
            }
        }
    }
    property int retryCount: 0
    Connections {
        target: plugins
        function onEmulatorsChanged() {
            if (gameSettingsModal.visible)
                emuReloadTimer.start();
        }
    }
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

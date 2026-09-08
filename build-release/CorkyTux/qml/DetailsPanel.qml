import QtQuick
import QtQuick.Controls
import "components"

// DetailsPanel – floating right overlay: title + star, cover, Play, time,
// size/paths, 3x3 actions. Mirrors Java gamePanel (StackPane overlay).
Rectangle {
    id: root
    property var game: ({})
    property bool gameRunning: false
    signal playClicked
    signal stopClicked
    signal actionClicked(string action)
    signal favoriteToggled
    signal closed

    width: 380
    color: Theme.panel
    radius: 8
    border.color: Theme.accent
    border.width: 1
    visible: false

    // slide+fade entry (~220ms, GPU transform => 60fps)
    NumberAnimation on x { id: slideAnim; duration: 220; easing.type: Easing.OutCubic }
    NumberAnimation on opacity { id: fadeAnim; duration: 220 }

    function show() {
        slideAnim.stop();
        fadeAnim.stop();
        x = 40;
        opacity = 0;
        visible = true;
        slideAnim.to = 0;
        fadeAnim.to = 1;
        slideAnim.start();
        fadeAnim.start();
    }
    function hide() {
        slideAnim.stop();
        fadeAnim.stop();
        visible = false; // instant hide (no laggy fade over heavy subtree)
        opacity = 1;
        x = 0;
    }

    Flickable {
        anchors.fill: parent
        anchors.margins: 16
        contentHeight: contentCol.height
        clip: true
        Column {
            id: contentCol
            width: parent.width - 8
            spacing: 16

            Row {
                width: parent.width
                spacing: 8
                Text {
                    text: game.name || "Game Title"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 16
                    width: parent.width - 88
                    elide: Text.ElideRight
                }
                CButton {
                    text: ""
                    iconSource: game.favorite ? "star_gold" : "star_gray"
                    iconSize: 22
                    iconThemed: false
                    iconDy: -4
                    kind: "icon"
                    width: 36
                    height: 36
                    onClicked: root.favoriteToggled()
                }
                CButton {
                    text: "✕"
                    kind: "accenticon"
                    width: 36
                    height: 36
                    fontSize: 18
                    onClicked: root.closed()
                }
            }

            Rectangle {
                width: parent.width
                height: 200
                radius: 8
                color: Theme.isLight ? "#E9ECEF" : "#282828"
                Image {
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 8, 340)
                    height: 192
                    source: game.banner ? ("file://" + game.banner) : ""
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    asynchronous: true
                    sourceSize.width: 460
                    sourceSize.height: 215
                }
            }

            CButton {
                text: gameRunning ? "Stop" : "Play"
                iconSource: gameRunning ? "stop" : "play"
                iconThemed: false
                iconSize: 16
                width: parent.width
                height: 44
                onClicked: gameRunning ? root.stopClicked() : root.playClicked()
            }

            Text {
                text: {
                    var s = Number(game.timeSpent || 0);
                    if (s < 3600) {
                        var m = Math.max(0, Math.round(s / 60));
                        return "Time played: " + m + " min";
                    }
                    return "Time played: " + Math.round(s / 3600) + " h";
                }
                color: Theme.textSec
                font.pixelSize: 12
            }

            CCard {
                width: parent.width
                corner: 6
                outlineColor: Theme.border
                outlineWidth: 1
                Row {
                    width: parent.width
                    spacing: 8
                    CIcon { iconName: "folder"; iconSize: 18; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: "Install Info"; color: Theme.textMain; font.bold: true; font.pixelSize: 14 }
                }
                Rectangle { width: parent.width; height: 1; color: Theme.border }
                    Text { text: "SIZE"; color: Theme.textMuted; font.pixelSize: 11; font.bold: true }
                    Text { text: game.sizeText || "--"; color: Theme.textMain; font.pixelSize: 13 }
                    Text { text: "INSTALL PATH"; color: Theme.textMuted; font.pixelSize: 11; font.bold: true }
                    Text { text: game.mainPath || "--"; color: Theme.textMain; font.pixelSize: 13; wrapMode: Text.WrapAnywhere; width: parent.width }
                    Text { text: "PREFIX PATH"; color: Theme.textMuted; font.pixelSize: 11; font.bold: true; visible: !(game.executor || "") }
                    Text { text: game.prefixPath || "--"; color: Theme.textMain; font.pixelSize: 13; wrapMode: Text.WrapAnywhere; width: parent.width; visible: !(game.executor || "") }
                    Text { text: "EMULATOR"; color: Theme.textMuted; font.pixelSize: 11; font.bold: true; visible: !!(game.executor || "") }
                    Text { text: game.executor || "--"; color: Theme.textMain; font.pixelSize: 13; visible: !!(game.executor || "") }
            }

            CCard {
                width: parent.width
                outlineColor: Theme.accent
                outlineWidth: 1
                Text { text: "Actions"; color: Theme.textMain; font.bold: true; font.pixelSize: 14 }
                Grid {
                    width: parent.width
                    columns: 3
                    spacing: 4
                    Repeater {
                        model: [
                            { "label": "Settings", "icon": "settings", "emulator": false },
                            { "label": "Debug", "icon": "debug", "emulator": false },
                            { "label": "Remove", "icon": "remove", "emulator": false },
                            { "label": "Wine", "icon": "wine", "emulator": true },
                            { "label": "Run exe", "icon": "run", "emulator": true },
                            { "label": "Folders", "icon": "folder", "emulator": false },
                            { "label": "SteamDB", "icon": "db", "emulator": true },
                            { "label": "ProtonDB", "icon": "protondb", "emulator": true },
                            { "label": "Steam", "icon": "steam", "emulator": true }
                        ]
                        CButton {
                            required property var modelData
                            text: modelData.label
                            iconSource: modelData.icon
                            kind: "action"
                            width: (parent.width - 8) / 3
                            height: 36
                            visible: modelData.emulator ? !(root.game.executor || "") : true
                            onClicked: root.actionClicked(modelData.label)
                        }
                    }
                }
            }
        }
        ScrollBar.vertical: ScrollBar {
            policy: ScrollBar.AsNeeded
            contentItem: Rectangle { implicitWidth: 3; radius: 2; color: Theme.accent }
            background: Rectangle { implicitWidth: 3; color: "transparent" }
        }
    }
}

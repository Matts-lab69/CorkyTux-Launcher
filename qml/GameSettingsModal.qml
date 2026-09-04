import QtQuick
import QtQuick.Controls
import "components"

// GameSettingsModal – per-game Proton/paths/overrides/env/args/flags.
// Mirrors Java GameSettings (View/Run/Graphics tab pill).
CModal {
    id: root
    title: "Game Settings"
    boxWidth: 640

    property string gameName: ""
    property string tab: "view"

    function openFor(name) {
        gameName = name;
        loadFields();
        globalWined3d.setSilent(config.launcherValue("gamesUsesWined3d", "User Settings") === "1");
        globalWayland.setSilent(config.launcherValue("gamesUsesWayland", "User Settings") === "1");
        open();
    }
    function loadFields() {
        var g = games.getGame(gameName);
        nameField.text = g.name || gameName;
        protonBox.model = ["GE-Proton Latest"].concat(proton.installedProtons());
        protonBox.currentIndex = Math.max(0, protonBox.find(g.proton || "GE-Proton Latest"));
        prefixField.text = g.prefixPath || "";
        overridesField.text = g.overrides || "";
        envField.text = g.environment || "";
        argsBeforeField.text = g.argsBefore || "";
        argsAfterField.text = g.argsAfter || "";
        overlaySwitch.setSilent(g.steamOverlay === "true" || g.steamOverlay === "1");
        runtimeSwitch.setSilent(g.steamRuntime === "true" || g.steamRuntime === "1");
        wined3dSwitch.setSilent(g.wined3d === "true" || g.wined3d === "1");
        waylandSwitch.setSilent(g.nativeWayland === "true" || g.nativeWayland === "1");
    }
    function save() {
        games.addGame(gameName, {
            "proton": protonBox.currentText,
            "prefixPath": prefixField.text.trim(),
            "overrides": overridesField.text.trim(),
            "environment": envField.text.trim(),
            "argsBefore": argsBeforeField.text.trim(),
            "argsAfter": argsAfterField.text.trim()
        });
    }

    Column {
        width: parent.width
        spacing: 12
        // View tab
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "view"
            Text { text: "Game name in launcher"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: nameField; width: parent.width; onEditingFinished: save() }
            Text { text: "Install path"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { width: parent.width; readOnly: true; text: games.getGame(root.gameName).mainPath || "" }
            Text { text: "Proton version"; color: Theme.textSec; font.pixelSize: 12 }
            CComboBox {
                id: protonBox
                width: parent.width
                editable: true
                onActivated: save()
                onAccepted: save()
            }
            Text { text: "Prefix path"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: prefixField; width: parent.width; onEditingFinished: save() }
        }
        // Run tab
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "run"
            Text { text: "DLL overrides (WINEDLLOVERRIDES)"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: overridesField; width: parent.width; onEditingFinished: save() }
            Text { text: "Environment variables"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: envField; width: parent.width; onEditingFinished: save() }
            Text { text: "Arguments before executable"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: argsBeforeField; width: parent.width; onEditingFinished: save() }
            Text { text: "Arguments after executable"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: argsAfterField; width: parent.width; onEditingFinished: save() }
            Row {
                spacing: 16
                CSwitch {
                    id: overlaySwitch
                    objectName: "Steam overlay"
                    onToggled: games.addGame(root.gameName, {"steamOverlay": checked ? "true" : "false"})
                }
                CSwitch {
                    id: runtimeSwitch
                    objectName: "Steam runtime"
                    onToggled: games.addGame(root.gameName, {"steamRuntime": checked ? "true" : "false"})
                }
            }
            Row {
                spacing: 16
                CSwitch {
                    id: wined3dSwitch
                    objectName: "Use wined3d"
                    onToggled: games.addGame(root.gameName, {"wined3d": checked ? "true" : "false"})
                }
                CSwitch {
                    id: waylandSwitch
                    objectName: "Prefer Wayland"
                    onToggled: games.addGame(root.gameName, {"nativeWayland": checked ? "true" : "false"})
                }
            }
        }
        // Graphics tab: GLOBAL defaults for new games (mirrors Java
        // gamesUsesWined3d/gamesUsesWayland). Per-game override stays manual.
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "graphics"
            Text {
                text: "Defaults for new games (applies automatically on add)."
                color: Theme.textSec
                font.pixelSize: 12
                wrapMode: Text.Wrap
                width: parent.width
            }
            Text {
                text: "Per-game override stays manual in each game's own settings."
                color: Theme.textMuted
                font.pixelSize: 11
                wrapMode: Text.Wrap
                width: parent.width
            }
            CSwitch {
                id: globalWined3d
                objectName: "Use wined3d for new games"
                Component.onCompleted: setSilent(config.launcherValue("gamesUsesWined3d", "User Settings") === "1")
                onToggled: config.setLauncherValue("gamesUsesWined3d", checked ? "1" : "0", "User Settings")
            }
            CSwitch {
                id: globalWayland
                objectName: "Prefer Wayland for new games"
                Component.onCompleted: setSilent(config.launcherValue("gamesUsesWayland", "User Settings") === "1")
                onToggled: config.setLauncherValue("gamesUsesWayland", checked ? "1" : "0", "User Settings")
            }
        }
        // bottom tab pill (centered, wraps content tightly)
        Rectangle {
            width: tabRow.implicitWidth + 24
            height: 62
            radius: 14
            color: Theme.isLight ? "#FFFFFF" : "#181818"
            border.color: Theme.border
            border.width: 1
            anchors.horizontalCenter: parent.horizontalCenter
        Row {
            id: tabRow
            anchors.centerIn: parent
            spacing: 10
            Repeater {
                model: [
                    { "id": "view", "label": "View", "icon": "palette" },
                    { "id": "run", "label": "Run", "icon": "startup" },
                    { "id": "graphics", "label": "Graphics", "icon": "graphics" }
                ]
                Button {
                    required property var modelData
                    text: modelData.label
                    checkable: true
                    checked: root.tab === modelData.id
                    autoExclusive: true
                    background: Rectangle {
                        radius: 12
                        color: parent.checked ? Theme.hover : "transparent"
                    }
                    contentItem: Column {
                        spacing: 2
                        CIcon {
                            iconName: modelData.icon
                            iconSize: 20
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                        Text {
                            text: modelData.label
                            color: root.tab === modelData.id ? Theme.accentText : Theme.textSec
                            font.bold: true
                            font.pixelSize: 11
                            horizontalAlignment: Text.AlignHCenter
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                        Rectangle {
                            width: 24
                            height: 3
                            radius: 2
                            color: Theme.accent
                            anchors.horizontalCenter: parent.horizontalCenter
                            visible: root.tab === modelData.id
                        }
                    }
                    onClicked: root.tab = modelData.id
                }
            }
        }
        }
    }
}

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

            // ---- Dependency Installer (only if plugin installed) ----
            property var depScanResults: []
            property string depMessage: ""
            property string depMode: "scan"  // "scan" or "install"
            property int depTotal: 0
            property int depCompleted: 0

            Column {
                id: depSection
                width: parent.width
                spacing: 8
                visible: {
                    for (var i = 0; i < plugins.plugins.length; i++) {
                        if (plugins.plugins[i].id === "dependency-installer" && plugins.plugins[i].enabled)
                            return true;
                    }
                    return false;
                }

                function getPluginPath() {
                    for (var i = 0; i < plugins.plugins.length; i++) {
                        if (plugins.plugins[i].id === "dependency-installer" && plugins.plugins[i].enabled)
                            return plugins.plugins[i].path || "";
                    }
                    return "";
                }

                Rectangle {
                    width: parent.width
                    height: 1
                    color: Theme.border
                    anchors.topMargin: 4
                    anchors.bottomMargin: 4
                }

                Text {
                    text: "Plugin: Dependency Installer"
                    color: Theme.accent
                    font.bold: true
                    font.pixelSize: 13
                }
                Text {
                    text: "Auto-detect missing Windows components (VC++, DirectX, .NET)"
                    color: Theme.textSec
                    font.pixelSize: 11
                    wrapMode: Text.Wrap
                    width: parent.width
                }
                CButton {
                    text: "Scan Game"
                    width: parent.width
                    height: 32
                    onClicked: {
                        var g = games.getGame(root.gameName);
                        if (!g || !g.mainPath) {
                            depSection.parent.depMessage = "No game directory found";
                            depSection.parent.depScanResults = [];
                            return;
                        }
                        var plugPath = depSection.getPluginPath();
                        if (!plugPath) {
                            depSection.parent.depMessage = "Plugin not found or not enabled";
                            depSection.parent.depScanResults = [];
                            return;
                        }
                        depSection.parent.depMode = "scan";
                        depSection.parent.depMessage = "Scanning " + g.mainPath + "...";
                        depSection.parent.depScanResults = [];
                        var scanArgs = ["scan", g.mainPath];
                        if (g.prefixPath) scanArgs = scanArgs.concat(["--prefix", g.prefixPath]);
                        integrations.runPlugin(plugPath, scanArgs);
                    }
                }
                Text {
                    text: depSection.parent.depMessage
                    color: depSection.parent.depScanResults.length > 0 ? Theme.textSec : Theme.accent
                    font.pixelSize: 12
                    wrapMode: Text.Wrap
                    width: parent.width
                    visible: text !== ""
                }
                // Progress bar (visible during install)
                Rectangle {
                    width: parent.width
                    height: 6
                    radius: 3
                    color: Theme.border
                    visible: depSection.parent.depMode === "install" && depSection.parent.depTotal > 0
                    Rectangle {
                        width: parent.width * (depSection.parent.depTotal > 0 ? depSection.parent.depCompleted / depSection.parent.depTotal : 0)
                        height: parent.height
                        radius: 3
                        color: Theme.accent
                        Behavior on width { NumberAnimation { duration: 300 } }
                    }
                }
                Repeater {
                    id: depList
                    model: depSection.parent.depScanResults
                    delegate: Row {
                        property string depId: modelData.id || ""
                        property alias depCheck: checkBox
                        property string depStatus: modelData.status || (modelData.confidence === "installed" ? "installed" : "")
                        spacing: 8
                        width: parent.width

                        // Status icon
                        Text {
                            text: depStatus === "done" || depStatus === "installed" ? "\u2714"
                                : depStatus === "error" ? "\u2718"
                                : depStatus === "installing" ? "\u25B6" : ""
                            color: depStatus === "done" || depStatus === "installed" ? "#4caf50"
                                : depStatus === "error" ? "#f44336" : Theme.accent
                            font.pixelSize: 14
                            font.bold: true
                            visible: depStatus !== ""
                            width: 18
                            horizontalAlignment: Text.AlignHCenter
                        }

                        // Checkbox (hidden for installed items or during install)
                        CSwitch {
                            id: checkBox
                            width: 40
                            visible: depStatus === "" && depSection.parent.depMode === "scan"
                            Component.onCompleted: setSilent(false)
                        }

                        Column {
                            width: parent.width - (depStatus !== "" || depSection.parent.depMode === "install" ? 26 : 50)
                            Text {
                                text: modelData.desc || modelData.id || "?"
                                color: Theme.textMain
                                font.pixelSize: 12
                            }
                            Text {
                                text: modelData.confidence || (depStatus === "error" ? (modelData.error || "Failed") : "")
                                color: modelData.confidence === "recommended" ? "#ff6b6b" : Theme.textSec
                                font.pixelSize: 10
                                elide: Text.ElideRight
                                width: parent.width
                            }
                        }
                    }
                }
                CButton {
                    text: "Install Selected"
                    width: parent.width
                    height: 32
                    visible: {
                        if (depSection.parent.depMode !== "scan") return false;
                        for (var i = 0; i < depList.count; i++) {
                            var item = depList.itemAt(i);
                            if (item && item.depStatus === "" && item.depCheck.visible) return true;
                        }
                        return false;
                    }
                    onClicked: {
                        var deps = [];
                        for (var i = 0; i < depList.count; i++) {
                            var item = depList.itemAt(i);
                            if (item && item.depCheck.checked && item.depStatus !== "installed")
                                deps.push(item.depId);
                        }
                        if (deps.length === 0) return;
                        var plugPath = depSection.getPluginPath();
                        if (!plugPath) { depSection.parent.depMessage = "Plugin not found"; return; }
                        var g = games.getGame(root.gameName);
                        if (!g) { depSection.parent.depMessage = "Game not found"; return; }
                        var prefix = g.prefixPath || "";
                        if (!prefix) { depSection.parent.depMessage = "No prefix found"; return; }
                        var protonName = g.proton || "";
                        var protonFullPath = "";
                        if (protonName) {
                            var details = proton.installedProtonDetails();
                            for (var j = 0; j < details.length; j++) {
                                if (details[j].name === protonName) {
                                    protonFullPath = details[j].path + "/" + protonName;
                                    break;
                                }
                            }
                        }
                        var installArgs = ["install", "--prefix", prefix];
                        if (protonFullPath) installArgs.push("--proton", protonFullPath);
                        installArgs = installArgs.concat(deps);
                        depSection.parent.depMode = "install";
                        depSection.parent.depTotal = deps.length;
                        depSection.parent.depCompleted = 0;
                        depSection.parent.depMessage = "Installing " + deps.length + " dependencies...";
                        depSection.parent.depScanResults = [];
                        integrations.runPlugin(plugPath, installArgs);
                    }
                }
            }

            Connections {
                target: integrations
                function onPluginProgress(progress) {
                    console.log("[DepInstaller] progress:", JSON.stringify(progress))
                    // Update or add item in the results list
                    var results = depSection.parent.depScanResults.slice();
                    var found = false;
                    for (var i = 0; i < results.length; i++) {
                        if (results[i].id === progress.id) {
                            results[i] = progress;
                            found = true;
                            break;
                        }
                    }
                    if (!found) results.push(progress);
                    depSection.parent.depScanResults = results;
                    depSection.parent.depMessage = "Installing " + progress.desc + "...";
                    // Update progress counter
                    if (progress.status === "done" || progress.status === "error") {
                        depSection.parent.depCompleted = depSection.parent.depCompleted + 1;
                    }
                }
                function onPluginResult(result) {
                    console.log("[DepInstaller] result:", JSON.stringify(result))
                    if (result && result.type === "done") {
                        var installed = result.installed || [];
                        var failed = result.failed || [];
                        depSection.parent.depCompleted = depSection.parent.depTotal;
                        depSection.parent.depMode = "scan";
                        if (failed.length === 0) {
                            depSection.parent.depMessage = "All " + installed.length + " dependencies installed successfully";
                        } else {
                            depSection.parent.depMessage = installed.length + " installed, " + failed.length + " failed";
                        }
                        // Mark all results with final status
                        var results = depSection.parent.depScanResults.slice();
                        for (var i = 0; i < results.length; i++) {
                            if (installed.indexOf(results[i].id) >= 0)
                                results[i].status = "done";
                            else if (failed.indexOf(results[i].id) >= 0)
                                results[i].status = "error";
                        }
                        depSection.parent.depScanResults = results;
                    } else if (result && result.ok && result.deps) {
                        depSection.parent.depScanResults = result.deps;
                        depSection.parent.depMessage = result.deps.length > 0
                            ? ""
                            : (result.message || "No dependencies needed");
                    } else {
                        depSection.parent.depMessage = result ? (result.error || "Plugin failed") : "No response";
                    }
                }
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

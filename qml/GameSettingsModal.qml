import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "components"

// GameSettingsModal – per-game Proton/paths/overrides/env/args/flags.
// Mirrors Java GameSettings (View/Run/Graphics tab pill).
CModal {
    id: root
    title: "Game Settings"
    boxWidth: 640

    property string gameName: ""
    property string tab: "view"
    property bool isEmulatorGame: false
    property var emuSettingsDefs: []
    property var emuSettingsValues: ({})
    property bool hasEmuSettings: false
    property bool pendingLoad: false
    property var gameModeStatus: ({})
    property var mangoHudStatus: ({})

    function reloadEmuFields() {
        console.log("[GS] reloadEmuFields called, pendingLoad:", pendingLoad, "isEmu:", isEmulatorGame);
        if (pendingLoad || isEmulatorGame)
            loadFields();
    }

    // Dependency Installer state
    property var depScanResults: []
    property string depMessage: ""
    property string depMode: "scan"
    property int depTotal: 0
    property int depCompleted: 0
    property bool depBusy: false

    function openFor(name) {
        gameName = name;
        tab = "view";
        // Detect emulator BEFORE opening so tabs render correctly
        var g = games.getGame(name);
        var executor = (g && g.executor) ? g.executor : "";
        isEmulatorGame = executor !== "";
        pendingLoad = true;
        // Clear dependency state
        depScanResults = [];
        depMessage = "";
        depMode = "scan";
        depTotal = 0;
        depCompleted = 0;
        depBusy = false;
        open();
        // Always load fields immediately
        loadFields();
        // If emulator, ensure emulators list loads (Main.qml will call reloadEmuFields on change)
        if (isEmulatorGame && plugins.emulators.length === 0)
            plugins.listEmulators();
    }
    function loadFields() {
        var g = games.getGame(gameName);
        if (!g || !g.name) return;
        nameField.text = g.name || gameName;
        protonBox.model = ["GE-Proton Latest"].concat(proton.installedProtons());
        protonBox.currentIndex = Math.max(0, protonBox.find(g.proton || "GE-Proton Latest"));
        bannerField.text = g.banner || "";
        iconField.text = g.icon || "";
        prefixField.text = g.prefixPath || "";
        overridesField.text = g.overrides || "";
        envField.text = g.environment || "";
        argsBeforeField.text = g.argsBefore || "";
        argsAfterField.text = g.argsAfter || "";
        overlaySwitch.setSilent(g.steamOverlay === "true" || g.steamOverlay === "1");
        runtimeSwitch.setSilent(g.steamRuntime === "true" || g.steamRuntime === "1");
        graphicsWined3d.setSilent(g.wined3d === "true" || g.wined3d === "1");
        graphicsWayland.setSilent(g.nativeWayland === "true" || g.nativeWayland === "1");
        gameModeStatus = proton.graphicsComponentStatus("gamemode");
        mangoHudStatus = proton.graphicsComponentStatus("mangohud");
        // Load emulator settings
        var executor = g.executor || "";
        isEmulatorGame = executor !== "";
        emuSettingsValues = g.emuSettings || {};
        emuSettingsDefs = [];
        hasEmuSettings = false;
        if (isEmulatorGame) {
            var emus = plugins.emulators;
            console.log("[GS] loadFields executor:", executor, "emus:", emus.length);
            for (var i = 0; i < emus.length; i++) {
                if (emus[i].name === executor) {
                    emuSettingsDefs = emus[i].settings || [];
                    hasEmuSettings = emuSettingsDefs.length > 0;
                    console.log("[GS] matched:", executor, "defs:", emuSettingsDefs.length);
                    break;
                }
            }
            if (tab === "run" || tab === "graphics")
                tab = "view";
        }
        pendingLoad = false;
    }
    function save() {
        var fields = {
            "proton": protonBox.currentText,
            "prefixPath": prefixField.text.trim(),
            "overrides": overridesField.text.trim(),
            "environment": envField.text.trim(),
            "argsBefore": argsBeforeField.text.trim(),
            "argsAfter": argsAfterField.text.trim(),
            "banner": bannerField.text.trim(),
            "icon": iconField.text.trim()
        };
        // Save emulator settings if this is an emulator game
        if (isEmulatorGame) {
            fields["emuSettings"] = emuSettingsValues;
        }
        games.addGame(gameName, fields);
    }
    function saveName() {
        var newName = nameField.text.trim();
        if (newName !== "" && newName !== gameName && games.renameGame(gameName, newName))
            gameName = newName;
    }
    property string pendingEmuPathId: ""

    function saveEmuPath(path) {
        var vals = {};
        for (var k in root.emuSettingsValues)
            vals[k] = root.emuSettingsValues[k];
        vals[root.pendingEmuPathId] = path;
        root.emuSettingsValues = vals;
        root.save();
    }
    function localFilePath(url) {
        var value = url.toString();
        if (value.indexOf("file://") === 0)
            value = value.substring(7);
        return decodeURIComponent(value);
    }

    function emulatorSettingLabel(setting) {
        var labels = {
            "fullscreen": "Fullscreen",
            "show_osd": "Show OSD overlay",
            "jit_enable": "Enable JIT (higher performance)",
            "batch": "Batch mode (run without opening the interface)",
            "user_dir": "Save and configuration folder",
            "hide_osd": "Hide OSD overlay",
            "exit_on_pause": "Exit when opening the pause menu",
            "no_gui": "No GUI (run the game directly)",
            "full_boot": "Full boot (do not skip the PS3 animation)",
            "keys_path": "Encryption keys path (prod.keys)",
            "title_keys_path": "Title keys path (title.keys)",
            "config_path": "Custom configuration path",
            "load_config": "Load configuration file",
            "frameskip": "Frame skip (higher speed, less smooth)",
            "mlc_path": "Custom mlc folder path"
        };
        return labels[setting.id] || setting.id || "Setting";
    }

    Column {
        width: parent.width
        spacing: 12
        // View tab (shared)
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "view"
            Text { text: "Game name in launcher"; color: Theme.textSec; font.pixelSize: 12 }
            CTextField { id: nameField; width: parent.width; onEditingFinished: saveName() }
            Row {
                width: parent.width
                spacing: 12
                Column {
                    width: (parent.width - 12) / 2
                    spacing: 4
                    Text { text: "Icon"; color: Theme.textSec; font.pixelSize: 12 }
                    Rectangle {
                        width: parent.width
                        height: 120
                        radius: 8
                        color: Theme.isLight ? "#E9ECEF" : "#181818"
                        border.color: Theme.border
                        border.width: 1
                        Image {
                            anchors.centerIn: parent
                            width: 88
                            height: 88
                            source: iconField.text ? ("file://" + iconField.text) : ""
                            fillMode: Image.PreserveAspectFit
                            sourceSize.width: 128
                            sourceSize.height: 128
                        }
                        CButton {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: 6
                            text: "✎"
                            width: 34
                            height: 30
                            onClicked: iconDialog.open()
                        }
                    }
                }
                Column {
                    width: (parent.width - 12) / 2
                    spacing: 4
                    Text { text: "Banner / cover"; color: Theme.textSec; font.pixelSize: 12 }
                    Rectangle {
                        width: parent.width
                        height: 120
                        radius: 8
                        color: Theme.isLight ? "#E9ECEF" : "#181818"
                        border.color: Theme.border
                        border.width: 1
                        Image {
                            anchors.fill: parent
                            anchors.margins: 4
                            source: bannerField.text ? ("file://" + bannerField.text) : ""
                            fillMode: Image.PreserveAspectFit
                            sourceSize.width: 460
                            sourceSize.height: 215
                        }
                        CButton {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: 6
                            text: "✎"
                            width: 34
                            height: 30
                            onClicked: bannerDialog.open()
                        }
                    }
                }
            }
            CTextField { id: bannerField; visible: false }
            CTextField { id: iconField; visible: false }
            Text { text: "Install path"; color: Theme.textSec; font.pixelSize: 12; visible: false }
            CTextField { visible: false; readOnly: true; text: games.getGame(root.gameName).mainPath || "" }
            // Emulator name
            Column {
                width: parent.width
                spacing: 8
                visible: root.isEmulatorGame
                Text { text: "Emulator"; color: Theme.textSec; font.pixelSize: 12 }
                CTextField { width: parent.width; readOnly: true; text: games.getGame(root.gameName).executor || "" }
            }
        }
        // Run tab (Wine/Proton only)
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "run" && !root.isEmulatorGame
            Row {
                width: parent.width
                spacing: 12
                CCard {
                    width: (parent.width - 12) / 2
                    outlineColor: Theme.border
                    outlineWidth: 1
                    Text { text: "Launch options"; color: Theme.textMain; font.bold: true; font.pixelSize: 13 }
                    Text { text: "DLL overrides (WINEDLLOVERRIDES)"; color: Theme.textSec; font.pixelSize: 12 }
                    CTextField { id: overridesField; width: parent.width; onEditingFinished: save() }
                    Text { text: "Environment variables"; color: Theme.textSec; font.pixelSize: 12 }
                    CTextField { id: envField; width: parent.width; onEditingFinished: save() }
                    Text { text: "Arguments before executable"; color: Theme.textSec; font.pixelSize: 12 }
                    CTextField { id: argsBeforeField; width: parent.width; onEditingFinished: save() }
                    Text { text: "Arguments after executable"; color: Theme.textSec; font.pixelSize: 12 }
                    CTextField { id: argsAfterField; width: parent.width; onEditingFinished: save() }
                    Row {
                        spacing: 10
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
                }
                CCard {
                    width: (parent.width - 12) / 2
                    outlineColor: Theme.border
                    outlineWidth: 1
                    Text { text: "Proton and prefix"; color: Theme.textMain; font.bold: true; font.pixelSize: 13 }
                    Text { text: "Proton version"; color: Theme.textSec; font.pixelSize: 12 }
                    CComboBox {
                        id: protonBox
                        width: parent.width
                        height: 28
                        editable: true
                        onActivated: save()
                        onAccepted: save()
                    }
                    Text { text: "Prefix path"; color: Theme.textSec; font.pixelSize: 12 }
                    CTextField { id: prefixField; width: parent.width; onEditingFinished: save() }
                }
            }
        }
        // Emulator tab (emulator games only, only if settings exist)
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "emulator" && root.isEmulatorGame && root.hasEmuSettings
            Repeater {
                model: root.emuSettingsDefs
                delegate: Loader {
                    required property var modelData
                    required property int index
                    width: parent.width
                    active: true
                    sourceComponent: modelData.type === "bool" ? boolSettingComp
                        : modelData.type === "path" ? pathSettingComp : null
                    Component {
                        id: boolSettingComp
                        CSwitch {
                            width: parent.width
                            objectName: root.emulatorSettingLabel(modelData)
                            checked: root.emuSettingsValues[modelData.id] !== undefined
                                ? root.emuSettingsValues[modelData.id] === true
                                : modelData.default === true
                            onToggled: {
                                var vals = {};
                                for (var k in root.emuSettingsValues)
                                    vals[k] = root.emuSettingsValues[k];
                                vals[modelData.id] = checked;
                                root.emuSettingsValues = vals;
                                root.save();
                            }
                        }
                    }
                    Component {
                        id: pathSettingComp
                        Column {
                            width: parent.width
                            spacing: 4
                            Text {
                                text: root.emulatorSettingLabel(modelData)
                                color: Theme.textSec
                                font.pixelSize: 12
                            }
                            Row {
                                width: parent.width
                                spacing: 8
                                CTextField {
                                    id: pathField
                                    width: parent.width - 88
                                    text: root.emuSettingsValues[modelData.id] || ""
                                    onEditingFinished: {
                                        var vals = {};
                                        for (var k in root.emuSettingsValues)
                                            vals[k] = root.emuSettingsValues[k];
                                        vals[modelData.id] = text.trim();
                                        root.emuSettingsValues = vals;
                                        root.save();
                                    }
                                }
                                CButton {
                                    text: "..."
                                    width: 36
                                    height: 32
                                    onClicked: {
                                        root.pendingEmuPathId = modelData.id;
                                        emuPathDialog.open();
                                    }
                                }
                                CButton {
                                    text: "X"
                                    width: 36
                                    height: 32
                                    onClicked: {
                                        pathField.text = "";
                                        var vals = {};
                                        for (var k in root.emuSettingsValues)
                                            vals[k] = root.emuSettingsValues[k];
                                        vals[modelData.id] = "";
                                        root.emuSettingsValues = vals;
                                        root.save();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Graphics tab: per-game graphics overrides.
        Column {
            width: parent.width
            spacing: 8
            visible: root.tab === "graphics"
            Text {
                text: "Graphics options for this game only."
                color: Theme.textSec
                font.pixelSize: 12
                wrapMode: Text.Wrap
                width: parent.width
            }
            Text {
                text: "New games use the defaults from Settings > Miscellaneous."
                color: Theme.textMuted
                font.pixelSize: 11
                wrapMode: Text.Wrap
                width: parent.width
            }
            CSwitch {
                id: graphicsWined3d
                objectName: "Use wined3d"
                Component.onCompleted: setSilent(games.getGame(root.gameName).wined3d === "true"
                    || games.getGame(root.gameName).wined3d === "1")
                onToggled: games.addGame(root.gameName, {"wined3d": checked ? "true" : "false"})
            }
            CSwitch {
                id: graphicsWayland
                objectName: "Prefer Wayland"
                Component.onCompleted: setSilent(games.getGame(root.gameName).nativeWayland === "true"
                    || games.getGame(root.gameName).nativeWayland === "1")
                onToggled: games.addGame(root.gameName, {"nativeWayland": checked ? "true" : "false"})
            }
            CSwitch {
                objectName: "Use GameMode"
                enabled: gameModeStatus.available === true
                Component.onCompleted: setSilent(games.getGame(root.gameName).gameMode === "true")
                onToggled: games.addGame(root.gameName, {"gameMode": checked ? "true" : "false"})
            }
            Text {
                text: "GameMode: " + (gameModeStatus.available === true ? "32-bit and 64-bit ready"
                    : gameModeStatus.installed32 === true ? "32-bit installed; 64-bit missing"
                    : gameModeStatus.installed64 === true ? "64-bit installed; 32-bit missing"
                    : "not installed")
                color: Theme.textSec
                font.pixelSize: 11
            }
            CSwitch {
                objectName: "Use MangoHud"
                enabled: mangoHudStatus.available === true
                Component.onCompleted: setSilent(games.getGame(root.gameName).mangoHud === "true")
                onToggled: games.addGame(root.gameName, {"mangoHud": checked ? "true" : "false"})
            }
            Text {
                text: "MangoHud: " + (mangoHudStatus.available === true ? "32-bit and 64-bit ready"
                    : mangoHudStatus.installed32 === true ? "32-bit installed; 64-bit missing"
                    : mangoHudStatus.installed64 === true ? "64-bit installed; 32-bit missing"
                    : "not installed")
                color: Theme.textSec
                font.pixelSize: 11
            }

            // ---- Dependency Installer (only if plugin installed) ----
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
                    enabled: !root.depBusy
                    onClicked: {
                        var g = games.getGame(root.gameName);
                        if (!g || !g.mainPath) {
                            root.depMessage = "No game directory found";
                            root.depScanResults = [];
                            return;
                        }
                        var plugPath = depSection.getPluginPath();
                        if (!plugPath) {
                            root.depMessage = "Plugin not found or not enabled";
                            root.depScanResults = [];
                            return;
                        }
                        root.depBusy = true;
                        root.depMode = "scan";
                        root.depMessage = "Scanning " + g.mainPath + "...";
                        root.depScanResults = [];
                        var scanArgs = ["scan", g.mainPath];
                        if (g.prefixPath) scanArgs = scanArgs.concat(["--prefix", g.prefixPath]);
                        integrations.runPlugin(plugPath, scanArgs);
                    }
                }
                Text {
                    text: root.depMessage
                    color: root.depScanResults.length > 0 ? Theme.textSec : Theme.accent
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
                    visible: root.depMode === "install" && root.depTotal > 0
                    Rectangle {
                        width: parent.width * (root.depTotal > 0 ? root.depCompleted / root.depTotal : 0)
                        height: parent.height
                        radius: 3
                        color: Theme.accent
                        Behavior on width { NumberAnimation { duration: 300 } }
                    }
                }
                Repeater {
                    id: depList
                    model: root.depScanResults
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
                            visible: depStatus === "" && root.depMode === "scan"
                            Component.onCompleted: setSilent(false)
                        }

                        Column {
                            width: parent.width - (depStatus !== "" || root.depMode === "install" ? 26 : 50)
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
                    enabled: !root.depBusy
                    visible: {
                        if (root.depMode !== "scan") return false;
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
                        if (!plugPath) { root.depMessage = "Plugin not found"; return; }
                        var g = games.getGame(root.gameName);
                        if (!g) { root.depMessage = "Game not found"; return; }
                        var prefix = g.prefixPath || "";
                        if (!prefix) { root.depMessage = "No prefix found"; return; }
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
                        root.depBusy = true;
                        root.depMode = "install";
                        root.depTotal = deps.length;
                        root.depCompleted = 0;
                        root.depMessage = "Installing " + deps.length + " dependencies...";
                        root.depScanResults = [];
                        integrations.runPlugin(plugPath, installArgs);
                    }
                }
            }

            Connections {
                target: integrations
                function onPluginProgress(progress) {
                    console.log("[DepInstaller] progress:", JSON.stringify(progress))
                    // Update or add item in the results list
                    var results = root.depScanResults.slice();
                    var found = false;
                    for (var i = 0; i < results.length; i++) {
                        if (results[i].id === progress.id) {
                            results[i] = progress;
                            found = true;
                            break;
                        }
                    }
                    if (!found) results.push(progress);
                    root.depScanResults = results;
                    root.depMessage = "Installing " + (root.depCompleted + 1) + "/" + root.depTotal + ": " + progress.desc + "...";
                    // Update progress counter
                    if (progress.status === "done" || progress.status === "error") {
                        root.depCompleted = root.depCompleted + 1;
                    }
                }
                function onPluginResult(result) {
                    console.log("[DepInstaller] result:", JSON.stringify(result))
                    root.depBusy = false;
                    if (result && result.type === "done") {
                        var installed = result.installed || [];
                        var failed = result.failed || [];
                        root.depCompleted = root.depTotal;
                        root.depMode = "scan";
                        if (failed.length === 0) {
                            root.depMessage = "All " + installed.length + " dependencies installed successfully";
                        } else {
                            root.depMessage = installed.length + " installed, " + failed.length + " failed";
                        }
                        // Mark all results with final status
                        var results = root.depScanResults.slice();
                        for (var i = 0; i < results.length; i++) {
                            if (installed.indexOf(results[i].id) >= 0)
                                results[i].status = "done";
                            else if (failed.indexOf(results[i].id) >= 0)
                                results[i].status = "error";
                        }
                        root.depScanResults = results;
                    } else if (result && result.ok && result.deps) {
                        root.depScanResults = result.deps;
                        root.depMessage = result.deps.length > 0
                            ? ""
                            : (result.message || "No dependencies needed");
                    } else {
                        root.depMessage = result ? (result.error || "Plugin failed") : "No response";
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
                model: root.isEmulatorGame ? [
                    { "id": "view", "label": "View", "icon": "palette" },
                    { "id": "emulator", "label": "Emulator", "icon": "startup" }
                ] : [
                    { "id": "view", "label": "View", "icon": "palette" },
                    { "id": "run", "label": "Run", "icon": "startup" },
                    { "id": "graphics", "label": "Graphics", "icon": "graphics" }
                ]
                Button {
                    required property var modelData
                    text: modelData.label
                    width: 104
                    height: 52
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

    Item {
        width: 0
        height: 0
        Connections {
            target: plugins
            function onEmulatorsChanged() { root.reloadEmuFields(); }
        }
        FileDialog {
            id: emuPathDialog
            title: "Choose emulator path"
            onAccepted: root.saveEmuPath(root.localFilePath(selectedFile))
        }
        FileDialog {
            id: bannerDialog
            title: "Choose cover or banner"
            nameFilters: ["Images (*.png *.jpg *.jpeg *.webp)", "All files (*)"]
            onAccepted: {
                bannerField.text = root.localFilePath(selectedFile);
                save();
            }
        }
        FileDialog {
            id: iconDialog
            title: "Choose icon"
            nameFilters: ["Images (*.png *.jpg *.jpeg *.webp *.ico)", "All files (*)"]
            onAccepted: {
                iconField.text = root.localFilePath(selectedFile);
                save();
            }
        }
    }
}

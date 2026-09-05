import QtQuick
import QtQuick.Controls
import "components"

// SettingsModal – Visuals / Paths / Protons / Misc / Plugins / Integrations / About.
// Mirrors Java LauncherSettings (720px modal).
CModal {
    id: root
    title: "Settings"
    boxWidth: 720

    signal openProton
    property string page: "visuals"
    readonly property int tabCount: 7

    function setPage(p) {
        root.page = p;
        if (p === "integrations")
            integrationsPage.refresh();
        if (p === "protons")
            proton.fetchReleases();
        if (p === "plugins") {
            plugins.refresh();
            plugins.fetchRegistry();
        }
    }

    Column {
        width: parent.width
        spacing: 12

        // ---- pages (scrollable) ----
        Flickable {
            width: parent.width
            height: 380
            clip: true
            contentWidth: width
            contentHeight: pagesCol.height
            Column {
                id: pagesCol
                width: parent.width

            // VISUALS
            Column {
                width: parent.width
                spacing: 10
                visible: root.page === "visuals"
                Text { text: "Theme Mode"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                Text {
                    text: "Dark is the default look. Light is easier on bright screens."
                    color: Theme.textSec
                    font.pixelSize: 11
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Row {
                    spacing: 8
                    anchors.horizontalCenter: parent.horizontalCenter
                    Button {
                        width: 130
                        height: 36
                        checkable: true
                        checked: Theme.theme === "dark"
                        autoExclusive: true
                        background: Rectangle {
                            radius: 8
                            color: "#000000"
                            border.color: "#FFFFFF"
                            border.width: parent.checked ? 3 : 1.5
                        }
                        contentItem: Item {
                            Row {
                            spacing: 8
                            anchors.centerIn: parent
                            CIcon { iconName: "moon"; iconSize: 20; themed: false }
                            Text {
                                text: "Dark"
                                color: "#FFFFFF"
                                font.bold: true
                                font.pixelSize: 13
                                verticalAlignment: Text.AlignVCenter
                            }
                            }
                        }
                        onClicked: Theme.theme = "dark"
                    }
                    Button {
                        width: 130
                        height: 36
                        checkable: true
                        checked: Theme.theme === "light"
                        autoExclusive: true
                        background: Rectangle {
                            radius: 8
                            color: "#FFFFFF"
                            border.color: "#000000"
                            border.width: parent.checked ? 3 : 1.5
                        }
                        contentItem: Item {
                            Row {
                            spacing: 8
                            anchors.centerIn: parent
                            CIcon { iconName: "sun"; iconSize: 20; forceDark: true }
                            Text {
                                text: "Light"
                                color: "#000000"
                                font.bold: true
                                font.pixelSize: 13
                                verticalAlignment: Text.AlignVCenter
                            }
                            }
                        }
                        onClicked: Theme.theme = "light"
                    }
                }
                Text { text: "Theme Colors"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                Text {
                    text: "Choose an accent color for buttons, switches, and highlights"
                    color: Theme.textSec
                    font.pixelSize: 11
                }
                Flow {
                    width: parent.width
                    spacing: 8
                    Repeater {
                        model: Theme.accentIds()
                        Button {
                            required property string modelData
                            text: Theme.accentInfo(modelData).name
                            width: 100
                            height: 34
                            checkable: true
                            checked: Theme.accentId === modelData
                            autoExclusive: true
                            background: Rectangle {
                                radius: 8
                                color: Theme.accentInfo(modelData).primary
                                border.color: Theme.isLight ? "#000000" : "#FFFFFF"
                                border.width: parent.checked ? 3 : 0
                            }
                            contentItem: Text {
                                text: parent.text
                                color: "#FFFFFF"
                                font.bold: true
                                font.pixelSize: 12
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                            }
                            onClicked: Theme.accentId = modelData
                        }
                    }
                }
            }

            // PATHS
            Column {
                width: parent.width
                spacing: 10
                visible: root.page === "paths"
                Text { text: "Paths"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                Repeater {
                    model: [
                        { "label": "Installs path", "key": "installsPath", "base": "installs" },
                        { "label": "Downloads path", "key": "downloadsPath", "base": "downloads" },
                        { "label": "Prefixes path", "key": "prefixesPath", "base": "prefixes" },
                        { "label": "Proton Path 1", "key": "protonsPath", "base": "protons" },
                        { "label": "Proton Path 2 (optional)", "key": "protonsPath2", "base": "" },
                        { "label": "Proton Path 3 (optional)", "key": "protonsPath3", "base": "" }
                    ]
                    Column {
                        required property var modelData
                        width: parent.width
                        spacing: 4
                        Text { text: modelData.label; color: Theme.textSec; font.pixelSize: 12 }
                        CTextField {
                            width: parent.width
                            text: {
                                var custom = config.launcherValue(modelData.key, "User Settings");
                                if (custom !== "")
                                    return custom;
                                return modelData.base !== "" ? config.basePathFor(modelData.base) : "";
                            }
                            onEditingFinished: config.setLauncherValue(modelData.key, text.trim(), "User Settings")
                        }
                    }
                }
            }

            // PROTONS
            Column {
                id: protonsPage
                width: parent.width
                spacing: 10
                visible: root.page === "protons"
                property string filter: "all"
                property var releases: []
                property string pathFilter: ""
                property var installedDetails: []
                property var defaultProtonModel: proton.installedProtons()
                Text { text: "Protons"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                Row {
                    width: parent.width
                    spacing: 8
                    Text {
                        text: "Default Proton for new games:"
                        color: Theme.textMain
                        font.pixelSize: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: 200
                    }
                    CComboBox {
                        id: defaultProtonBox
                        width: 260
                        model: protonsPage.defaultProtonModel
                        Component.onCompleted: currentIndex = Math.max(0, find(config.launcherValue("defaultProton", "User Settings")))
                        onActivated: config.setLauncherValue("defaultProton", currentText, "User Settings")
                    }
                }
                // source filter: All | CachyOS | Proton-GE (mirrors Java protonFilter)
                Row {
                    spacing: 6
                    Repeater {
                        model: [
                            { "id": "all", "label": "All" },
                            { "id": "cachy", "label": "CachyOS" },
                            { "id": "ge", "label": "Proton-GE" }
                        ]
                        CButton {
                            required property var modelData
                            text: modelData.label
                            width: 110
                            height: 30
                            kind: protonsPage.filter === modelData.id ? "primary" : "outline"
                            onClicked: protonsPage.filter = modelData.id
                        }
                    }
                }
                Text {
                    id: protonStatus
                    text: ""
                    color: Theme.textSec
                    font.pixelSize: 12
                    width: parent.width
                    wrapMode: Text.Wrap
                    visible: text !== ""
                }
                // Installed builds (path filter like Java protonPathsCombo)
                Text {
                    text: "Installed builds"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 13
                }
                Row {
                    width: parent.width
                    spacing: 8
                    Text {
                        text: "Path:"
                        color: Theme.textSec
                        font.pixelSize: 12
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    CComboBox {
                        id: protonPathBox
                        width: 400
                        model: ["All paths"].concat(config.allProtonPaths())
                        onActivated: {
                            protonsPage.pathFilter = currentIndex <= 0 ? "" : currentText;
                            protonsPage.installedDetails = proton.installedProtonDetails();
                        }
                        Component.onCompleted: protonsPage.installedDetails = proton.installedProtonDetails()
                    }
                }
                ListView {
                    id: protonInstalledList
                    width: parent.width
                    height: Math.min(120, count * 40)
                    visible: count > 0
                    clip: true
                    spacing: 4
                    model: protonsPage.installedDetails.filter(function(d) {
                        return protonsPage.pathFilter === "" || d.path === protonsPage.pathFilter;
                    })
                    delegate: Rectangle {
                        required property var modelData
                        width: parent ? parent.width : 0
                        height: 36
                        radius: 8
                        color: Theme.well
                        Row {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 8
                            spacing: 8
                            Text {
                                width: parent.width - 60
                                text: modelData.name
                                color: Theme.textMain
                                font.pixelSize: 12
                                verticalAlignment: Text.AlignVCenter
                                anchors.verticalCenter: parent.verticalCenter
                                elide: Text.ElideMiddle
                            }
                            CButton {
                                width: 36
                                height: 28
                                anchors.verticalCenter: parent.verticalCenter
                                iconSource: "remove"
                                iconSize: 14
                                text: ""
                                onClicked: {
                                    proton.removeProton(modelData.name);
                                    protonsPage.installedDetails = proton.installedProtonDetails();
                                    protonsPage.defaultProtonModel = proton.installedProtons();
                                }
                            }
                        }
                    }
                }
                Text {
                    text: "Available for download"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 13
                }
                ListView {
                    id: protonAvailList
                    width: parent.width
                    height: Math.min(200, count * 44)
                    visible: count > 0
                    clip: true
                    spacing: 4
                    model: protonsPage.releases.filter(function(r) {
                        if (protonsPage.filter === "cachy") return r.source === "cachy";
                        if (protonsPage.filter === "ge") return r.source === "ge";
                        return true;
                    })
                    delegate: Rectangle {
                        required property var modelData
                        width: protonAvailList.width
                        height: 40
                        radius: 8
                        color: Theme.well
                        Row {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 8
                            spacing: 8
                            Text {
                                width: parent.width - 130
                                text: (modelData.source === "cachy" ? "[CachyOS] " : "[GE] ") + modelData.tag
                                color: Theme.textMain
                                font.pixelSize: 12
                                verticalAlignment: Text.AlignVCenter
                                anchors.verticalCenter: parent.verticalCenter
                                elide: Text.ElideMiddle
                            }
                            CButton {
                                width: 36
                                height: 28
                                anchors.verticalCenter: parent.verticalCenter
                                iconSource: proton.installedProtons().indexOf(modelData.tag) >= 0 ? "remove" : "download"
                                iconSize: 16
                                text: ""
                                onClicked: {
                                    if (proton.installedProtons().indexOf(modelData.tag) >= 0) {
                                        proton.removeProton(modelData.tag);
                                        protonStatus.text = "Removed " + modelData.tag;
                                    } else {
                                        protonStatus.text = "Downloading " + modelData.tag + "…";
                                        proton.downloadProton(modelData.tag, modelData.url);
                                    }
                                }
                            }
                        }
                    }
                }
                Connections {
                    target: proton
                    function onReleasesReady(list) { protonsPage.releases = list; }
                    function onDownloadFinished(ok, message) {
                        protonStatus.text = ok ? ("Installed " + message) : ("Failed: " + message);
                        protonsPage.installedDetails = proton.installedProtonDetails();
                        protonsPage.defaultProtonModel = proton.installedProtons();
                    }
                }
                Component.onCompleted: proton.fetchReleases()
            }

            // MISC
            Column {
                width: parent.width
                spacing: 10
                visible: root.page === "misc"
                Text { text: "Miscellaneous"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                Text {
                    text: "General"
                    color: Theme.textSec
                    font.bold: true
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                CCard {
                    width: parent.width
                    CSwitch {
                    objectName: "Use wined3d instead of DXVK"
                    Component.onCompleted: setSilent(config.launcherValue("gamesUsesWined3d", "User Settings") === "1")
                    onToggled: config.setLauncherValue("gamesUsesWined3d", checked ? "1" : "0", "User Settings")
                }
                CSwitch {
                    objectName: "Prefer Wayland driver"
                    Component.onCompleted: setSilent(config.launcherValue("gamesUsesWayland", "User Settings") === "1")
                    onToggled: config.setLauncherValue("gamesUsesWayland", checked ? "1" : "0", "User Settings")
                }
                CSwitch {
                    objectName: "Use umu-launcher (unified Proton runner)"
                    Component.onCompleted: setSilent(proton.useUmu)
                    onToggled: proton.useUmu = checked
                }
                }
            }

            // PLUGINS
            Column {
                id: pluginsPage
                width: parent.width
                spacing: 10
                visible: root.page === "plugins"
                property bool emuExpanded: false
                property var emuList: []
                property var emuInstalling: ({})

                Timer {
                    id: emuErrorTimer
                    interval: 3000
                    onTriggered: { delete pluginsPage.emuInstalling["_error"]; pluginsPage.emuInstalling = pluginsPage.emuInstalling; }
                }

                Text { text: "Plugins"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }

                // ---- Installed Plugins ----
                Text {
                    text: "Installed Plugins"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 14
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    text: "No plugins installed."
                    color: Theme.textSec
                    font.pixelSize: 12
                    wrapMode: Text.Wrap
                    width: parent.width
                    visible: plugins.plugins.length === 0
                }
                Repeater {
                    model: plugins.plugins
                    CCard {
                        required property var modelData
                        width: parent.width
                        // ---- Regular plugin (switch + delete) ----
                        Column {
                            width: parent.width
                            visible: (modelData.type || "") !== "emulator-manager"
                            Row {
                                width: parent.width
                                spacing: 8
                                Column {
                                    width: parent.width - 110
                                    spacing: 2
                                    Text {
                                        text: modelData.name + (modelData.version ? "  v" + modelData.version : "")
                                        color: Theme.textMain
                                        font.bold: true
                                        font.pixelSize: 13
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                    Text {
                                        text: modelData.description || ""
                                        color: Theme.textSec
                                        font.pixelSize: 11
                                        wrapMode: Text.Wrap
                                        width: parent.width
                                        visible: (modelData.description || "") !== ""
                                    }
                                    Text {
                                        text: "Capabilities: " + (modelData.capabilities || []).join(", ")
                                        color: Theme.textMuted
                                        font.pixelSize: 11
                                        wrapMode: Text.Wrap
                                        width: parent.width
                                        visible: (modelData.capabilities || []).length > 0
                                    }
                                }
                                CSwitch {
                                    objectName: ""
                                    width: 52
                                    Component.onCompleted: setSilent(!!modelData.enabled)
                                    onToggled: plugins.setEnabled(modelData.id, checked)
                                }
                                CButton {
                                    text: ""
                                    iconSource: "remove"
                                    iconSize: 14
                                    width: 36
                                    height: 28
                                    kind: "outline"
                                    onClicked: plugins.removePlugin(modelData.id)
                                }
                            }
                        }
                        // ---- Emulator-manager plugin (expand + delete) ----
                        Column {
                            width: parent.width
                            visible: (modelData.type || "") === "emulator-manager"
                            spacing: 4
                            Row {
                                width: parent.width
                                spacing: 8
                                Text {
                                    text: pluginsPage.emuExpanded ? "\u25BC" : "\u25B6"
                                    color: Theme.textMain
                                    font.pixelSize: 14
                                    anchors.verticalCenter: parent.verticalCenter
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            pluginsPage.emuExpanded = !pluginsPage.emuExpanded;
                                            if (pluginsPage.emuExpanded)
                                                plugins.listEmulators();
                                        }
                                    }
                                }
                                Column {
                                    width: parent.width - 110
                                    spacing: 2
                                    Text {
                                        text: modelData.name + (modelData.version ? "  v" + modelData.version : "")
                                        color: Theme.textMain
                                        font.bold: true
                                        font.pixelSize: 13
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                    Text {
                                        text: modelData.description || ""
                                        color: Theme.textSec
                                        font.pixelSize: 11
                                        wrapMode: Text.Wrap
                                        width: parent.width
                                        visible: (modelData.description || "") !== ""
                                    }
                                }
                                CButton {
                                    text: ""
                                    iconSource: "remove"
                                    iconSize: 14
                                    width: 36
                                    height: 28
                                    kind: "outline"
                                    onClicked: {
                                        plugins.removePlugin(modelData.id);
                                    }
                                }
                            }
                            // Status text
                            Column {
                                width: parent.width
                                spacing: 4
                                visible: pluginsPage.emuExpanded
                                Repeater {
                                    model: pluginsPage.emuList
                                    Row {
                                        required property var modelData
                                        width: parent.width
                                        spacing: 8
                                        Rectangle {
                                            width: 6
                                            height: 6
                                            radius: 3
                                            color: modelData.installed ? "#00E639" : Theme.textMuted
                                            anchors.verticalCenter: parent.verticalCenter
                                        }
                                        Text {
                                            width: parent.width - 140
                                            text: modelData.name + " — " + modelData.description
                                            color: Theme.textMain
                                            font.pixelSize: 12
                                            elide: Text.ElideRight
                                            anchors.verticalCenter: parent.verticalCenter
                                        }
                                        Text {
                                            text: pluginsPage.emuInstalling[modelData.name] || ""
                                            color: Theme.accent
                                            font.pixelSize: 10
                                            width: 70
                                            anchors.verticalCenter: parent.verticalCenter
                                            visible: (pluginsPage.emuInstalling[modelData.name] || "") !== ""
                                        }
                                        CButton {
                                            text: modelData.installed ? "Remove" : "Install"
                                            width: 70
                                            height: 26
                                            kind: modelData.installed ? "outline" : "filled"
                                            enabled: !(pluginsPage.emuInstalling[modelData.name] || "")
                                            anchors.verticalCenter: parent.verticalCenter
                                            onClicked: {
                                                if (modelData.installed) {
                                                    plugins.removeEmulator(modelData.name);
                                                } else {
                                                    pluginsPage.emuInstalling[modelData.name] = "Installing…";
                                                    pluginsPage.emuInstalling = pluginsPage.emuInstalling;
                                                    plugins.installEmulator(modelData.name);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ---- Install Plugins (remote registry) ----
                Text {
                    text: "Available Plugins"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 14
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    id: registryStatus
                    text: ""
                    color: Theme.textSec
                    font.pixelSize: 12
                    width: parent.width
                    wrapMode: Text.Wrap
                    visible: text !== ""
                }
                ProgressBar {
                    id: pluginDlBar
                    width: parent.width
                    visible: plugins.downloadProgress >= 0 && plugins.downloadProgress < 1
                    value: plugins.downloadProgress < 0 ? 0 : plugins.downloadProgress
                    background: Rectangle {
                        color: Theme.isLight ? "#E9ECEF" : "#3E3E3E"
                        radius: 4
                    }
                    contentItem: Item {
                        Rectangle {
                            width: parent.width * pluginDlBar.visualPosition
                            height: parent.height
                            radius: 4
                            color: Theme.accent
                        }
                    }
                }
                Repeater {
                    id: registryList
                    model: []
                    CCard {
                        required property var modelData
                        required property int index
                        width: parent.width
                        Row {
                            width: parent.width
                            spacing: 8
                            Column {
                                width: parent.width - 100
                                spacing: 2
                                Text {
                                    text: modelData.name || modelData.tag
                                    color: Theme.textMain
                                    font.bold: true
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                    width: parent.width
                                }
                                Text {
                                    text: modelData.description || ""
                                    color: Theme.textSec
                                    font.pixelSize: 11
                                    wrapMode: Text.Wrap
                                    width: parent.width
                                    visible: (modelData.description || "") !== ""
                                }
                                Text {
                                    text: modelData.date || ""
                                    color: Theme.textMuted
                                    font.pixelSize: 10
                                    width: parent.width
                                    visible: (modelData.date || "") !== ""
                                }
                            }
                            CButton {
                                text: "Install"
                                width: 80
                                height: 28
                                enabled: registryStatus.text !== "Installing…"
                                onClicked: {
                                    registryStatus.text = "Installing " + (modelData.tag || "") + "…";
                                    plugins.downloadPlugin(modelData.tag, modelData.url);
                                }
                            }
                        }
                    }
                }
                Connections {
                    target: plugins
                    function onRegistryReady(list) {
                        // Filter out plugins already installed
                        var installedIds = [];
                        var installedNames = [];
                        for (var i = 0; i < plugins.plugins.length; i++) {
                            installedIds.push((plugins.plugins[i].id || "").toLowerCase());
                            var n = plugins.plugins[i].name || "";
                            n = n.replace(/\s+v[\d.]+\s*$/i, "").trim().toLowerCase();
                            installedNames.push(n);
                        }
                        var filtered = [];
                        for (var j = 0; j < list.length; j++) {
                            var rname = (list[j].name || "").replace(/\s+v[\d.]+\s*$/i, "").trim().toLowerCase();
                            var rtag = (list[j].tag || "").toLowerCase();
                            var rasset = (list[j].assetName || "").toLowerCase();
                            // Check against tag, name, or asset name
                            var found = false;
                            for (var k = 0; k < installedIds.length; k++) {
                                var id = installedIds[k];
                                if (rtag.indexOf(id) >= 0 || rasset.indexOf(id) >= 0 || rname === installedNames[k]) {
                                    found = true;
                                    break;
                                }
                            }
                            if (!found)
                                filtered.push(list[j]);
                        }
                        registryList.model = filtered;
                        registryStatus.text = filtered.length === 0 ? "No new plugins available." : "";
                    }
                    function onDownloadFinished(ok, message) {
                        if (ok) {
                            registryStatus.text = "Installed " + message;
                            plugins.refresh();
                        } else {
                            registryStatus.text = "Failed: " + message;
                        }
                    }
                    function onEmulatorsChanged() {
                        pluginsPage.emuList = plugins.emulators;
                    }
                    function onPluginsChanged() {
                        plugins.fetchRegistry();
                    }
                    function onEmulatorInstallFinished(ok, message) {
                        // Find which emulator finished and clear its status
                        var installing = pluginsPage.emuInstalling;
                        for (var name in installing) {
                            if (installing[name] && message.indexOf(name) >= 0) {
                                delete installing[name];
                                break;
                            }
                        }
                        pluginsPage.emuInstalling = installing;
                        if (!ok) {
                            pluginsPage.emuInstalling["_error"] = message;
                            emuErrorTimer.start();
                        }
                        plugins.listEmulators();
                    }
                }
            }

            // INTEGRATIONS
            Column {
                width: parent.width
                spacing: 10
                visible: root.page === "integrations"
                Column {
                    id: integrationsPage
                    width: parent.width
                    spacing: 10
                    function refresh() {
                        protondbToggle.setSilent(integrations.isEnabled("protondb"));
                        igdbToggle.setSilent(integrations.isEnabled("igdb"));
                    }
                    Text { text: "Integrations"; color: Theme.textMain; font.bold: true; font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                width: parent.width }
                    Text {
                        text: "Connect external platforms. Local scans need no API key."
                        color: Theme.textSec
                        font.pixelSize: 11
                        wrapMode: Text.Wrap
                        width: parent.width
                    }
                    // Steam
                    CCard {
                        width: parent.width
                        Row {
                            width: parent.width
                            Text {
                                text: "Steam"
                                color: Theme.textMain
                                font.bold: true
                                font.pixelSize: 13
                                width: parent.width - 150
                            }
                            CButton {
                                text: "Scan & Import"
                                width: 140
                                height: 32
                                onClicked: integrations.scanSteam()
                            }
                        }
                        Text {
                            text: "Import installed Steam games with their existing Proton prefixes"
                            color: Theme.textSec
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                            width: parent.width
                        }
                    }
                    // Lutris
                    CCard {
                        width: parent.width
                        Row {
                            width: parent.width
                            Text {
                                text: "Lutris"
                                color: Theme.textMain
                                font.bold: true
                                font.pixelSize: 13
                                width: parent.width - 150
                            }
                            CButton {
                                text: "Scan & Import"
                                width: 140
                                height: 32
                                onClicked: integrations.scanLutris()
                            }
                        }
                        Text {
                            text: "Largest Linux gaming platform – scan installed Lutris games"
                            color: Theme.textSec
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                            width: parent.width
                        }
                    }
                    // ProtonDB toggle
                    CCard {
                        width: parent.width
                        Row {
                            width: parent.width
                            spacing: 8
                            Text {
                                text: "ProtonDB – compatibility ratings (public API, no key)"
                                color: Theme.textMain
                                font.pixelSize: 12
                                width: parent.width - 60
                                wrapMode: Text.Wrap
                            }
                            CSwitch {
                                id: protondbToggle
                                objectName: ""
                                width: 52
                                onToggled: integrations.setEnabled("protondb", checked)
                            }
                        }
                    }
                    // IGDB key
                    CCard {
                        width: parent.width
                        Row {
                            width: parent.width
                            Text {
                                text: "IGDB"
                                color: Theme.textMain
                                font.bold: true
                                font.pixelSize: 13
                                width: parent.width - 60
                            }
                            CSwitch {
                                id: igdbToggle
                                objectName: ""
                                width: 52
                                onToggled: integrations.setEnabled("igdb", checked)
                            }
                        }
                        Text {
                            text: "Game info & ratings – needs Twitch ClientID:ClientSecret"
                            color: Theme.textSec
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                            width: parent.width
                        }
                        CTextField {
                            id: igdbField
                            width: parent.width
                            placeholderText: "ClientID:Secret"
                            Component.onCompleted: text = integrations.apiKey("igdb")
                            onEditingFinished: integrations.setApiKey("igdb", text.trim())
                        }
                    }
                }
            }

            // ABOUT
            Column {
                width: parent.width
                spacing: 8
                visible: root.page === "about"
                Item { width: 1; height: 20 }
                Image {
                    source: "qrc:/assets/qml/assets/corkytux.png"
                    sourceSize.width: 120
                    sourceSize.height: 120
                    width: 120
                    height: 120
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    anchors.horizontalCenter: parent.horizontalCenter
                }
                Text {
                    text: "CorkyTux"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 18
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    text: appVersion
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                CButton {
                    text: "GitHub"
                    iconSource: "github"
                    iconSize: 18
                    kind: "outline"
                    width: 160
                    anchors.horizontalCenter: parent.horizontalCenter
                    onClicked: Qt.openUrlExternally("https://github.com/Matts-lab69/corkytux")
                }
                Text {
                    text: "by Matts-lab69"
                    color: Theme.textMuted
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
            }
            }
        }

        // ---- tab bar (gray pill, Java hboxAlt style) ----
        Rectangle {
            width: parent.width
            height: 58
            radius: 14
            color: Theme.isLight ? "#FFFFFF" : "#181818"
            border.color: Theme.border
            border.width: 1
        Row {
            anchors.fill: parent
            anchors.margins: 3
            spacing: 0
            Repeater {
                model: [
                    { "id": "visuals", "label": "Visuals", "icon": "palette" },
                    { "id": "paths", "label": "Paths", "icon": "fileview" },
                    { "id": "protons", "label": "Protons", "icon": "proton17" },
                    { "id": "misc", "label": "Misc", "icon": "settings" },
                    { "id": "plugins", "label": "Plugins", "icon": "plugins" },
                    { "id": "integrations", "label": "Integrations", "icon": "openIn" },
                    { "id": "about", "label": "About", "icon": "about" }
                ]
                Button {
                    required property var modelData
                    text: modelData.label
                    width: parent.width / root.tabCount
                    height: 52
                    checkable: true
                    checked: root.page === modelData.id
                    autoExclusive: true
                    background: null
                    icon.name: ""
                    contentItem: Column {
                        spacing: 1
                        CIcon {
                            iconName: modelData.icon
                            iconSize: 18
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                        Text {
                            text: modelData.label
                            color: root.page === modelData.id ? Theme.accentText : Theme.textSec
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
                            visible: root.page === modelData.id
                        }
                    }
                    onClicked: root.setPage(modelData.id)
                }
            }
        }
        }
    }

    signal notice(string message)
}

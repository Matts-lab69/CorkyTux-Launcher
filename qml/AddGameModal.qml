import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "components"

// AddGameModal – mirrors Java newGameConfigurator: candidate list with
// 50px two-line rows (name + directory), Next, then grouped meta form
// (name / install path / prefix path), clean-up toggle, Add/Cancel.
// RAR candidates extract via unrar/7z before adding (old RarExtractor flow).
// If emulator-manager is installed, first step asks for executor type.
CModal {
    id: root
    title: "Add Game"
    boxWidth: 440

    property var candidates: []
    property string pickedDir: ""
    property bool busy: false
    property bool nameDirty: false // true once the user typed a custom name
    // Executor selection
    property string selectedExecutor: "" // "" = Wine/Proton, or emulator name
    property var installedEmulators: []
    property var emulatorExtensions: []
    // Extension filter per emulator (fallback: show all)
    readonly property var emulatorExtMap: {
        "melonDS": ["nds", "srl", "dsi"],
        "Dolphin": ["iso", "gcm", "wbfs", "rvz", "wia"],
        "Mupen64Plus": ["z64", "n64", "v64", "rom"],
        "PCSX2": ["iso", "bin", "img", "mdf", "nrg"],
        "PPSSPP": ["iso", "cso"],
        "RPCS3": ["pkg", "iso"],
        "Ryujinx": ["nca", "xci", "nsp"],
        "Vita3K": ["vpk"],
        "Azahar": ["3ds", "cia", "3dsx"],
        "DuckStation": ["iso", "bin", "cue", "img", "mdf"],
        "Cemu": ["wud", "wua", "rpx", "wux"],
        "Desmume": ["nds", "srl", "dsi"],
    }

    function baseNameOf(path) {
        var base = String(path).split("/").pop();
        return base.indexOf(".") >= 0 ? base.substring(0, base.lastIndexOf(".")) : base;
    }

    function hasEmulators() {
        return plugins.isEmulatorManagerInstalled() && installedEmulators.length > 0;
    }

    function reset() {
        // NEVER assign candList.model imperatively: it kills the
        // 'model: root.candidates' binding forever (silent QML semantics).
        // Drive the view only through root.candidates.
        root.candidates = [];
        pickedDir = "";
        busy = false;
        statusText.text = "";
        candList.currentIndex = -1;
        metaName.text = "";
        metaPrefix.text = "";
        nameDirty = false;
        selectedExecutor = "";
        emulatorExtensions = [];
        metaBox.visible = false;
        pickBox.visible = true;
        executorBox.visible = false;
    }
    onOpened: {
        // Always load emulators when modal opens
        plugins.listEmulators();
        reset();
    }
    // When emulators arrive, show executor step if we have some
    onInstalledEmulatorsChanged: {
        if (installedEmulators.length > 0 && selectedExecutor === "" && !metaBox.visible) {
            pickBox.visible = false;
            executorBox.visible = true;
        }
    }

    function proceedAdd(mainFile, gameDir) {
        var gname = metaName.text.trim();
        if (gname === "")
            return;
        var prefix = metaPrefix.text.trim();
        var fields = {
            "executable": mainFile,
            "mainPath": gameDir,
        };
        if (root.selectedExecutor !== "") {
            // Emulator game
            fields["executor"] = root.selectedExecutor;
        } else {
            // Wine/Proton game
            if (prefix === "")
                prefix = config.basePathFor("prefixes") + "/" + gname;
            fields["prefixPath"] = prefix;
        }
        games.addGame(gname, fields);
        // Always fetch artwork and plugins scan
        plugins.applyScanPlugins(gname, gameDir);
        integrations.resolveArtwork(gname, "");
        if (root.selectedExecutor === "" && mainFile.toLowerCase().endsWith(".exe"))
            integrations.extractExeIconAsync(mainFile, gname);
        root.close();
    }

    Column {
        width: parent.width
        spacing: 10

        // ---- Executor Selection ----
        Column {
            id: executorBox
            width: parent.width
            spacing: 10
            visible: false
            Text {
                text: "Select executor"
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 14
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
            }
            Text {
                text: "How do you want to run this game?"
                color: Theme.textSec
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
            }
            // Wine/Proton option
            Rectangle {
                width: parent.width
                height: 50
                radius: 8
                color: root.selectedExecutor === "" ? Theme.accent
                     : executorWineMouse.containsMouse ? Theme.hover : Theme.well
                Column {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 8
                    anchors.topMargin: 5
                    anchors.bottomMargin: 5
                    Text {
                        text: "Wine / Proton"
                        color: root.selectedExecutor === "" ? "#000000" : Theme.textMain
                        font.bold: true
                        font.pixelSize: 13
                    }
                    Text {
                        text: "Run .exe / .msi games via Proton or Wine"
                        color: root.selectedExecutor === "" ? "#000000" : Theme.textSec
                        font.pixelSize: 11
                    }
                }
                MouseArea {
                    id: executorWineMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        root.selectedExecutor = "";
                        executorBox.visible = false;
                        pickBox.visible = true;
                    }
                }
            }
            // Emulator options
            Repeater {
                model: root.installedEmulators
                Rectangle {
                    required property var modelData
                    width: parent.width
                    height: 50
                    radius: 8
                    color: root.selectedExecutor === modelData.name ? Theme.accent
                         : emuMouse.containsMouse ? Theme.hover : Theme.well
                    Column {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 8
                        anchors.topMargin: 5
                        anchors.bottomMargin: 5
                        Text {
                            text: modelData.name
                            color: root.selectedExecutor === modelData.name ? "#000000" : Theme.textMain
                            font.bold: true
                            font.pixelSize: 13
                        }
                        Text {
                            text: modelData.description + " — " + (modelData.extensions || []).join(", ")
                            color: root.selectedExecutor === modelData.name ? "#000000" : Theme.textSec
                            font.pixelSize: 11
                        }
                    }
                    MouseArea {
                        id: emuMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            root.selectedExecutor = modelData.name;
                            root.emulatorExtensions = modelData.extensions || [];
                            executorBox.visible = false;
                            pickBox.visible = true;
                        }
                    }
                }
            }
        }

        Column {
            id: pickBox
            width: parent.width
            spacing: 10
            Text {
                text: root.selectedExecutor !== "" ? "Select " + root.selectedExecutor + " ROM" : "Select the main game file"
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 14
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
            }
            CButton {
                text: "Choose Folder…"
                kind: "outline"
                width: parent.width
                onClicked: folderDialog.open()
            }
            ListView {
                id: candList
                objectName: "candList"
                width: parent.width
                height: 220
                clip: true
                model: root.candidates
                onCountChanged: console.log("[AddGame] list count: " + count)
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    width: candList.width
                    height: 50
                    radius: 8
                    Component.onCompleted: console.log("[AddGame] row: " + modelData)
                    color: candList.currentIndex === index ? Theme.accent
                         : candMouse.containsMouse ? Theme.hover : Theme.well
                    Column {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 8
                        anchors.topMargin: 5
                        anchors.bottomMargin: 5
                        spacing: 0
                        Text {
                            text: modelData.split("/").pop() || modelData
                            color: candList.currentIndex === index ? "#000000" : Theme.textMain
                            font.bold: true
                            font.pixelSize: 12
                            elide: Text.ElideMiddle
                            width: parent.width
                        }
                        Text {
                            text: modelData.substring(0, Math.max(0, modelData.lastIndexOf("/")))
                            color: candList.currentIndex === index ? "#000000" : Theme.textSec
                            font.pixelSize: 11
                            elide: Text.ElideMiddle
                            width: parent.width
                        }
                    }
                    MouseArea {
                        id: candMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            console.log("[AddGame] row clicked: " + index);
                            candList.currentIndex = index;
                            metaName.text = root.baseNameOf(modelData);
                            root.nameDirty = false;
                        }
                    }
                }
            }
            Text {
                id: statusText
                text: ""
                color: Theme.textSec
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
                visible: text !== ""
            }
                CButton {
                    text: "Next"
                    width: 144
                    height: 40
                    anchors.horizontalCenter: parent.horizontalCenter
                    enabled: !root.busy && candList.currentIndex >= 0
                    onClicked: {
                        var sel = root.candidates[candList.currentIndex];
                        if (!root.nameDirty)
                            metaName.text = root.baseNameOf(sel);
                        root.nameDirty = false;
                        var defPrefix = config.basePathFor("prefixes") + "/" + metaName.text.trim();
                        metaPrefix.text = defPrefix;
                        pickBox.visible = false;
                        metaBox.visible = true;
                    }
                }
            }
        }
        Column {
            id: metaBox
            width: parent.width
            spacing: 10
            visible: false
            Text {
                text: root.selectedExecutor !== "" ? "Adding " + root.selectedExecutor + " game" : "Adding a new game"
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 15
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
            }
            CTextField { id: metaName; width: parent.width; placeholderText: "Game name in launcher"; onTextChanged: root.nameDirty = true }
            CTextField { id: metaPath; width: parent.width; placeholderText: "Game install path"; text: root.pickedDir }
            CTextField { id: metaPrefix; width: parent.width; placeholderText: "Prefix path"; visible: root.selectedExecutor === "" }
            Row {
                width: parent.width
                spacing: 10
                CButton {
                    text: "Cancel"
                    kind: "outline"
                    width: (parent.width - 10) / 2
                    onClicked: root.close()
                }
                CButton {
                    text: "Add"
                    width: (parent.width - 10) / 2
                    enabled: !root.busy
                    onClicked: {
                        if (metaName.text.trim() === "")
                            return;
                        var mainFile = candList.currentIndex >= 0
                            ? root.candidates[candList.currentIndex] : "";
                        var gameDir = (metaPath.text.trim() !== "")
                            ? metaPath.text.trim() : root.pickedDir;
                        if (mainFile.toLowerCase().endsWith(".rar")) {
                            root.busy = true;
                            statusText.text = "Extracting archive…";
                            pickBox.visible = true;
                            metaBox.visible = false;
                            integrations.extractRarAsync(
                                mainFile, gameDir + "/" + metaName.text.trim() + "_unpacked");
                            return;
                        }
                        root.proceedAdd(mainFile, gameDir);
                    }
                }
            }

        FolderDialog {
            id: folderDialog
            title: "Select game folder"
            onAccepted: {
                // decodeURIComponent: folders with spaces arrive %20-encoded
                var raw = decodeURIComponent(String(selectedFolder));
                console.log("[AddGame] folder picked: " + raw);
                root.pickedDir = raw.replace("file://", "");
                root.candidates = [];
                candList.currentIndex = -1;
                scanWorker.scan(root.pickedDir);
            }
        }

        QtObject {
            id: scanWorker
            function scan(dir) {
                console.log("[AddGame] scanning: " + dir);
                var all;
                // Use extension-specific scan for emulators
                if (root.emulatorExtensions.length > 0) {
                    all = integrations.scanDirForExtensions(dir, root.emulatorExtensions);
                } else {
                    all = integrations.scanDirSync(dir);
                }
                root.candidates = all;
                console.log("[AddGame] candidates: " + root.candidates.length);
                // pre-select first row so Next is immediately usable
                // (user can pick another row; always visible via highlight)
                if (root.candidates.length > 0)
                    candList.currentIndex = 0;
            }
        }

        Connections {
            target: integrations
            function onRarReady(dest, exe) {
                root.busy = false;
                statusText.text = "";
                root.proceedAdd(exe, dest);
            }
            function onRarFailed(msg) {
                root.busy = false;
                statusText.text = "Extraction failed: " + msg;
                pickBox.visible = true;
                metaBox.visible = false;
            }
        }

        Connections {
            target: plugins
            function onEmulatorsChanged() {
                root.installedEmulators = plugins.emulators.filter(function(e) { return e.installed; });
            }
        }
    }
}

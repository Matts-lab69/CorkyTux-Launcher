import QtQuick
import QtQuick.Controls
import "components"

// ProtonModal – GE-Proton/CachyOS release list + download progress.
// Shows a path picker when the user clicks "Get".
CModal {
    id: root
    title: "Download Proton"
    boxWidth: 440

    property string pendingTag: ""
    property string pendingUrl: ""
    property bool showPathPicker: false

    Column {
        width: parent.width
        spacing: 10

        // Path picker (shown before download)
        Column {
            width: parent.width
            spacing: 8
            visible: root.showPathPicker
            Text {
                text: "Install " + root.pendingTag + " to:"
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 13
                width: parent.width
                wrapMode: Text.Wrap
            }
            Repeater {
                model: proton.protonPaths()
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    width: relList.width - 8
                    height: 44
                    radius: 8
                    color: pathMouse.containsMouse ? Theme.hover : Theme.well
                    border.color: Theme.border
                    border.width: 1
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 8
                        CIcon {
                            iconName: "folder"
                            iconSize: 16
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 24
                            Text {
                                text: modelData.split("/").pop() || modelData
                                color: Theme.textMain
                                font.pixelSize: 12
                                font.bold: true
                                elide: Text.ElideMiddle
                                width: parent.width
                            }
                            Text {
                                text: modelData
                                color: Theme.textSec
                                font.pixelSize: 10
                                elide: Text.ElideMiddle
                                width: parent.width
                            }
                        }
                    }
                    MouseArea {
                        id: pathMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.showPathPicker = false;
                            statusLabel.text = "Downloading " + root.pendingTag;
                            proton.downloadProton(root.pendingTag, root.pendingUrl, modelData);
                        }
                    }
                }
            }
            CButton {
                text: "Cancel"
                kind: "outline"
                width: parent.width
                onClicked: root.showPathPicker = false
            }
        }

        // Normal view (progress + release list)
        Column {
            width: parent.width
            spacing: 10
            visible: !root.showPathPicker

            Text {
                id: statusLabel
                text: "Downloading"
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 13
                horizontalAlignment: Text.AlignHCenter
                width: parent.width
            }
            ProgressBar {
                id: bar
                width: parent.width
                value: proton.downloadProgress < 0 ? 0 : proton.downloadProgress
                background: Rectangle {
                    color: Theme.isLight ? "#E9ECEF" : "#3E3E3E"
                    radius: 4
                }
                contentItem: Item {
                    Rectangle {
                        width: parent.width * bar.visualPosition
                        height: parent.height
                        radius: 4
                        color: Theme.accent
                    }
                }
            }
            ListView {
                id: relList
                width: parent.width
                height: 220
                clip: true
                spacing: 4
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle { implicitWidth: 3; radius: 2; color: Theme.accent }
                    background: Rectangle { implicitWidth: 3; color: "transparent" }
                }
                delegate: Rectangle {
                    required property var modelData
                    width: relList.width - 8
                    height: 40
                    radius: 8
                    color: Theme.well
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 8
                        Text {
                            text: modelData.tag || ""
                            color: Theme.textMain
                            font.pixelSize: 12
                            width: parent.width - 120
                            elide: Text.ElideMiddle
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        CButton {
                            text: "Get"
                            width: 96
                            height: 28
                            anchors.verticalCenter: parent.verticalCenter
                            onClicked: {
                                var paths = proton.protonPaths();
                                // Always show path picker
                                root.pendingTag = modelData.tag;
                                root.pendingUrl = modelData.url;
                                root.showPathPicker = true;
                            }
                        }
                    }
                }
            }
            Row {
                width: parent.width
                spacing: 10
                CButton {
                    text: "Refresh"
                    kind: "outline"
                    width: (parent.width - 10) / 2
                    onClicked: proton.fetchReleases()
                }
                CButton {
                    text: "Close"
                    kind: "outline"
                    width: (parent.width - 10) / 2
                    onClicked: root.close()
                }
            }
        }

        Connections {
            target: proton
            function onReleasesReady(list) {
                relList.model = list;
            }
            function onDownloadFinished(ok, message) {
                statusLabel.text = ok ? ("Installed " + message) : ("Failed: " + message);
            }
        }
    }
    onOpened: {
        showPathPicker = false;
        proton.fetchReleases();
    }
}

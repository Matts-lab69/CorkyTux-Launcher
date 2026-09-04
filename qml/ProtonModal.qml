import QtQuick
import QtQuick.Controls
import "components"

// ProtonModal – GE-Proton release list + download progress.
// Mirrors Java ProtonDownloader (progress bar + label).
CModal {
    id: root
    title: "Download Proton"
    boxWidth: 440

    Column {
        width: parent.width
        spacing: 10
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
            delegate: Rectangle {
                required property var modelData
                width: relList.width
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
                            statusLabel.text = "Downloading " + modelData.tag;
                            proton.downloadProton(modelData.tag, modelData.url);
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
    onOpened: proton.fetchReleases()
}

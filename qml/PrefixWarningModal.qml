import QtQuick
import QtQuick.Controls
import "components"

// PrefixWarningModal – warns about games with missing prefix paths after import.
CModal {
    id: root
    title: "Warning"
    boxWidth: 520

    property var missingGames: []

    function openFor(games) {
        missingGames = games;
        open();
    }

    Column {
        width: parent.width
        spacing: 12
        Text {
            text: "The following games have prefix paths that no longer exist:"
            color: Theme.textMain
            font.bold: true
            font.pixelSize: 13
            wrapMode: Text.Wrap
            width: parent.width
        }
        Rectangle {
            width: parent.width
            height: Math.min(220, missingGamesCol.height + 16)
            radius: 8
            color: Theme.isLight ? "#FFF3CD" : "#332B00"
            border.color: Theme.isLight ? "#FFCC02" : "#665500"
            border.width: 1
            Flickable {
                anchors.fill: parent
                anchors.margins: 8
                contentHeight: missingGamesCol.height
                clip: true
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle { implicitWidth: 3; radius: 2; color: Theme.accent }
                }
                Column {
                    id: missingGamesCol
                    width: parent.width - 8
                    spacing: 6
                    Repeater {
                        model: root.missingGames
                        Row {
                            required property var modelData
                            spacing: 8
                            Rectangle {
                                width: 6
                                height: 6
                                radius: 3
                                color: "#FFCC02"
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            Column {
                                width: parent.width - 14
                                spacing: 2
                                Text {
                                    text: modelData.name
                                    color: Theme.textMain
                                    font.bold: true
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                    width: parent.width
                                }
                                Text {
                                    text: modelData.prefixPath || "(no prefix configured)"
                                    color: Theme.textSec
                                    font.pixelSize: 10
                                    elide: Text.ElideMiddle
                                    width: parent.width
                                }
                            }
                        }
                    }
                }
            }
        }
        Text {
            text: "These games were imported but may not launch until you create the prefix manually or update the prefix path in Game Settings."
            color: Theme.textSec
            font.pixelSize: 11
            wrapMode: Text.Wrap
            width: parent.width
        }
        CButton {
            text: "OK"
            width: 100
            height: 34
            anchors.horizontalCenter: parent.horizontalCenter
            onClicked: root.close()
        }
    }
}

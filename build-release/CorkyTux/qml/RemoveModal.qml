import QtQuick
import QtQuick.Controls
import "components"

// RemoveModal – compact confirm (Yes green / No red). Mirrors gameRemover.
CModal {
    id: root
    title: "Remove Game"
    boxWidth: 420
    signal removed(string name)

    property string gameName: ""
    property bool isEmulator: false

    function openFor(name, emulator) {
        gameName = name;
        isEmulator = emulator || false;
        open();
    }

    Column {
        width: parent.width
        spacing: 12
        Text {
            text: "Remove " + root.gameName + " from launcher?"
            color: Theme.textMain
            font.bold: true
            font.pixelSize: 13
            wrapMode: Text.Wrap
            width: parent.width
        }
        Column {
            width: parent.width
            spacing: 8
            CheckBox {
                id: prefixBox
                text: "Remove game prefix"
                checked: true
                visible: !root.isEmulator
                contentItem: Text {
                    text: prefixBox.text
                    color: Theme.textMain
                    font.pixelSize: 12
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: prefixBox.indicator.width + 8
                }
                indicator: Rectangle {
                    x: prefixBox.leftPadding
                    y: parent.height / 2 - height / 2
                    width: 18
                    height: 18
                    radius: 4
                    border.color: Theme.border
                    border.width: 1
                    color: prefixBox.checked ? Theme.accent : Theme.well
                    Rectangle {
                        width: 10
                        height: 10
                        radius: 2
                        color: "#FFFFFF"
                        anchors.centerIn: parent
                        visible: prefixBox.checked
                    }
                }
            }
            CheckBox {
                id: filesBox
                text: "Remove game from disk"
                checked: false
                contentItem: Text {
                    text: filesBox.text
                    color: Theme.textMain
                    font.pixelSize: 12
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: filesBox.indicator.width + 8
                }
                indicator: Rectangle {
                    x: filesBox.leftPadding
                    y: parent.height / 2 - height / 2
                    width: 18
                    height: 18
                    radius: 4
                    border.color: Theme.border
                    border.width: 1
                    color: filesBox.checked ? Theme.accent : Theme.well
                    Rectangle {
                        width: 10
                        height: 10
                        radius: 2
                        color: "#FFFFFF"
                        anchors.centerIn: parent
                        visible: filesBox.checked
                    }
                }
            }
        }
        Row {
            width: parent.width
            spacing: 10
            layoutDirection: Qt.RightToLeft
            Rectangle {
                width: 100
                height: 34
                radius: 20
                color: yesBtn.pressed ? "#00CC44" : (yesBtn.hovered ? "#00FF55" : "#00E639")
                Text {
                    text: "Yes"
                    color: "#FFFFFF"
                    font.bold: true
                    font.pixelSize: 13
                    anchors.centerIn: parent
                }
                MouseArea {
                    id: yesBtn
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        games.removeGameFull(root.gameName, prefixBox.checked, filesBox.checked);
                        root.removed(root.gameName);
                        root.close();
                    }
                }
            }
            CButton {
                text: "No"
                kind: "danger"
                width: 100
                height: 34
                onClicked: root.close()
            }
        }
    }
}

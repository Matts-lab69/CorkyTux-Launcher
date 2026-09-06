import QtQuick
import QtQuick.Controls
import "components"

// LogModal – game log viewer with Save/Close/GitHub buttons.
// Mirrors Java LogForm (issues -> repo).
CModal {
    id: root
    title: "Game Log"
    boxWidth: 640

    property alias logText: area.text

    Column {
        width: parent.width
        spacing: 10
        Text {
            text: "If there are errors in it, you can figure it out yourself or send the log to the community and they will help you"
            color: Theme.textSec
            font.pixelSize: 12
            wrapMode: Text.Wrap
            width: parent.width
        }
        Rectangle {
            width: parent.width
            height: 300
            radius: 8
            color: Theme.isLight ? "#F1F3F5" : "#181818"
            border.color: Theme.border
            border.width: 1
            Flickable {
                anchors.fill: parent
                anchors.margins: 8
                contentHeight: area.height
                clip: true
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle { implicitWidth: 3; radius: 2; color: Theme.accent }
                    background: Rectangle { implicitWidth: 3; color: "transparent" }
                }
                TextEdit {
                    id: area
                    width: parent.width - 8
                    readOnly: true
                    wrapMode: TextEdit.Wrap
                    color: Theme.textMain
                    font.pixelSize: 12
                    selectByMouse: true
                }
            }
        }
        Row {
            width: parent.width
            spacing: 10
            CButton {
                text: "GitHub"
                kind: "outline"
                width: 112
                onClicked: Qt.openUrlExternally("https://github.com/Matts-lab69/corkytux/issues")
            }
            Item { width: 1; height: 1 }
            CButton {
                text: "Close"
                kind: "danger"
                width: 112
                onClicked: root.close()
            }
            CButton {
                text: "Save"
                width: 112
                onClicked: {
                    var dir = config.dataDirPath() + "/logs";
                    config.ensureDir(dir);
                    var ts = new Date().toISOString().replace(/[:.]/g, "-");
                    var path = dir + "/log-" + ts + ".txt";
                    var ok = config.saveTextFile(path, area.text);
                    if (ok) {
                        toastLabel.text = "Log saved to: " + path;
                        toastPopup.open();
                    }
                }
            }
        }
    }
}

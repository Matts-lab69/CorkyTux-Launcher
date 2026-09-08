import QtQuick
import QtQuick.Controls

// CModal – centered dialog card over a dim overlay. Body via default property.
Popup {
    id: root
    default property alias body: bodyCol.children
    property alias title: titleLabel.text
    property int boxWidth: 440
    anchors.centerIn: Overlay.overlay
    modal: true
    focus: true
    closePolicy: Popup.CloseOnEscape
    padding: 0
    background: Rectangle {
        color: Theme.panel
        radius: 12
        border.color: Theme.border
        border.width: 1
    }
    contentItem: Column {
        width: root.boxWidth
        spacing: 0
        Row {
            width: parent.width
            height: 56
            spacing: 12
            leftPadding: 24
            rightPadding: 16
            Text {
                id: titleLabel
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 88
                color: Theme.textMain
                font.bold: true
                font.pixelSize: 18
                elide: Text.ElideRight
            }
            CButton {
                text: "✕"
                kind: "accenticon"
                width: 32
                height: 32
                anchors.verticalCenter: parent.verticalCenter
                onClicked: root.close()
            }
        }
        Rectangle {
            width: parent.width
            height: 1
            color: Theme.border
        }
        Column {
            id: bodyCol
            width: parent.width - 48
            anchors.horizontalCenter: parent.horizontalCenter
            topPadding: 20
            bottomPadding: 20
            spacing: 10
        }
    }
}

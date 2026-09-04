import QtQuick

// GameCard – 224px tile: banner, icon + name. Mirrors Prototypes.createPanel.
Rectangle {
    id: root
    property string name: ""
    property string banner: ""
    property string icon: ""
    signal clicked

    width: 224
    height: 140
    radius: 15
    color: Theme.card
    border.color: Theme.border
    border.width: Theme.isLight ? 1 : 0

    Column {
        anchors.fill: parent
        spacing: 0
        Image {
            width: 224
            height: 140
            source: banner ? ("file://" + banner) : ""
            fillMode: Image.PreserveAspectCrop
            smooth: true
            Rectangle {
                anchors.fill: parent
                radius: 15
                color: "transparent"
                border.color: Theme.border
                border.width: 0
            }
            Rectangle {
                // bottom label strip: real QColor w/ adaptive alpha.
                // (Never hex-concat alpha: QColor reads 8-digit hex as
                // #AARRGGBB, which turned yellow translucent into opaque pink.)
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 48
                color: Theme.accentStripColor
                Row {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    spacing: 4
                    Image {
                        width: 34
                        height: 34
                        anchors.verticalCenter: parent.verticalCenter
                        source: icon ? ("file://" + icon) : ""
                        fillMode: Image.PreserveAspectFit
                        visible: status === Image.Ready
                    }
                    Text {
                        text: root.name
                        color: "#FFFFFF"
                        font.bold: true
                        font.pixelSize: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: 176
                        elide: Text.ElideRight
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                    }
                }
            }
        }
    }
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
        onContainsMouseChanged: root.color = containsMouse ? Theme.hover : Theme.card
    }
}

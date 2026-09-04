import QtQuick

// CCard – rounded panel following the active theme. Auto-heights to content.
Item {
    id: root
    property int corner: 8
    default property alias content: inner.children
    implicitHeight: inner.implicitHeight + 28
    height: implicitHeight
    Rectangle {
        anchors.fill: parent
        color: Theme.panel
        radius: corner
        border.color: Theme.isLight ? Theme.border : "transparent"
        border.width: Theme.isLight ? 1 : 0
    }
    Column {
        id: inner
        width: parent.width - 28
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 14
        spacing: 12
    }
}

import QtQuick

// CCard – rounded panel following the active theme. Auto-heights to content.
Item {
    id: root
    property int corner: 8
    property color outlineColor: "transparent"
    property int outlineWidth: 0
    property color panelColor: Theme.panel
    property alias spacing: inner.spacing
    default property alias content: inner.children
    implicitHeight: inner.implicitHeight + 28
    height: implicitHeight
    Rectangle {
        anchors.fill: parent
        color: root.panelColor
        radius: corner
        border.color: root.outlineWidth > 0 ? root.outlineColor
                             : (Theme.isLight ? Theme.border : "transparent")
        border.width: root.outlineWidth > 0 ? root.outlineWidth : (Theme.isLight ? 1 : 0)
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

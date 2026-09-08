import QtQuick

// CSwitch – accent toggle. Mirrors Java SwitchComponent (logic in toggled()).
Item {
    id: root
    property bool checked: false
    signal toggled
    width: parent ? parent.width : 120
    height: 24

    function setSilent(v) { checked = v; }

    Text {
        id: labelItem
        text: root.objectName
        color: Theme.textMain
        font.pixelSize: 12
        anchors.left: parent.left
        anchors.right: track.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        elide: Text.ElideRight
    }
    Item {
        id: track
        width: 44
        height: 24
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        Rectangle {
            anchors.fill: parent
            radius: 12
            color: root.checked ? Theme.accent : (Theme.isLight ? "#CED4DA" : "#383838")
            Behavior on color { ColorAnimation { duration: 150 } }
        }
        Rectangle {
            id: thumb
            width: 18
            height: 18
            radius: 9
            color: "#FFFFFF"
            anchors.verticalCenter: parent.verticalCenter
            x: root.checked ? parent.width - width - 3 : 3
            Behavior on x { NumberAnimation { duration: 150 } }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.checked = !root.checked;
                root.toggled();
            }
        }
    }
}

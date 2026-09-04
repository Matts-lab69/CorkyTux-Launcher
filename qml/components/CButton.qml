import QtQuick
import QtQuick.Controls

// CButton – primary / outline / icon action button, theme + accent aware.
Button {
    id: root
    property string kind: "primary" // primary | outline | action | icon | danger | accenticon
    property int corner: kind === "icon" ? 14 : 20
    property string iconSource: "" // CIcon name (themed) — empty = text only
    property int iconSize: 15
    property bool iconThemed: true
    property int iconDy: 0 // vertical nudge for optical centering
    property int fontSize: -1 // -1 = auto (13 normal, 11 action)

    background: Rectangle {
        radius: root.corner
        color: {
            if (!root.enabled) return Theme.isLight ? "#E9ECEF" : "#333333";
            switch (root.kind) {
            case "outline": return "transparent";
            case "danger": return root.pressed ? "#B02A37" : (root.hovered ? "#BB2D3B" : Theme.danger);
            case "action": return root.hovered ? Theme.hover : (Theme.isLight ? "#F1F3F5" : "#242424");
            case "icon": return root.hovered ? Theme.hover : "transparent";
            case "accenticon": return root.hovered ? Theme.hover : "transparent";
            default: return root.pressed ? Theme.accentPressed : (root.hovered ? Theme.accentHover : Theme.accent);
            }
        }
        border.color: root.kind === "outline" ? Theme.accent : "transparent"
        border.width: root.kind === "outline" ? 1.5 : 0
    }
    contentItem: Item {
        implicitWidth: centerRow.visible ? centerRow.implicitWidth : actionRow.implicitWidth
        implicitHeight: centerRow.visible ? centerRow.implicitHeight : actionRow.implicitHeight
        Row {
            id: centerRow
            spacing: root.iconSource === "" ? 0 : 8
            anchors.centerIn: parent
            visible: root.kind !== "action"
            CIcon {
                iconName: root.iconSource
                iconSize: root.iconSize
                themed: root.iconThemed
                visible: root.iconSource !== ""
                transform: Translate { y: root.iconDy }
            }
            Text {
                text: root.text
                color: (root.kind === "primary" || root.kind === "danger") ? "#FFFFFF"
                     : root.kind === "accenticon" ? Theme.accentText : Theme.textMain
                font.bold: true
                font.pixelSize: root.fontSize > 0 ? root.fontSize : 13
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
        Row {
            id: actionRow
            spacing: 8
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            visible: root.kind === "action"
            CIcon {
                iconName: root.iconSource
                iconSize: root.iconSize
                themed: root.iconThemed
                visible: root.iconSource !== ""
                transform: Translate { y: root.iconDy }
            }
            Text {
                text: root.text
                color: Theme.textMain
                font.bold: true
                font.pixelSize: root.fontSize > 0 ? root.fontSize : 11
                verticalAlignment: Text.AlignVCenter
            }
        }
    }
    padding: 10
}

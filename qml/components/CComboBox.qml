import QtQuick
import QtQuick.Controls

// CComboBox – gray in light mode, dark well in dark mode. Mirrors Java combos.
ComboBox {
    id: root
    font.pixelSize: 12
    background: Rectangle {
        radius: 8
        color: Theme.isLight ? "#E9ECEF" : "#242424"
        border.color: Theme.border
        border.width: 1
    }
    contentItem: Text {
        text: root.displayText
        color: Theme.textMain
        font.pixelSize: 12
        verticalAlignment: Text.AlignVCenter
        leftPadding: 12
        elide: Text.ElideRight
    }
    popup: Popup {
        y: root.height + 4
        width: root.width
        padding: 4
        background: Rectangle {
            radius: 8
            color: Theme.isLight ? "#FFFFFF" : "#242424"
            border.color: Theme.border
            border.width: 1
        }
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: root.popup.visible ? root.delegateModel : null
            currentIndex: root.highlightedIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }
    }
    delegate: ItemDelegate {
        width: root.width
        contentItem: Text {
            text: modelData
            color: Theme.textMain
            font.pixelSize: 12
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 6
            color: highlighted ? Theme.hover : "transparent"
        }
        highlighted: ListView.isCurrentItem
    }
}

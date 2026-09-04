import QtQuick
import QtQuick.Controls

// CTextField – themed input (mirrors .text-input).
TextField {
    id: root
    color: Theme.textMain
    placeholderTextColor: Theme.isLight ? "#ADB5BD" : "#A7A7A7"
    font.pixelSize: 12
    background: Rectangle {
        color: Theme.isLight ? "#F1F3F5" : "#3E3E3E"
        radius: 6
        border.color: root.activeFocus ? Theme.accent : "transparent"
        border.width: 1
    }
    padding: 8
}

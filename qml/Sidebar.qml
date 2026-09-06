import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "components"

// Sidebar – YOUR LIBRARY: 2x2 filter grid (single selection), live search,
// scrollable game list. Mirrors Java sidebar + applyFilter/applySearchFilter.
Rectangle {
    id: root
    signal gameClicked(string name)
    color: Theme.bg

    Column {
        anchors.fill: parent
        anchors.margins: 8
        anchors.topMargin: 24
        spacing: 8

        CCard {
            id: headerCard
            width: parent.width
            height: libansion.implicitHeight + 24
            outlineColor: Theme.accent
            outlineWidth: 1
            panelColor: Theme.bg
            Column {
                id: libansion
                width: parent.width
                spacing: 12
                Text {
                    text: "Your Library"
                    color: Theme.textMain
                    font.bold: true
                    font.pixelSize: 15
                }
                GridLayout {
                    id: filterGrid
                    width: parent.width
                    columns: 2
                    rowSpacing: 6
                    columnSpacing: 6
                    Repeater {
                        model: [
                            { "id": "all", "label": "All" },
                            { "id": "favorites", "label": "Favorites" },
                            { "id": "az", "label": "A-Z" },
                            { "id": "mostplayed", "label": "Most Played" },
                            { "id": "recent", "label": "Recently Added", "span": true }
                        ]
                        Button {
                            required property var modelData
                            text: modelData.label
                            Layout.fillWidth: true
                            Layout.columnSpan: modelData.span === true ? 2 : 1
                            checkable: true
                            checked: library.mode === modelData.id
                            autoExclusive: true
                            font.pixelSize: 12
                            background: Rectangle {
                                radius: 8
                                color: parent.checked ? (Theme.isLight ? "#FFFFFF" : "#242424")
                                                     : (Theme.isLight ? "#F1F3F5" : "#1E1E1E")
                                border.color: parent.checked ? Theme.accent : "transparent"
                                border.width: 1
                            }
                            contentItem: Text {
                                text: parent.text
                                color: parent.checked ? Theme.accentText : Theme.textMain
                                font.bold: parent.checked
                                font.pixelSize: 12
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                            }
                            onClicked: library.mode = modelData.id
                        }
                    }
                }
                Row {
                    id: searchRow
                    width: parent.width
                    spacing: 6
                    Rectangle {
                        width: parent.width
                        height: 32
                        radius: 16
                        color: Theme.isLight ? "#F1F3F5" : "#242424"
                        Row {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            spacing: 6
                            CIcon {
                                iconName: "search"
                                iconSize: 14
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            TextField {
                                id: searchField
                                width: parent.width - 40
                                anchors.verticalCenter: parent.verticalCenter
                                placeholderText: "Search in your library"
                                background: Item {}
                                color: Theme.textMain
                                placeholderTextColor: Theme.textMuted
                                font.pixelSize: 12
                                onTextChanged: library.search = text
                            }
                        }
                    }
                }
            }
        }

        Rectangle {
            id: gameListFrame
            width: parent.width
            height: parent.height - headerCard.height - 56
            radius: 10
            color: "transparent"
            border.color: Theme.accent
            border.width: 1
            ListView {
                id: gameList
                anchors.fill: parent
                anchors.topMargin: 1
                anchors.bottomMargin: 1
                anchors.leftMargin: 1
                anchors.rightMargin: 1
                clip: true
                model: library
                spacing: 2
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle { implicitWidth: 3; radius: 2; color: Theme.accent }
                    background: Rectangle { implicitWidth: 3; color: "transparent" }
                }
                delegate: Rectangle {
                    property string gameName: model.name || ""
                    property string gameIcon: model.icon || ""
                    width: gameList.width - 8
                    height: 32
                    radius: 6
                    // Solid accent on hover (mirrors sidebar-tab:selected: accent bg + black text)
                    color: mouse.containsMouse ? Theme.accent : "transparent"
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        spacing: 8
                        Image {
                            width: 24
                            height: 24
                            anchors.verticalCenter: parent.verticalCenter
                            source: gameIcon ? ("file://" + gameIcon) : ""
                            fillMode: Image.PreserveAspectFit
                            visible: status === Image.Ready
                            asynchronous: true
                            sourceSize.width: 48
                            sourceSize.height: 48
                        }
                        Text {
                            text: gameName
                            color: mouse.containsMouse ? "#000000" : Theme.textMain
                            font.pixelSize: 12
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            width: parent.width - 40
                        }
                    }
                    MouseArea {
                        id: mouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.gameClicked(gameName)
                    }
                }
            }
        }
    }
}

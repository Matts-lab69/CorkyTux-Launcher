import QtQuick

// CIcon – themed UI icon. Uses <name>_dark.png on light theme.
Image {
    id: root
    property string iconName: ""
    property int iconSize: 16
    // themed=false keeps base icon (e.g. white glyphs on accent buttons)
    property bool themed: true
    // forceDark=true always uses the _dark variant (e.g. dark sun on white button)
    property bool forceDark: false
    source: {
        if (iconName === "")
            return "";
        var suffix = (!themed || forceDark) ? (forceDark ? "_dark.png" : ".png")
                                            : (Theme.isLight ? "_dark.png" : ".png");
        return "qrc:/assets/qml/assets/" + iconName + suffix;
    }
    sourceSize.width: iconSize
    sourceSize.height: iconSize
    width: iconSize
    height: iconSize
    fillMode: Image.PreserveAspectFit
    smooth: true
}

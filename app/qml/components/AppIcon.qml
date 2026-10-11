import QtQuick

// An application icon from the system icon theme.
Item {
    id: root
    property string iconName: ""
    property int size: 24
    implicitWidth: size
    implicitHeight: size
    Image {
        anchors.fill: parent
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        source: "image://theme/" + (root.iconName || "application-x-executable")
        smooth: true
        mipmap: true
    }
}

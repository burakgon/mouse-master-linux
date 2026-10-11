import QtQuick

// A tinted Lucide icon. An Item wrapper keeps its implicit size equal to `size`
// (an Image's implicit size would follow the oversampled source).
Item {
    id: root
    property string name: "circle-dot"
    property color color: Theme.text
    property int size: 18

    implicitWidth: size
    implicitHeight: size

    Image {
        anchors.fill: parent
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        source: root.name ? "image://lucide/" + root.name + "/" + Theme.hex(root.color) : ""
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
    }
}

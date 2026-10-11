import QtQuick

// A rounded surface with a hairline border.
Rectangle {
    id: root
    property int padding: 20
    default property alias content: inner.data
    property alias contentItem: inner

    radius: Theme.radiusLarge
    color: Theme.surface
    border.color: Theme.stroke
    border.width: 1
    implicitHeight: inner.childrenRect.height + padding * 2
    implicitWidth: inner.childrenRect.width + padding * 2

    Item {
        id: inner
        anchors.fill: parent
        anchors.margins: root.padding
    }
}

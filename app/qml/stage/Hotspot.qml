import QtQuick

// A dot placed on a control of the device artwork.
Item {
    id: root
    property bool selected: false
    property bool active: false
    property bool highlighted: false
    signal clicked()

    width: 22
    height: 22

    Rectangle {
        id: ring
        anchors.centerIn: parent
        width: root.active ? 34 : root.selected || root.highlighted ? 24 : 18
        height: width
        radius: width / 2
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
        opacity: root.active ? 0.9 : root.selected || root.highlighted ? 0.8 : 0.45
        Behavior on width { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutBack } }
        Behavior on opacity { NumberAnimation { duration: Theme.anim } }
    }
    Rectangle {
        anchors.centerIn: parent
        width: root.active ? 14 : 10
        height: width
        radius: width / 2
        color: Theme.accent
        border.width: 2
        border.color: "#ffffff"
        Behavior on width { NumberAnimation { duration: Theme.animFast } }
    }
    // A soft pulse that draws attention to the selected control.
    Rectangle {
        anchors.centerIn: parent
        width: 18
        height: 18
        radius: 9
        color: Theme.accent
        opacity: 0
        visible: root.selected || root.active
        SequentialAnimation on scale {
            running: root.selected || root.active
            loops: Animation.Infinite
            NumberAnimation { from: 1; to: 2.6; duration: 1100; easing.type: Easing.OutCubic }
        }
        SequentialAnimation on opacity {
            running: root.selected || root.active
            loops: Animation.Infinite
            NumberAnimation { from: 0.35; to: 0; duration: 1100; easing.type: Easing.OutCubic }
        }
    }
    MouseArea {
        anchors.fill: parent
        anchors.margins: -8
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}

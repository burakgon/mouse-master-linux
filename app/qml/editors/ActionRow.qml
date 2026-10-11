import QtQuick

// One selectable action in the action picker.
Rectangle {
    id: root
    property string iconName: "circle-dot"
    property string label
    property string detail: ""
    property bool current: false
    property bool expandable: false
    property bool expanded: false
    signal clicked()

    width: parent ? parent.width : 300
    height: 44
    radius: 10
    color: current ? Theme.accentAlpha(Theme.dark ? 0.18 : 0.1) : hover.hovered ? Theme.surfaceHover : "transparent"
    Behavior on color { ColorAnimation { duration: Theme.animFast } }

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: root.clicked() }

    Rectangle {
        id: badge
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 30
        height: 30
        radius: 9
        color: root.current ? Theme.accent : Theme.surfaceAlt
        Icon {
            anchors.centerIn: parent
            name: root.iconName
            size: 16
            color: root.current ? Theme.accentText : Theme.textSecondary
        }
    }
    Column {
        anchors.left: badge.right
        anchors.leftMargin: 12
        anchors.right: trailing.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        Text {
            width: parent.width
            text: root.label
            color: Theme.text
            font.pixelSize: Theme.fontBody
            font.weight: root.current ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }
        Text {
            visible: root.detail !== ""
            width: parent.width
            text: root.detail
            color: Theme.textTertiary
            font.pixelSize: Theme.fontCaption
            elide: Text.ElideRight
        }
    }
    Item {
        id: trailing
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        width: 18
        height: 18
        Icon {
            anchors.centerIn: parent
            visible: root.current && !root.expandable
            name: "check"
            size: 16
            color: Theme.accent
        }
        Icon {
            anchors.centerIn: parent
            visible: root.expandable
            name: root.expanded ? "chevron-up" : "chevron-down"
            size: 16
            color: Theme.textTertiary
        }
    }
}

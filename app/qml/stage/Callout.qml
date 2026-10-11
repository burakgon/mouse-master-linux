import QtQuick

// A label card describing what a control does.
Rectangle {
    id: root
    property string title
    property string label
    property string iconName: "circle-dot"
    property bool selected: false
    property bool active: false      // the physical control is held right now
    property bool inherited: false   // shown from the default profile
    property bool compact: false
    signal clicked()

    implicitWidth: compact ? 44 : 214
    implicitHeight: compact ? 44 : 58
    radius: compact ? 22 : 14
    color: selected ? Theme.accentAlpha(Theme.dark ? 0.18 : 0.1) : hover.hovered ? Theme.surfaceHover : Theme.surface
    border.width: selected || active ? 2 : 1
    border.color: active ? Theme.accent : selected ? Theme.accentAlpha(0.7) : Theme.stroke
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    scale: active ? 1.04 : 1
    Behavior on scale { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutBack } }

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: root.clicked() }

    Rectangle {
        id: badge
        x: root.compact ? (root.width - width) / 2 : 10
        anchors.verticalCenter: parent.verticalCenter
        width: 32
        height: 32
        radius: 10
        color: root.active || root.selected ? Theme.accent : Theme.accentAlpha(Theme.dark ? 0.16 : 0.12)
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
        Icon {
            anchors.centerIn: parent
            name: root.iconName
            size: 17
            color: root.active || root.selected ? Theme.accentText : Theme.accent
        }
    }
    Column {
        visible: !root.compact
        anchors.left: badge.right
        anchors.leftMargin: 10
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Text {
            width: parent.width
            text: root.title
            color: Theme.textTertiary
            font.pixelSize: Theme.fontCaption
            font.weight: Font.Medium
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: root.label
            color: root.inherited ? Theme.textSecondary : Theme.text
            font.pixelSize: Theme.fontSmall + 1
            font.weight: Font.DemiBold
            font.italic: root.inherited
            elide: Text.ElideRight
        }
    }
}

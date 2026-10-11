import QtQuick
import QtQuick.Controls.Basic as C

// Buttons: "primary" (accent), "secondary" (subtle fill), "ghost" (text only) and "danger".
C.AbstractButton {
    id: root
    property string kind: "secondary"
    property string iconName: ""
    property bool compact: false

    readonly property color fill: kind === "primary" ? Theme.accent
        : kind === "danger" ? Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.14)
        : kind === "ghost" ? "transparent" : Theme.surfaceAlt
    readonly property color ink: kind === "primary" ? Theme.accentText
        : kind === "danger" ? Theme.danger : kind === "ghost" ? Theme.textSecondary : Theme.text

    implicitHeight: compact ? 30 : 36
    implicitWidth: row.implicitWidth + (compact ? 20 : 28)
    focusPolicy: Qt.StrongFocus
    hoverEnabled: true
    opacity: enabled ? 1 : 0.45

    background: Rectangle {
        radius: height / 2
        color: root.fill
        border.width: root.kind === "secondary" ? 1 : 0
        border.color: Theme.stroke
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: root.kind === "primary" ? "#ffffff" : Theme.text
            opacity: root.pressed ? 0.12 : root.hovered ? 0.06 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.animFast } }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -3
            radius: height / 2
            color: "transparent"
            border.width: 2
            border.color: Theme.accentAlpha(0.6)
            visible: root.visualFocus
        }
    }

    contentItem: Item {
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 7
            Icon {
                visible: root.iconName !== ""
                name: root.iconName
                color: root.ink
                size: root.compact ? 14 : 16
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: root.text !== ""
                text: root.text
                color: root.ink
                font.pixelSize: root.compact ? Theme.fontSmall : Theme.fontBody
                font.weight: Font.DemiBold
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }
}

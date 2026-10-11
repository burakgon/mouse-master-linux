import QtQuick
import QtQuick.Controls.Basic as C

C.AbstractButton {
    id: root
    property string iconName: "x"
    property int size: 32
    property color ink: Theme.textSecondary
    property string tip: ""

    implicitWidth: size
    implicitHeight: size
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus

    background: Rectangle {
        radius: width / 2
        color: root.pressed ? Theme.surfacePressed : root.hovered ? Theme.surfaceHover : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
        border.width: root.visualFocus ? 2 : 0
        border.color: Theme.accentAlpha(0.6)
    }
    contentItem: Item {
        Icon {
            anchors.centerIn: parent
            name: root.iconName
            color: root.hovered ? Theme.text : root.ink
            size: Math.round(root.size * 0.52)
        }
    }
    C.ToolTip.visible: tip !== "" && hovered
    C.ToolTip.delay: 500
    C.ToolTip.text: tip
}

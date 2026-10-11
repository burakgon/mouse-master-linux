import QtQuick
import QtQuick.Controls.Basic as C

C.TextField {
    id: root
    property string iconName: ""
    implicitHeight: 38
    leftPadding: iconName !== "" ? 36 : 12
    rightPadding: 12
    color: Theme.text
    placeholderTextColor: Theme.textTertiary
    selectionColor: Theme.accentAlpha(0.35)
    selectedTextColor: Theme.text
    font.pixelSize: Theme.fontBody
    background: Rectangle {
        radius: 10
        color: Theme.surfaceAlt
        border.width: root.activeFocus ? 2 : 1
        border.color: root.activeFocus ? Theme.accentAlpha(0.7) : Theme.stroke
        Behavior on border.color { ColorAnimation { duration: Theme.animFast } }
        Icon {
            visible: root.iconName !== ""
            name: root.iconName
            size: 16
            color: Theme.textTertiary
            anchors.verticalCenter: parent.verticalCenter
            x: 12
        }
    }
}

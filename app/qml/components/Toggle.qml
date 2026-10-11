import QtQuick
import QtQuick.Controls.Basic as C

// An animated switch.
C.AbstractButton {
    id: root
    checkable: true
    implicitWidth: 44
    implicitHeight: 26
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45

    background: Rectangle {
        radius: height / 2
        color: root.checked ? Theme.accent : (Theme.dark ? "#3a3c45" : "#cfd3dc")
        Behavior on color { ColorAnimation { duration: Theme.anim } }
        border.width: root.visualFocus ? 2 : 0
        border.color: Theme.accentAlpha(0.5)
    }
    contentItem: Item {
        Rectangle {
            id: knob
            width: 20
            height: 20
            radius: 10
            y: 3
            x: root.checked ? root.width - width - 3 : 3
            color: "#ffffff"
            Behavior on x { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutCubic } }
            scale: root.pressed ? 0.9 : 1
            Behavior on scale { NumberAnimation { duration: Theme.animFast } }
        }
    }
}

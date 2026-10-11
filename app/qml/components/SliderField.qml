import QtQuick
import QtQuick.Controls.Basic as C

// A slider with a value bubble while dragging.
C.Slider {
    id: root
    property string suffix: ""
    property var format: function (v) { return Math.round(v) + root.suffix }
    property bool showBubble: true

    implicitHeight: 28
    implicitWidth: 240
    hoverEnabled: true
    opacity: enabled ? 1 : 0.45

    background: Item {
        x: root.leftPadding
        y: root.topPadding + root.availableHeight / 2 - 3
        width: root.availableWidth
        height: 6
        Rectangle {
            anchors.fill: parent
            radius: 3
            color: Theme.dark ? "#30323a" : "#dde0e7"
        }
        Rectangle {
            width: root.visualPosition * parent.width
            height: parent.height
            radius: 3
            color: Theme.accent
        }
    }

    handle: Rectangle {
        x: root.leftPadding + root.visualPosition * (root.availableWidth - width)
        y: root.topPadding + root.availableHeight / 2 - height / 2
        width: 20
        height: 20
        radius: 10
        color: "#ffffff"
        border.width: 1
        border.color: Theme.dark ? Qt.rgba(0, 0, 0, 0.3) : Qt.rgba(0, 0, 0, 0.12)
        scale: root.pressed ? 1.12 : root.hovered ? 1.05 : 1
        Behavior on scale { NumberAnimation { duration: Theme.animFast } }
        Rectangle {
            anchors.centerIn: parent
            width: 8
            height: 8
            radius: 4
            color: Theme.accent
        }

        Rectangle {
            id: bubble
            visible: root.showBubble && (root.pressed || opacity > 0)
            opacity: root.pressed ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.anim } }
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.top
            anchors.bottomMargin: 8
            width: bubbleText.implicitWidth + 16
            height: 26
            radius: 8
            color: Theme.dark ? "#f1f2f5" : "#1c1d22"
            Text {
                id: bubbleText
                anchors.centerIn: parent
                text: root.format(root.value)
                color: Theme.dark ? "#15161a" : "#ffffff"
                font.pixelSize: Theme.fontSmall
                font.weight: Font.DemiBold
            }
        }
    }
}

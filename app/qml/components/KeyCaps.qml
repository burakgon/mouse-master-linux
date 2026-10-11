import QtQuick
import "Actions.js" as Actions

// Shows a key combination as keycaps.
Row {
    id: root
    property var keys: []
    spacing: 4
    Repeater {
        model: root.keys
        delegate: Rectangle {
            required property string modelData
            height: 26
            width: Math.max(26, capText.implicitWidth + 14)
            radius: 6
            color: Theme.surfaceAlt
            border.width: 1
            border.color: Theme.strokeStrong
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 2
                radius: 6
                color: Theme.strokeStrong
            }
            Text {
                id: capText
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -1
                text: Actions.keyLabel(modelData)
                color: Theme.text
                font.pixelSize: Theme.fontCaption
                font.weight: Font.DemiBold
            }
        }
    }
}

pragma ComponentBehavior: Bound
import QtQuick

// A segmented control. `options` is a list of { value, label, icon? }.
Rectangle {
    id: root
    property var options: []
    property var value
    property int minimumWidth: 160
    signal activated(var value)

    readonly property int count: options.length
    readonly property int currentIndex: {
        for (var i = 0; i < options.length; ++i)
            if (options[i].value === value)
                return i
        return -1
    }
    // Widest segment, measured from the labels so sizing never depends on layout.
    readonly property real segmentWidth: {
        var w = 0
        for (var i = 0; i < options.length; ++i) {
            var o = options[i]
            w = Math.max(w, metrics.advanceWidth(o.label) + (o.icon ? 21 : 0) + 30)
        }
        return w
    }

    FontMetrics {
        id: metrics
        font.pixelSize: Theme.fontSmall
        font.weight: Font.DemiBold
    }

    implicitHeight: 36
    implicitWidth: Math.max(minimumWidth, count * segmentWidth + 6)
    radius: height / 2
    color: Theme.surfaceAlt
    border.width: 1
    border.color: Theme.stroke

    Rectangle {
        id: highlight
        visible: root.currentIndex >= 0
        y: 3
        height: parent.height - 6
        width: root.count > 0 ? (parent.width - 6) / root.count : 0
        x: 3 + Math.max(0, root.currentIndex) * width
        radius: height / 2
        color: Theme.dark ? "#363842" : "#ffffff"
        border.width: Theme.dark ? 0 : 1
        border.color: Theme.stroke
        Behavior on x { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutCubic } }
    }

    Repeater {
        model: root.options
        delegate: Item {
            id: segment
            required property var modelData
            required property int index
            x: 3 + index * width
            y: 3
            width: root.count > 0 ? (root.width - 6) / root.count : 0
            height: root.height - 6
            Row {
                anchors.centerIn: parent
                spacing: 6
                Icon {
                    visible: !!segment.modelData.icon
                    name: segment.modelData.icon || ""
                    size: 15
                    color: segment.index === root.currentIndex ? Theme.text : Theme.textSecondary
                    anchors.verticalCenter: parent.verticalCenter
                }
                Text {
                    text: segment.modelData.label
                    font.pixelSize: Theme.fontSmall
                    font.weight: segment.index === root.currentIndex ? Font.DemiBold : Font.Medium
                    color: segment.index === root.currentIndex ? Theme.text : Theme.textSecondary
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    root.value = segment.modelData.value
                    root.activated(segment.modelData.value)
                }
            }
        }
    }
}

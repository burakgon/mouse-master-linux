import QtQuick

// Battery glyph with fill level, charging bolt and an optional percentage.
Row {
    id: root
    property var battery: null
    property bool showText: true
    readonly property int percent: {
        if (!battery) return -1
        if (battery.percent !== null && battery.percent !== undefined) return battery.percent
        switch (battery.level) {
        case "critical": return 5
        case "low": return 20
        case "good": return 60
        case "full": return 95
        }
        return -1
    }
    readonly property bool charging: !!battery && (battery.state === "charging" || battery.state === "charging-slow")
    readonly property color levelColor: percent >= 0 && percent <= 15 && !charging ? Theme.danger
        : percent >= 0 && percent <= 30 && !charging ? Theme.warning : charging ? Theme.success : Theme.text

    spacing: 6
    visible: percent >= 0

    Item {
        width: 24
        height: 12
        anchors.verticalCenter: parent.verticalCenter
        Rectangle {
            id: shell
            width: 21
            height: 12
            radius: 3
            color: "transparent"
            border.width: 1.5
            border.color: Theme.textSecondary
            Rectangle {
                x: 2.5
                y: 2.5
                height: parent.height - 5
                width: Math.max(1.5, (parent.width - 5) * Math.max(0, root.percent) / 100)
                radius: 1.5
                color: root.levelColor
            }
        }
        Rectangle {
            x: 22
            y: 4
            width: 2
            height: 4
            radius: 1
            color: Theme.textSecondary
        }
        Icon {
            visible: root.charging
            name: "zap"
            size: 11
            color: Theme.dark ? "#ffffff" : "#000000"
            anchors.centerIn: shell
        }
    }
    Text {
        visible: root.showText
        text: root.percent >= 0 ? root.percent + "%" : ""
        color: Theme.textSecondary
        font.pixelSize: Theme.fontSmall
        anchors.verticalCenter: parent.verticalCenter
    }
}

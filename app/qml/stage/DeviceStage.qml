import QtQuick
import QtQuick.Effects
import QtQuick.Shapes
import "Actions.js" as Actions
import "Layouts.js" as Layouts

// The device artwork with a callout for every configurable control.
Item {
    id: stage
    property var device
    property var bindings: ({})          // control key -> { action, inherited }
    property string selected: ""
    property var pressed: ({})           // control key -> true while held
    property string hovered: ""
    signal controlClicked(string key)

    readonly property var layout: Layouts.forDevice(device)
    readonly property bool compact: width < 760
    readonly property int calloutWidth: compact ? 44 : 214
    readonly property int calloutHeight: compact ? 44 : 58
    readonly property int gap: compact ? 28 : 56

    // Controls shown on the artwork, in a stable order.
    readonly property var items: {
        var out = []
        if (!device)
            return out
        var spots = layout.spots
        var controls = device.controls || []
        for (var i = 0; i < controls.length; ++i) {
            var c = controls[i]
            if (c.is_virtual || !spots[c.key])
                continue
            out.push({ key: c.key, name: c.name, spot: spots[c.key] })
        }
        if (device.capabilities && device.capabilities.thumbwheel && spots["thumbwheel"])
            out.push({ key: "thumbwheel", name: "Thumb wheel", spot: spots["thumbwheel"] })
        return out
    }

    readonly property real artHeight: {
        var avail = width - (compact ? 2 * (calloutWidth + gap) : 2 * (calloutWidth + gap))
        var byWidth = avail * layout.height / layout.width
        return Math.max(160, Math.min(height - 48, byWidth, 620))
    }
    readonly property real artScale: artHeight / layout.height
    readonly property real artWidth: layout.width * artScale
    readonly property real artX: (width - artWidth) / 2
    readonly property real artY: (height - artHeight) / 2

    function spotPoint(spot) {
        return Qt.point(artX + spot.x * artScale, artY + spot.y * artScale)
    }

    // Spreads callouts of one side vertically so they never overlap.
    function placements(side) {
        var list = []
        for (var i = 0; i < items.length; ++i)
            if (items[i].spot.side === side)
                list.push({ key: items[i].key, y: spotPoint(items[i].spot).y - calloutHeight / 2 })
        list.sort(function (a, b) { return a.y - b.y })
        var spacing = calloutHeight + 12
        for (var j = 1; j < list.length; ++j)
            list[j].y = Math.max(list[j].y, list[j - 1].y + spacing)
        var bottom = height - calloutHeight - 8
        if (list.length && list[list.length - 1].y > bottom) {
            list[list.length - 1].y = bottom
            for (var k = list.length - 2; k >= 0; --k)
                list[k].y = Math.min(list[k].y, list[k + 1].y - spacing)
        }
        for (var m = 0; m < list.length; ++m)
            list[m].y = Math.max(8, list[m].y)
        var map = {}
        for (var n = 0; n < list.length; ++n)
            map[list[n].key] = list[n].y
        return map
    }
    readonly property var leftY: placements("left")
    readonly property var rightY: placements("right")

    // Spotlight behind the device: a circle fading to transparent at its edge.
    Shape {
        id: spotlight
        readonly property real r: stage.artHeight * 0.78
        readonly property real cx: stage.artX + stage.artWidth / 2
        readonly property real cy: stage.artY + stage.artHeight * 0.52
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: 0
            strokeColor: "transparent"
            fillGradient: RadialGradient {
                centerX: spotlight.cx
                centerY: spotlight.cy
                centerRadius: spotlight.r
                focalX: spotlight.cx
                focalY: spotlight.cy
                GradientStop { position: 0.0; color: Theme.accentAlpha(Theme.dark ? 0.13 : 0.10) }
                GradientStop { position: 0.6; color: Theme.accentAlpha(Theme.dark ? 0.04 : 0.03) }
                GradientStop { position: 1.0; color: Theme.accentAlpha(0.0) }
            }
            startX: spotlight.cx + spotlight.r
            startY: spotlight.cy
            PathAngleArc {
                centerX: spotlight.cx
                centerY: spotlight.cy
                radiusX: spotlight.r
                radiusY: spotlight.r
                startAngle: 0
                sweepAngle: 360
            }
        }
    }

    // Ground shadow
    Rectangle {
        x: stage.artX + stage.artWidth * 0.12
        y: stage.artY + stage.artHeight * 0.9
        width: stage.artWidth * 0.82
        height: stage.artHeight * 0.08
        radius: height / 2
        color: Theme.shadow
        layer.enabled: true
        layer.effect: MultiEffect { blurEnabled: true; blur: 1.0; blurMax: 48 }
    }

    Image {
        id: art
        x: stage.artX
        y: stage.artY
        width: stage.artWidth
        height: stage.artHeight
        source: stage.layout.art
        sourceSize: Qt.size(Math.round(stage.artWidth * 2), Math.round(stage.artHeight * 2))
        smooth: true
        mipmap: true
        visible: false
    }
    MultiEffect {
        source: art
        anchors.fill: art
        shadowEnabled: true
        shadowColor: Theme.shadow
        shadowBlur: 1.0
        shadowVerticalOffset: 14
        shadowHorizontalOffset: 4
        shadowScale: 1.0
    }

    // Connectors
    Repeater {
        model: stage.items
        delegate: Shape {
            id: connector
            required property var modelData
            readonly property point p: stage.spotPoint(modelData.spot)
            readonly property bool onLeft: modelData.spot.side === "left"
            readonly property real cy: (onLeft ? stage.leftY[modelData.key] : stage.rightY[modelData.key]) + stage.calloutHeight / 2
            readonly property real cx: onLeft ? stage.artX - stage.gap : stage.artX + stage.artWidth + stage.gap
            readonly property bool emphasized: stage.selected === modelData.key || stage.hovered === modelData.key || !!stage.pressed[modelData.key]
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            opacity: emphasized ? 1 : 0.55
            Behavior on opacity { NumberAnimation { duration: Theme.anim } }
            ShapePath {
                strokeWidth: connector.emphasized ? 2 : 1.4
                strokeColor: connector.emphasized ? Theme.accent : Theme.textTertiary
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                startX: connector.p.x
                startY: connector.p.y
                PathCubic {
                    x: connector.cx
                    y: connector.cy
                    control1X: (connector.p.x + connector.cx) / 2
                    control1Y: connector.p.y
                    control2X: (connector.p.x + connector.cx) / 2
                    control2Y: connector.cy
                }
            }
        }
    }

    // Hotspots
    Repeater {
        model: stage.items
        delegate: Hotspot {
            required property var modelData
            readonly property point p: stage.spotPoint(modelData.spot)
            x: p.x - width / 2
            y: p.y - height / 2
            selected: stage.selected === modelData.key
            highlighted: stage.hovered === modelData.key
            active: !!stage.pressed[modelData.key]
            onClicked: stage.controlClicked(modelData.key)
        }
    }

    // Callouts
    Repeater {
        model: stage.items
        delegate: Callout {
            required property var modelData
            readonly property bool onLeft: modelData.spot.side === "left"
            readonly property var binding: stage.bindings[modelData.key] || { action: { type: "default" }, inherited: false }
            readonly property var info: Actions.describe(binding.action, Daemon.catalog, modelData.key, Apps.lookup)
            compact: stage.compact
            width: stage.calloutWidth
            height: stage.calloutHeight
            x: onLeft ? stage.artX - stage.gap - width : stage.artX + stage.artWidth + stage.gap
            y: onLeft ? stage.leftY[modelData.key] : stage.rightY[modelData.key]
            title: modelData.name
            label: info.label
            iconName: info.icon
            inherited: binding.inherited
            selected: stage.selected === modelData.key
            active: !!stage.pressed[modelData.key]
            onClicked: stage.controlClicked(modelData.key)
            HoverHandler {
                onHoveredChanged: stage.hovered = hovered ? modelData.key : (stage.hovered === modelData.key ? "" : stage.hovered)
            }
            Behavior on x { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutCubic } }
            Behavior on y { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutCubic } }
        }
    }
}

pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts

ScrollPage {
    id: page
    property var device
    readonly property var caps: device ? device.capabilities : ({})
    readonly property var st: device ? device.state : ({})
    readonly property var settings: device ? device.config.settings : ({})
    readonly property var dpiPresets: {
        var p = settings.dpi_presets || []
        if (p.length >= 2)
            return p
        if (!caps.dpi)
            return []
        return [400, 800, 1200, 1600, 2400, 3200, 4000].filter(function (v) { return v >= caps.dpi.min && v <= caps.dpi.max })
    }
    property bool editingPresets: false
    readonly property bool smartshiftOn: (st.smartshift_threshold || 0) !== 255 && (st.smartshift_threshold || 0) !== 0

    function set(patch, done) {
        Daemon.command(device.id, { command: "set-settings", settings: patch }, function (err) {
            if (err)
                Notify.show(err, true)
            else if (done)
                done()
        })
    }

    PageHeader {
        title: "Point & scroll"
        subtitle: "Pointer speed, the scroll wheel and the thumb wheel."
    }

    // Pointer speed
    Card {
        visible: !!page.caps.dpi
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 16
            RowLayout {
                Layout.fillWidth: true
                SectionTitle {
                    text: "Pointer speed"
                    subtitle: "Sensor resolution. Higher values move the pointer further for the same hand movement."
                }
                Column {
                    Layout.alignment: Qt.AlignRight | Qt.AlignTop
                    Text {
                        anchors.right: parent.right
                        text: Math.round(dpiSlider.pressed ? dpiSlider.value : (page.st.dpi || 0))
                        color: Theme.text
                        font.pixelSize: 34
                        font.weight: Font.Bold
                    }
                    Text {
                        anchors.right: parent.right
                        text: "DPI"
                        color: Theme.textTertiary
                        font.pixelSize: Theme.fontCaption
                        font.weight: Font.DemiBold
                        font.letterSpacing: 1.2
                    }
                }
            }
            SliderField {
                id: dpiSlider
                Layout.fillWidth: true
                from: page.caps.dpi ? page.caps.dpi.min : 200
                to: page.caps.dpi ? page.caps.dpi.max : 4000
                stepSize: page.caps.dpi && page.caps.dpi.range.kind === "range" ? page.caps.dpi.range.step : 50
                snapMode: C.Slider.SnapAlways
                value: page.st.dpi || 1000
                suffix: " DPI"
                onPressedChanged: if (!pressed) page.set({ dpi: Math.round(value) })
            }
            RowLayout {
                Layout.fillWidth: true
                Label { text: "Presets"; secondary: true; size: Theme.fontSmall; Layout.alignment: Qt.AlignVCenter }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: page.dpiPresets
                        delegate: Chip {
                            required property int modelData
                            text: modelData
                            selected: page.st.dpi === modelData
                            closable: page.editingPresets
                            onClicked: page.set({ dpi: modelData })
                            onCloseClicked: page.set({ dpi_presets: page.dpiPresets.filter(function (v) { return v !== modelData }) })
                        }
                    }
                    Chip {
                        visible: page.editingPresets && page.dpiPresets.indexOf(page.st.dpi) < 0 && page.dpiPresets.length < 8
                        text: "Add " + page.st.dpi
                        iconName: "plus"
                        onClicked: page.set({ dpi_presets: page.dpiPresets.concat([page.st.dpi]) })
                    }
                }
                AppButton {
                    text: page.editingPresets ? "Done" : "Edit"
                    kind: "ghost"
                    compact: true
                    onClicked: page.editingPresets = !page.editingPresets
                }
            }
            Label {
                Layout.fillWidth: true
                text: "Assign “Cycle pointer speed” to a button to switch between presets, or “Pointer speed → While held” for a precision mode."
                tertiary: true
                size: Theme.fontCaption
                wrapMode: Text.WordWrap
            }
        }
    }

    // Scroll wheel
    Card {
        visible: !!page.caps.smartshift || !!page.caps.wheel
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle {
                text: "Scroll wheel"
                subtitle: "SmartShift switches the wheel from ratchet to free spin when you scroll fast."
            }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                visible: !!page.caps.smartshift
                iconName: "disc-3"
                title: "SmartShift"
                description: "Automatically free spin when you flick the wheel."
                Toggle {
                    checked: page.smartshiftOn
                    onToggled: page.set({ smartshift: checked })
                }
            }
            SettingRow {
                visible: !!page.caps.smartshift && page.smartshiftOn
                iconName: "gauge"
                title: "Sensitivity"
                description: "Higher switches to free spin with a gentler flick."
                SliderField {
                    width: 200
                    from: 1
                    to: 50
                    stepSize: 1
                    value: 51 - Math.min(50, page.st.smartshift_threshold || 10)
                    format: function (v) { return Math.round(v * 2) + "%" }
                    onPressedChanged: if (!pressed) page.set({ smartshift: true, smartshift_threshold: 51 - Math.round(value) })
                }
            }
            SettingRow {
                visible: !!page.caps.smartshift
                iconName: "mouse"
                title: "Wheel mode"
                description: "The mode the wheel starts in. The button behind the wheel toggles it."
                Segmented {
                    minimumWidth: 220
                    options: [{ value: "ratchet", label: "Ratchet" }, { value: "free-spin", label: "Free spin" }]
                    value: page.st.wheel_mode || "ratchet"
                    onActivated: function (v) { page.set({ wheel_mode: v }) }
                }
            }
            SettingRow {
                visible: !!page.caps.smartshift && page.caps.smartshift.torque
                iconName: "sliders-horizontal"
                title: "Ratchet force"
                SliderField {
                    width: 200
                    from: 1
                    to: 100
                    value: page.st.smartshift_torque || 50
                    suffix: "%"
                    onPressedChanged: if (!pressed) page.set({ smartshift_torque: Math.round(value) })
                }
            }
            SettingRow {
                visible: !!page.caps.wheel
                iconName: "scroll-text"
                title: "Smooth scrolling"
                description: "High resolution wheel reports for fluid scrolling."
                Toggle {
                    checked: !!page.st.hires_scroll
                    onToggled: page.set({ hires_scroll: checked })
                }
            }
            SettingRow {
                visible: !!page.caps.wheel && page.caps.wheel.invert
                iconName: "arrow-down-up"
                title: "Natural scrolling"
                description: "Content moves in the direction you turn the wheel."
                divider: false
                Toggle {
                    checked: !!page.st.invert_scroll
                    onToggled: page.set({ invert_scroll: checked })
                }
            }
        }
    }

    // Thumb wheel
    Card {
        visible: !!page.caps.thumbwheel
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle {
                text: "Thumb wheel"
                subtitle: "Choose what it does on the Buttons page — scrolling, zoom, volume, tabs and more."
            }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                iconName: "arrow-down-up"
                title: "Invert direction"
                divider: false
                Toggle {
                    checked: !!page.st.thumbwheel_invert
                    onToggled: page.set({ thumbwheel_invert: checked })
                }
            }
        }
    }

    // Report rate & onboard memory (gaming mice)
    Card {
        visible: (page.caps.report_rates_us || []).length > 1 || !!page.caps.onboard_profiles
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle { text: "Performance" }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                visible: (page.caps.report_rates_us || []).length > 1
                iconName: "zap"
                title: "Report rate"
                description: "How often the mouse reports its position."
                Segmented {
                    minimumWidth: 240
                    options: (page.caps.report_rates_us || []).slice().reverse().map(function (us) {
                        var hz = Math.round(1000000 / us)
                        return { value: us, label: hz >= 1000 ? (hz / 1000) + " kHz" : hz + " Hz" }
                    })
                    value: page.st.report_rate_us
                    onActivated: function (v) { page.set({ report_rate_us: v }) }
                }
            }
            SettingRow {
                visible: !!page.caps.onboard_profiles
                iconName: "cpu"
                title: "Use settings from Mouse Master"
                description: "Turn off to use the profiles stored in the mouse's onboard memory."
                divider: false
                Toggle {
                    checked: page.st.host_mode === true
                    onToggled: page.set({ host_mode: checked })
                }
            }
        }
    }
}

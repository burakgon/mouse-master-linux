pragma Singleton
import QtQuick

// Colours, type and metrics. Follows the system light/dark preference and
// accent colour, with curated neutrals so the app looks the same everywhere.
QtObject {
    id: theme

    // 0 = follow system, 1 = light, 2 = dark. Persisted by Main.qml.
    property int mode: 0
    property bool useSystemAccent: true

    readonly property bool systemDark: Qt.styleHints.colorScheme === Qt.ColorScheme.Dark
        || (Qt.styleHints.colorScheme === Qt.ColorScheme.Unknown && _palette.window.hslLightness < 0.5)
    readonly property bool dark: mode === 2 || (mode === 0 && systemDark)

    readonly property SystemPalette _palette: SystemPalette { colorGroup: SystemPalette.Active }

    // A system accent that is too grey or too dark/light is replaced by ours.
    readonly property color brand: "#5b7cff"
    readonly property color _sys: _palette.highlight
    readonly property bool _sysUsable: _sys.hslSaturation > 0.25 && _sys.hslLightness > 0.25 && _sys.hslLightness < 0.8
    readonly property color accent: useSystemAccent && _sysUsable ? _sys : brand
    readonly property color accentText: accent.hslLightness > 0.62 ? "#10121a" : "#ffffff"
    function accentAlpha(a) { return Qt.rgba(accent.r, accent.g, accent.b, a) }

    // Neutrals
    readonly property color window: dark ? "#111215" : "#f3f4f7"
    readonly property color sidebar: dark ? "#16171b" : "#eaecf1"
    readonly property color surface: dark ? "#1b1c21" : "#ffffff"
    readonly property color surfaceAlt: dark ? "#212228" : "#f5f6f9"
    readonly property color surfaceHover: dark ? "#25272e" : "#eef0f4"
    readonly property color surfacePressed: dark ? "#2c2e36" : "#e5e8ee"
    readonly property color raised: dark ? "#23252c" : "#ffffff"
    readonly property color stroke: dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(0, 0, 0, 0.08)
    readonly property color strokeStrong: dark ? Qt.rgba(1, 1, 1, 0.13) : Qt.rgba(0, 0, 0, 0.14)
    readonly property color text: dark ? "#f1f2f5" : "#15161a"
    readonly property color textSecondary: dark ? "#a4a7b3" : "#5b5f6e"
    readonly property color textTertiary: dark ? "#6f7380" : "#8c909d"
    readonly property color overlay: dark ? Qt.rgba(0, 0, 0, 0.55) : Qt.rgba(0.07, 0.08, 0.12, 0.35)
    readonly property color shadow: dark ? Qt.rgba(0, 0, 0, 0.55) : Qt.rgba(0.1, 0.12, 0.2, 0.16)

    // Status
    readonly property color success: dark ? "#3fd38f" : "#1a9e63"
    readonly property color warning: dark ? "#f6b847" : "#c47f00"
    readonly property color danger: dark ? "#ff6474" : "#d6334a"

    // The stage behind device artwork
    readonly property color stageTop: dark ? "#1d1f26" : "#ffffff"
    readonly property color stageBottom: dark ? "#121317" : "#e9ecf2"

    // Type scale
    readonly property int fontDisplay: 30
    readonly property int fontTitle: 22
    readonly property int fontHeadline: 16
    readonly property int fontBody: 14
    readonly property int fontSmall: 13
    readonly property int fontCaption: 12

    // Metrics
    readonly property int radiusSmall: 8
    readonly property int radius: 12
    readonly property int radiusLarge: 18
    readonly property int spacing: 12
    readonly property int animFast: 120
    readonly property int anim: 200
    readonly property int animSlow: 320

    function hex(c) {
        // "rrggbb" for icon tinting
        var s = c.toString()
        return s.length === 9 ? s.substr(3) : s.substr(1)
    }
}

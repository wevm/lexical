import Lexical

// Port of packages/lexical-table/src/constants.ts.

/// `PIXEL_VALUE_REG_EXP`: `/^(\d+(?:\.\d+)?)px$/`, matched whole (JS `\d`
/// is ASCII only, and `$` without the `m` flag is the end of the input).
nonisolated(unsafe) package let PIXEL_VALUE_REG_EXP = /([0-9]+(?:\.[0-9]+)?)px/

/// `PIXEL_VALUE_REG_EXP.test(value)`.
package func isPixelValue(_ value: String) -> Bool {
  (try? PIXEL_VALUE_REG_EXP.wholeMatch(in: value)) != nil
}

/// `.PlaygroundEditorTheme__tableCell` width value from
/// packages/lexical-playground/src/themes/PlaygroundEditorTheme.css.
package let COLUMN_WIDTH: Double = 75

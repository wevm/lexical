/// JS `\s`, and the characters `String.prototype.trim` removes.
package func isJSWhitespace(_ unit: UInt16) -> Bool {
  switch unit {
  case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F,
    0x3000, 0xFEFF:
    true
  default: false
  }
}

package func jsTrim(_ units: ArraySlice<UInt16>) -> ArraySlice<UInt16> {
  var slice = units
  while let first = slice.first, isJSWhitespace(first) { slice = slice.dropFirst() }
  while let last = slice.last, isJSWhitespace(last) { slice = slice.dropLast() }
  return slice
}

/// `string.trim()`.
package func jsTrim(_ string: String) -> String {
  let units = Array(string.utf16)
  return String(decoding: jsTrim(units[...]), as: UTF16.self)
}

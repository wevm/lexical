import Lexical
import LexicalText

// Port of packages/lexical-hashtag/src/LexicalHashtagExtension.ts.
//
// JS matches with `new RegExp(getHashtagRegexString(), 'i')`, a regular
// expression without the `u` flag, so it reads the text as UTF-16 code
// units and folds case with JS's `Canonicalize`. Swift's regular
// expressions read Unicode scalars and fold case with Unicode case folding,
// so `HashtagMatcher` runs the same expression by hand, over code units,
// with JS's case folding; the character tables are JS's, as strings of
// class ranges.

/// `String.fromCharCode(code)`: the code unit `code & 0xFFFF`. JS builds
/// the CJK extension ranges with it from code points past U+FFFF, which it
/// truncates, so the ranges hold the BMP characters they truncate to.
private func charCode(_ code: Int) -> String {
  String(Unicode.Scalar(UInt16(truncatingIfNeeded: code))!)
}

private func getHashtagRegexStringChars() -> (
  alpha: String, alphanumeric: String, hashChars: String
) {
  // Latin accented characters
  // Excludes 0xd7 from the range
  // (the multiplication sign, confusable with "x").
  // Also excludes 0xf7, the division sign
  let latinAccents = [
    "\u{c0}-\u{d6}",
    "\u{d8}-\u{f6}",
    "\u{f8}-\u{ff}",
    "\u{0100}-\u{024f}",
    "\u{0253}-\u{0254}",
    "\u{0256}-\u{0257}",
    "\u{0259}",
    "\u{025b}",
    "\u{0263}",
    "\u{0268}",
    "\u{026f}",
    "\u{0272}",
    "\u{0289}",
    "\u{028b}",
    "\u{02bb}",
    "\u{0300}-\u{036f}",
    "\u{1e00}-\u{1eff}",
  ].joined()

  // Cyrillic (Russian, Ukrainian, etc.)
  let nonLatinChars = [
    "\u{0400}-\u{04ff}",  // Cyrillic
    "\u{0500}-\u{0527}",  // Cyrillic Supplement
    "\u{2de0}-\u{2dff}",  // Cyrillic Extended A
    "\u{a640}-\u{a69f}",  // Cyrillic Extended B
    "\u{0591}-\u{05bf}",  // Hebrew
    "\u{05c1}-\u{05c2}",
    "\u{05c4}-\u{05c5}",
    "\u{05c7}",
    "\u{05d0}-\u{05ea}",
    "\u{05f0}-\u{05f4}",
    "\u{fb12}-\u{fb28}",  // Hebrew Presentation Forms
    "\u{fb2a}-\u{fb36}",
    "\u{fb38}-\u{fb3c}",
    "\u{fb3e}",
    "\u{fb40}-\u{fb41}",
    "\u{fb43}-\u{fb44}",
    "\u{fb46}-\u{fb4f}",
    "\u{0610}-\u{061a}",  // Arabic
    "\u{0620}-\u{065f}",
    "\u{066e}-\u{06d3}",
    "\u{06d5}-\u{06dc}",
    "\u{06de}-\u{06e8}",
    "\u{06ea}-\u{06ef}",
    "\u{06fa}-\u{06fc}",
    "\u{06ff}",
    "\u{0750}-\u{077f}",  // Arabic Supplement
    "\u{08a0}",  // Arabic Extended A
    "\u{08a2}-\u{08ac}",
    "\u{08e4}-\u{08fe}",
    "\u{fb50}-\u{fbb1}",  // Arabic Pres. Forms A
    "\u{fbd3}-\u{fd3d}",
    "\u{fd50}-\u{fd8f}",
    "\u{fd92}-\u{fdc7}",
    "\u{fdf0}-\u{fdfb}",
    "\u{fe70}-\u{fe74}",  // Arabic Pres. Forms B
    "\u{fe76}-\u{fefc}",
    "\u{200c}-\u{200c}",  // Zero-Width Non-Joiner
    "\u{0e01}-\u{0e3a}",  // Thai
    "\u{0e40}-\u{0e4e}",  // Hangul (Korean)
    "\u{1100}-\u{11ff}",  // Hangul Jamo
    "\u{3130}-\u{3185}",  // Hangul Compatibility Jamo
    "\u{A960}-\u{A97F}",  // Hangul Jamo Extended-A
    "\u{AC00}-\u{D7AF}",  // Hangul Syllables
    "\u{D7B0}-\u{D7FF}",  // Hangul Jamo Extended-B
    "\u{FFA1}-\u{FFDC}",  // Half-width Hangul
  ].joined()

  let cjkChars = [
    "\u{30A1}-\u{30FA}\u{30FC}-\u{30FE}",  // Katakana (full-width)
    "\u{FF66}-\u{FF9F}",  // Katakana (half-width)
    "\u{FF10}-\u{FF19}\u{FF21}-\u{FF3A}",
    "\u{FF41}-\u{FF5A}",  // Latin (full-width)
    "\u{3041}-\u{3096}\u{3099}-\u{309E}",  // Hiragana
    "\u{3400}-\u{4DBF}",  // Kanji (CJK Extension A)
    "\u{4E00}-\u{9FFF}",  // Kanji (Unified)
    // Disabled as it breaks the Regex.
    // charCode(0x20000) + "-" + charCode(0x2A6DF), // Kanji (CJK Extension B)
    charCode(0x2a700),
    "-",
    charCode(0x2b73f),  // Kanji (CJK Extension C)
    charCode(0x2b740),
    "-",
    charCode(0x2b81f),  // Kanji (CJK Extension D)
    charCode(0x2f800),
    "-",
    charCode(0x2fa1f),
    "\u{3003}\u{3005}\u{303B}",  // Kanji (CJK supplement)
  ].joined()

  let otherChars = latinAccents + nonLatinChars + cjkChars
  // equivalent of \p{L}

  let unicodeLetters = [
    "\u{0041}-\u{005A}\u{0061}-\u{007A}\u{00AA}\u{00B5}\u{00BA}\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}",
    "\u{00F8}-\u{0241}\u{0250}-\u{02C1}\u{02C6}-\u{02D1}\u{02E0}-\u{02E4}\u{02EE}\u{037A}\u{0386}",
    "\u{0388}-\u{038A}\u{038C}\u{038E}-\u{03A1}\u{03A3}-\u{03CE}\u{03D0}-\u{03F5}\u{03F7}-\u{0481}",
    "\u{048A}-\u{04CE}\u{04D0}-\u{04F9}\u{0500}-\u{050F}\u{0531}-\u{0556}\u{0559}\u{0561}-\u{0587}",
    "\u{05D0}-\u{05EA}\u{05F0}-\u{05F2}\u{0621}-\u{063A}\u{0640}-\u{064A}\u{066E}-\u{066F}",
    "\u{0671}-\u{06D3}\u{06D5}\u{06E5}-\u{06E6}\u{06EE}-\u{06EF}\u{06FA}-\u{06FC}\u{06FF}\u{0710}",
    "\u{0712}-\u{072F}\u{074D}-\u{076D}\u{0780}-\u{07A5}\u{07B1}\u{0904}-\u{0939}\u{093D}\u{0950}",
    "\u{0958}-\u{0961}\u{097D}\u{0985}-\u{098C}\u{098F}-\u{0990}\u{0993}-\u{09A8}\u{09AA}-\u{09B0}",
    "\u{09B2}\u{09B6}-\u{09B9}\u{09BD}\u{09CE}\u{09DC}-\u{09DD}\u{09DF}-\u{09E1}\u{09F0}-\u{09F1}",
    "\u{0A05}-\u{0A0A}\u{0A0F}-\u{0A10}\u{0A13}-\u{0A28}\u{0A2A}-\u{0A30}\u{0A32}-\u{0A33}",
    "\u{0A35}-\u{0A36}\u{0A38}-\u{0A39}\u{0A59}-\u{0A5C}\u{0A5E}\u{0A72}-\u{0A74}\u{0A85}-\u{0A8D}",
    "\u{0A8F}-\u{0A91}\u{0A93}-\u{0AA8}\u{0AAA}-\u{0AB0}\u{0AB2}-\u{0AB3}\u{0AB5}-\u{0AB9}\u{0ABD}",
    "\u{0AD0}\u{0AE0}-\u{0AE1}\u{0B05}-\u{0B0C}\u{0B0F}-\u{0B10}\u{0B13}-\u{0B28}\u{0B2A}-\u{0B30}",
    "\u{0B32}-\u{0B33}\u{0B35}-\u{0B39}\u{0B3D}\u{0B5C}-\u{0B5D}\u{0B5F}-\u{0B61}\u{0B71}\u{0B83}",
    "\u{0B85}-\u{0B8A}\u{0B8E}-\u{0B90}\u{0B92}-\u{0B95}\u{0B99}-\u{0B9A}\u{0B9C}\u{0B9E}-\u{0B9F}",
    "\u{0BA3}-\u{0BA4}\u{0BA8}-\u{0BAA}\u{0BAE}-\u{0BB9}\u{0C05}-\u{0C0C}\u{0C0E}-\u{0C10}",
    "\u{0C12}-\u{0C28}\u{0C2A}-\u{0C33}\u{0C35}-\u{0C39}\u{0C60}-\u{0C61}\u{0C85}-\u{0C8C}",
    "\u{0C8E}-\u{0C90}\u{0C92}-\u{0CA8}\u{0CAA}-\u{0CB3}\u{0CB5}-\u{0CB9}\u{0CBD}\u{0CDE}",
    "\u{0CE0}-\u{0CE1}\u{0D05}-\u{0D0C}\u{0D0E}-\u{0D10}\u{0D12}-\u{0D28}\u{0D2A}-\u{0D39}",
    "\u{0D60}-\u{0D61}\u{0D85}-\u{0D96}\u{0D9A}-\u{0DB1}\u{0DB3}-\u{0DBB}\u{0DBD}\u{0DC0}-\u{0DC6}",
    "\u{0E01}-\u{0E30}\u{0E32}-\u{0E33}\u{0E40}-\u{0E46}\u{0E81}-\u{0E82}\u{0E84}\u{0E87}-\u{0E88}",
    "\u{0E8A}\u{0E8D}\u{0E94}-\u{0E97}\u{0E99}-\u{0E9F}\u{0EA1}-\u{0EA3}\u{0EA5}\u{0EA7}",
    "\u{0EAA}-\u{0EAB}\u{0EAD}-\u{0EB0}\u{0EB2}-\u{0EB3}\u{0EBD}\u{0EC0}-\u{0EC4}\u{0EC6}",
    "\u{0EDC}-\u{0EDD}\u{0F00}\u{0F40}-\u{0F47}\u{0F49}-\u{0F6A}\u{0F88}-\u{0F8B}\u{1000}-\u{1021}",
    "\u{1023}-\u{1027}\u{1029}-\u{102A}\u{1050}-\u{1055}\u{10A0}-\u{10C5}\u{10D0}-\u{10FA}\u{10FC}",
    "\u{1100}-\u{1159}\u{115F}-\u{11A2}\u{11A8}-\u{11F9}\u{1200}-\u{1248}\u{124A}-\u{124D}",
    "\u{1250}-\u{1256}\u{1258}\u{125A}-\u{125D}\u{1260}-\u{1288}\u{128A}-\u{128D}\u{1290}-\u{12B0}",
    "\u{12B2}-\u{12B5}\u{12B8}-\u{12BE}\u{12C0}\u{12C2}-\u{12C5}\u{12C8}-\u{12D6}\u{12D8}-\u{1310}",
    "\u{1312}-\u{1315}\u{1318}-\u{135A}\u{1380}-\u{138F}\u{13A0}-\u{13F4}\u{1401}-\u{166C}",
    "\u{166F}-\u{1676}\u{1681}-\u{169A}\u{16A0}-\u{16EA}\u{1700}-\u{170C}\u{170E}-\u{1711}",
    "\u{1720}-\u{1731}\u{1740}-\u{1751}\u{1760}-\u{176C}\u{176E}-\u{1770}\u{1780}-\u{17B3}\u{17D7}",
    "\u{17DC}\u{1820}-\u{1877}\u{1880}-\u{18A8}\u{1900}-\u{191C}\u{1950}-\u{196D}\u{1970}-\u{1974}",
    "\u{1980}-\u{19A9}\u{19C1}-\u{19C7}\u{1A00}-\u{1A16}\u{1D00}-\u{1DBF}\u{1E00}-\u{1E9B}",
    "\u{1EA0}-\u{1EF9}\u{1F00}-\u{1F15}\u{1F18}-\u{1F1D}\u{1F20}-\u{1F45}\u{1F48}-\u{1F4D}",
    "\u{1F50}-\u{1F57}\u{1F59}\u{1F5B}\u{1F5D}\u{1F5F}-\u{1F7D}\u{1F80}-\u{1FB4}\u{1FB6}-\u{1FBC}",
    "\u{1FBE}\u{1FC2}-\u{1FC4}\u{1FC6}-\u{1FCC}\u{1FD0}-\u{1FD3}\u{1FD6}-\u{1FDB}\u{1FE0}-\u{1FEC}",
    "\u{1FF2}-\u{1FF4}\u{1FF6}-\u{1FFC}\u{2071}\u{207F}\u{2090}-\u{2094}\u{2102}\u{2107}",
    "\u{210A}-\u{2113}\u{2115}\u{2119}-\u{211D}\u{2124}\u{2126}\u{2128}\u{212A}-\u{212D}",
    "\u{212F}-\u{2131}\u{2133}-\u{2139}\u{213C}-\u{213F}\u{2145}-\u{2149}\u{2C00}-\u{2C2E}",
    "\u{2C30}-\u{2C5E}\u{2C80}-\u{2CE4}\u{2D00}-\u{2D25}\u{2D30}-\u{2D65}\u{2D6F}\u{2D80}-\u{2D96}",
    "\u{2DA0}-\u{2DA6}\u{2DA8}-\u{2DAE}\u{2DB0}-\u{2DB6}\u{2DB8}-\u{2DBE}\u{2DC0}-\u{2DC6}",
    "\u{2DC8}-\u{2DCE}\u{2DD0}-\u{2DD6}\u{2DD8}-\u{2DDE}\u{3005}-\u{3006}\u{3031}-\u{3035}",
    "\u{303B}-\u{303C}\u{3041}-\u{3096}\u{309D}-\u{309F}\u{30A1}-\u{30FA}\u{30FC}-\u{30FF}",
    "\u{3105}-\u{312C}\u{3131}-\u{318E}\u{31A0}-\u{31B7}\u{31F0}-\u{31FF}\u{3400}-\u{4DB5}",
    "\u{4E00}-\u{9FBB}\u{A000}-\u{A48C}\u{A800}-\u{A801}\u{A803}-\u{A805}\u{A807}-\u{A80A}",
    "\u{A80C}-\u{A822}\u{AC00}-\u{D7A3}\u{F900}-\u{FA2D}\u{FA30}-\u{FA6A}\u{FA70}-\u{FAD9}",
    "\u{FB00}-\u{FB06}\u{FB13}-\u{FB17}\u{FB1D}\u{FB1F}-\u{FB28}\u{FB2A}-\u{FB36}\u{FB38}-\u{FB3C}",
    "\u{FB3E}\u{FB40}-\u{FB41}\u{FB43}-\u{FB44}\u{FB46}-\u{FBB1}\u{FBD3}-\u{FD3D}\u{FD50}-\u{FD8F}",
    "\u{FD92}-\u{FDC7}\u{FDF0}-\u{FDFB}\u{FE70}-\u{FE74}\u{FE76}-\u{FEFC}\u{FF21}-\u{FF3A}",
    "\u{FF41}-\u{FF5A}\u{FF66}-\u{FFBE}\u{FFC2}-\u{FFC7}\u{FFCA}-\u{FFCF}\u{FFD2}-\u{FFD7}",
    "\u{FFDA}-\u{FFDC}",
  ].joined()

  // equivalent of \p{Mn}\p{Mc}
  let unicodeAccents = [
    "\u{0300}-\u{036F}\u{0483}-\u{0486}\u{0591}-\u{05B9}\u{05BB}-\u{05BD}\u{05BF}",
    "\u{05C1}-\u{05C2}\u{05C4}-\u{05C5}\u{05C7}\u{0610}-\u{0615}\u{064B}-\u{065E}\u{0670}",
    "\u{06D6}-\u{06DC}\u{06DF}-\u{06E4}\u{06E7}-\u{06E8}\u{06EA}-\u{06ED}\u{0711}\u{0730}-\u{074A}",
    "\u{07A6}-\u{07B0}\u{0901}-\u{0903}\u{093C}\u{093E}-\u{094D}\u{0951}-\u{0954}\u{0962}-\u{0963}",
    "\u{0981}-\u{0983}\u{09BC}\u{09BE}-\u{09C4}\u{09C7}-\u{09C8}\u{09CB}-\u{09CD}\u{09D7}",
    "\u{09E2}-\u{09E3}\u{0A01}-\u{0A03}\u{0A3C}\u{0A3E}-\u{0A42}\u{0A47}-\u{0A48}\u{0A4B}-\u{0A4D}",
    "\u{0A70}-\u{0A71}\u{0A81}-\u{0A83}\u{0ABC}\u{0ABE}-\u{0AC5}\u{0AC7}-\u{0AC9}\u{0ACB}-\u{0ACD}",
    "\u{0AE2}-\u{0AE3}\u{0B01}-\u{0B03}\u{0B3C}\u{0B3E}-\u{0B43}\u{0B47}-\u{0B48}\u{0B4B}-\u{0B4D}",
    "\u{0B56}-\u{0B57}\u{0B82}\u{0BBE}-\u{0BC2}\u{0BC6}-\u{0BC8}\u{0BCA}-\u{0BCD}\u{0BD7}",
    "\u{0C01}-\u{0C03}\u{0C3E}-\u{0C44}\u{0C46}-\u{0C48}\u{0C4A}-\u{0C4D}\u{0C55}-\u{0C56}",
    "\u{0C82}-\u{0C83}\u{0CBC}\u{0CBE}-\u{0CC4}\u{0CC6}-\u{0CC8}\u{0CCA}-\u{0CCD}\u{0CD5}-\u{0CD6}",
    "\u{0D02}-\u{0D03}\u{0D3E}-\u{0D43}\u{0D46}-\u{0D48}\u{0D4A}-\u{0D4D}\u{0D57}\u{0D82}-\u{0D83}",
    "\u{0DCA}\u{0DCF}-\u{0DD4}\u{0DD6}\u{0DD8}-\u{0DDF}\u{0DF2}-\u{0DF3}\u{0E31}\u{0E34}-\u{0E3A}",
    "\u{0E47}-\u{0E4E}\u{0EB1}\u{0EB4}-\u{0EB9}\u{0EBB}-\u{0EBC}\u{0EC8}-\u{0ECD}\u{0F18}-\u{0F19}",
    "\u{0F35}\u{0F37}\u{0F39}\u{0F3E}-\u{0F3F}\u{0F71}-\u{0F84}\u{0F86}-\u{0F87}\u{0F90}-\u{0F97}",
    "\u{0F99}-\u{0FBC}\u{0FC6}\u{102C}-\u{1032}\u{1036}-\u{1039}\u{1056}-\u{1059}\u{135F}",
    "\u{1712}-\u{1714}\u{1732}-\u{1734}\u{1752}-\u{1753}\u{1772}-\u{1773}\u{17B6}-\u{17D3}\u{17DD}",
    "\u{180B}-\u{180D}\u{18A9}\u{1920}-\u{192B}\u{1930}-\u{193B}\u{19B0}-\u{19C0}\u{19C8}-\u{19C9}",
    "\u{1A17}-\u{1A1B}\u{1DC0}-\u{1DC3}\u{20D0}-\u{20DC}\u{20E1}\u{20E5}-\u{20EB}\u{302A}-\u{302F}",
    "\u{3099}-\u{309A}\u{A802}\u{A806}\u{A80B}\u{A823}-\u{A827}\u{FB1E}\u{FE00}-\u{FE0F}",
    "\u{FE20}-\u{FE23}",
  ].joined()

  // equivalent of \p{Dn}
  let unicodeDigits = [
    "\u{0030}-\u{0039}\u{0660}-\u{0669}\u{06F0}-\u{06F9}\u{0966}-\u{096F}\u{09E6}-\u{09EF}",
    "\u{0A66}-\u{0A6F}\u{0AE6}-\u{0AEF}\u{0B66}-\u{0B6F}\u{0BE6}-\u{0BEF}\u{0C66}-\u{0C6F}",
    "\u{0CE6}-\u{0CEF}\u{0D66}-\u{0D6F}\u{0E50}-\u{0E59}\u{0ED0}-\u{0ED9}\u{0F20}-\u{0F29}",
    "\u{1040}-\u{1049}\u{17E0}-\u{17E9}\u{1810}-\u{1819}\u{1946}-\u{194F}\u{19D0}-\u{19D9}",
    "\u{FF10}-\u{FF19}",
  ].joined()

  // An alpha char is a unicode chars excluding unicode combining marks
  // but including other chars, a hashtag must start with one of these,
  // it does not make sense to have a combining mark before a base character.
  let alpha = unicodeLetters + otherChars

  // A numeric character is any with the number digit property, or
  // underscore. These characters can be included in hashtags, but a hashtag
  // cannot have only these characters.
  let numeric = unicodeDigits + "_"

  // Alphanumeric char is any alpha char or a unicode char with decimal
  // number property \p{Nd}
  let alphanumeric = alpha + unicodeAccents + numeric
  let hashChars = "#\u{FF03}"  // normal "#" or full-width "#"

  return (alpha, alphanumeric, hashChars)
}

/// `Canonicalize(ch)` of the ECMAScript spec for a regular expression with
/// the `i` flag and without `u`: the code unit's upper case when that is
/// one code unit, unless it maps a non-ASCII unit to ASCII.
private func canonicalize(_ unit: UInt16) -> UInt16 {
  guard let scalar = Unicode.Scalar(unit) else {
    return unit
  }
  let upper = Array(String(scalar).uppercased().utf16)
  guard upper.count == 1 else {
    return unit
  }
  if unit >= 128 && upper[0] < 128 {
    return unit
  }
  return upper[0]
}

private let CANONICAL: [UInt16] = (0...0xFFFF).map { canonicalize(UInt16($0)) }

/// A character class `[ranges]` (`[^ranges]` when `negated`) under the `i`
/// flag: it matches a code unit whose canonical form is the canonical form
/// of one of its members.
private struct CharacterClass: Sendable {
  private let canonicalMembers: [Bool]
  private let negated: Bool

  init(_ ranges: String, negated: Bool = false) {
    var members = [Bool](repeating: false, count: 0x10000)
    let units = Array(ranges.utf16)
    var i = 0
    while i < units.count {
      let low = units[i]
      if i + 2 < units.count && units[i + 1] == 0x2D {
        for unit in low...units[i + 2] {
          members[Int(CANONICAL[Int(unit)])] = true
        }
        i += 3
      } else {
        members[Int(CANONICAL[Int(low)])] = true
        i += 1
      }
    }
    canonicalMembers = members
    self.negated = negated
  }

  func matches(_ unit: UInt16) -> Bool {
    canonicalMembers[Int(CANONICAL[Int(unit)])] != negated
  }
}

/// `getHashtagRegexString()` run as `HashtagMatcher.exec`:
/// `(^|$|[^&/alphanumeric])([hashChars])([alphanumeric]*[alpha][alphanumeric]*)`
/// with the `i` flag.
private struct HashtagMatcher: Sendable {
  let hashtagAlpha: CharacterClass
  let hashtagAlphanumeric: CharacterClass
  let hashtagBoundary: CharacterClass
  let hashCharList: CharacterClass

  init() {
    let (alpha, alphanumeric, hashChars) = getHashtagRegexStringChars()
    hashtagAlpha = CharacterClass(alpha)
    hashtagAlphanumeric = CharacterClass(alphanumeric)
    hashtagBoundary = CharacterClass("&/" + alphanumeric, negated: true)
    hashCharList = CharacterClass(hashChars)
  }

  /// The first match, as `(index, the length of group 1, the length of
  /// group 3)`, or `nil`.
  func exec(_ text: String) -> (index: Int, boundary: Int, hashtag: Int)? {
    let units = Array(text.utf16)
    // Group 3 from `start`: the alphanumeric run there (`*` is greedy, and
    // backtracks only to find an alpha char in it), or nil without one.
    func hashtag(_ start: Int) -> Int? {
      var end = start
      var hasAlpha = false
      while end < units.count && hashtagAlphanumeric.matches(units[end]) {
        hasAlpha = hasAlpha || hashtagAlpha.matches(units[end])
        end += 1
      }
      return hasAlpha ? end - start : nil
    }
    func hashAt(_ i: Int) -> Bool { i < units.count && hashCharList.matches(units[i]) }
    for index in 0...units.count {
      // `^`
      if index == 0, hashAt(0), let length = hashtag(1) {
        return (index, 0, length)
      }
      // `$` is never followed by a hash char.
      if index < units.count, hashtagBoundary.matches(units[index]), hashAt(index + 1),
        let length = hashtag(index + 2)
      {
        return (index, 1, length)
      }
    }
    return nil
  }
}

private let REGEX = HashtagMatcher()

private func getHashtagMatch(_ text: String) -> EntityMatch? {
  guard let matchArr = REGEX.exec(text) else {
    return nil
  }
  let hashtagLength = matchArr.hashtag + 1
  let startOffset = matchArr.index + matchArr.boundary
  let endOffset = startOffset + hashtagLength
  return EntityMatch(start: startOffset, end: endOffset)
}

/// `HashtagConfig`.
public struct HashtagConfig {
  /// The matching function used by the extension. Has a default
  /// implementation that should be suitable for most use cases. Returns
  /// `nil` if no match, otherwise the start and end (UTF-16 offsets) of the
  /// first `#hashtag` match.
  public var getHashtagMatch: (String) -> EntityMatch?

  public init(getHashtagMatch: @escaping (String) -> EntityMatch? = defaultGetHashtagMatch) {
    self.getHashtagMatch = getHashtagMatch
  }
}

/// The default `HashtagConfig.getHashtagMatch`.
public func defaultGetHashtagMatch(_ text: String) -> EntityMatch? {
  getHashtagMatch(text)
}

/// `registerLexicalHashtag(editor, config)`: turns `#hashtag` text into
/// `HashtagNode`s and back.
public func registerLexicalHashtag(
  _ editor: Editor, _ config: HashtagConfig = HashtagConfig()
) throws -> () -> Void {
  mergeRegister(
    try registerLexicalTextEntity(editor, config.getHashtagMatch, HashtagNode.self) {
      try createHashtagNode($0.getTextContent())
    })
}

/// `HashtagExtension`: registers `HashtagNode` and the hashtag transforms.
public let HashtagExtension = defineExtension(
  name: "@lexical/hashtag/Hashtag", config: HashtagConfig(),
  nodes: { [.klass(HashtagNode.self)] },
  register: { editor, config, _ in try registerLexicalHashtag(editor, config) })

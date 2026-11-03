#if canImport(UIKit)
  import Foundation

  /// The text view's undo manager: the editor's history, so the system's
  /// undo (Command-Z, the three-finger swipe and shake, the keyboard's undo
  /// key) undoes through `historyUndo` / `historyRedo` like the editor's
  /// own keys. Undo actions the system registers (the keyboard registers
  /// one per typing run) are dropped, since the history already holds them.
  final class HistoryUndoManager: UndoManager {
    var canUndoEdit: () -> Bool = { false }
    var canRedoEdit: () -> Bool = { false }
    var onUndo: () -> Void = {}
    var onRedo: () -> Void = {}

    override var canUndo: Bool { canUndoEdit() }
    override var canRedo: Bool { canRedoEdit() }
    override var isUndoRegistrationEnabled: Bool { false }
    override func undo() { onUndo() }
    override func redo() { onRedo() }
    override func undoNestedGroup() { onUndo() }
    override func registerUndo(withTarget target: Any, selector: Selector, object: Any?) {}
    override func prepare(withInvocationTarget target: Any) -> Any { self }
    override func beginUndoGrouping() {}
    override func endUndoGrouping() {}
    override var groupingLevel: Int { 0 }
  }
#endif

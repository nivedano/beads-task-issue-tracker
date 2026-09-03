/**
 * Pure helpers behind the in-app Edit menu (Windows/Linux).
 *
 * The native Edit menu uses predefined OS items; the in-app menu bar has to
 * apply the same edits to the focused field itself, so the value/caret maths
 * lives here where it can be tested.
 */

export interface FieldSelection {
  value: string
  selectionStart: number | null
  selectionEnd: number | null
}

export interface PasteResult {
  value: string
  caret: number
}

/** Replace the current selection (or insert at the caret) with `text`. */
export function computePaste(field: FieldSelection, text: string): PasteResult {
  const rawEnd = field.selectionEnd ?? field.value.length
  const rawStart = field.selectionStart ?? rawEnd
  const start = Math.min(rawStart, rawEnd)
  const end = Math.max(rawStart, rawEnd)
  return {
    value: field.value.slice(0, start) + text + field.value.slice(end),
    caret: start + text.length,
  }
}

/** Text the Copy/Cut commands should act on, or '' when there is no selection. */
export function selectedText(field: FieldSelection): string {
  const start = Math.min(field.selectionStart ?? 0, field.selectionEnd ?? 0)
  const end = Math.max(field.selectionStart ?? 0, field.selectionEnd ?? 0)
  if (end <= start) return ''
  return field.value.slice(start, end)
}

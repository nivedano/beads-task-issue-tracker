/**
 * Keyboard shortcut matching for the webview-handled accelerators.
 *
 * Windows/Linux have no native menu to carry accelerators (see AppMenuBar), so
 * the shortcuts it advertises are matched here. Kept pure for testing.
 */

export interface KeyEventLike {
  key: string
  code?: string
  ctrlKey: boolean
  metaKey: boolean
  altKey: boolean
  shiftKey: boolean
}

export interface ShortcutSpec {
  /** Compared case-insensitively against `event.key`. */
  key: string
  /** Prefer a physical key when Shift changes `event.key` (for example 1 → !). */
  code?: string
  /** Fallback when `event.code` is unavailable but Shift changes the character. */
  shiftedKey?: string
  /** Ctrl on Windows/Linux, Cmd on macOS. Defaults to true. */
  ctrlOrMeta?: boolean
  /** Require Ctrl specifically, leaving platform-reserved Cmd shortcuts untouched. */
  ctrlOnly?: boolean
  /** Defaults to false. */
  alt?: boolean
  /** Defaults to false — Shift must not be held unless the spec asks for it. */
  shift?: boolean
}

export function matchesShortcut(event: KeyEventLike, spec: ShortcutSpec): boolean {
  if (spec.code) {
    if (event.code) {
      if (event.code !== spec.code) return false
    } else {
      const normalizedKey = event.key.toLowerCase()
      const matchesBaseKey = normalizedKey === spec.key.toLowerCase()
      const matchesShiftedKey = spec.shiftedKey !== undefined && normalizedKey === spec.shiftedKey.toLowerCase()
      if (!matchesBaseKey && !matchesShiftedKey) return false
    }
  } else if (event.key.toLowerCase() !== spec.key.toLowerCase()) {
    return false
  }
  if (spec.ctrlOnly) {
    if (!event.ctrlKey || event.metaKey) return false
  } else if ((spec.ctrlOrMeta ?? true) !== (event.ctrlKey || event.metaKey)) {
    return false
  }
  if ((spec.alt ?? false) !== event.altKey) return false
  return (spec.shift ?? false) === event.shiftKey
}

export const SHORTCUT_FIND: ShortcutSpec = { key: 'f' }
export const SHORTCUT_HELP: ShortcutSpec = { key: 'F1', ctrlOrMeta: false }
export const SHORTCUT_NEW_ISSUE: ShortcutSpec = { key: 'n' }
export const SHORTCUT_DELETE_ISSUE: ShortcutSpec = { key: 'd' }
export const SHORTCUT_SETTINGS: ShortcutSpec = { key: ',' }
export const SHORTCUT_LOGS: ShortcutSpec = { key: 'l', shift: true }
export const SHORTCUT_DASHBOARD_PANEL: ShortcutSpec = { key: '1', code: 'Digit1', shiftedKey: '!', shift: true }
export const SHORTCUT_DETAILS_PANEL: ShortcutSpec = { key: '2', code: 'Digit2', shiftedKey: '@', shift: true }
export const SHORTCUT_FOCUS_ISSUE_LIST: ShortcutSpec = { key: 'q', ctrlOnly: true }
export const SHORTCUT_FOCUS_DASHBOARD: ShortcutSpec = { key: '1', code: 'Digit1' }
export const SHORTCUT_FOCUS_DETAILS: ShortcutSpec = { key: '2', code: 'Digit2' }
export const SHORTCUT_SELECT_PROJECT: ShortcutSpec = { key: 'o', shift: true }
export const SHORTCUT_TOGGLE_MAXIMIZE: ShortcutSpec = { key: 'Enter', ctrlOrMeta: false, alt: true }

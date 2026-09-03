import { describe, expect, it } from 'vitest'
import {
  matchesShortcut,
  SHORTCUT_DASHBOARD_PANEL,
  SHORTCUT_DELETE_ISSUE,
  SHORTCUT_DETAILS_PANEL,
  SHORTCUT_FIND,
  SHORTCUT_FOCUS_DASHBOARD,
  SHORTCUT_FOCUS_DETAILS,
  SHORTCUT_FOCUS_ISSUE_LIST,
  SHORTCUT_HELP,
  SHORTCUT_LOGS,
  SHORTCUT_NEW_ISSUE,
  SHORTCUT_SELECT_PROJECT,
  SHORTCUT_SETTINGS,
  SHORTCUT_TOGGLE_MAXIMIZE,
  type KeyEventLike,
} from '../../app/utils/shortcuts'

const event = (overrides: Partial<KeyEventLike>): KeyEventLike => ({
  key: 'f',
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...overrides,
})

describe('matchesShortcut', () => {
  it('matches Ctrl+F', () => {
    expect(matchesShortcut(event({ key: 'f', ctrlKey: true }), SHORTCUT_FIND)).toBe(true)
  })

  it('matches Cmd+F', () => {
    expect(matchesShortcut(event({ key: 'f', metaKey: true }), SHORTCUT_FIND)).toBe(true)
  })

  it('matches plain F1 for help and rejects modified variants', () => {
    expect(matchesShortcut(event({ key: 'F1' }), SHORTCUT_HELP)).toBe(true)
    expect(matchesShortcut(event({ key: 'F1', ctrlKey: true }), SHORTCUT_HELP)).toBe(false)
    expect(matchesShortcut(event({ key: 'F1', shiftKey: true }), SHORTCUT_HELP)).toBe(false)
  })

  it('matches Ctrl/Cmd+N for a new issue', () => {
    expect(matchesShortcut(event({ key: 'n', ctrlKey: true }), SHORTCUT_NEW_ISSUE)).toBe(true)
    expect(matchesShortcut(event({ key: 'N', metaKey: true }), SHORTCUT_NEW_ISSUE)).toBe(true)
    expect(matchesShortcut(event({ key: 'n' }), SHORTCUT_NEW_ISSUE)).toBe(false)
  })

  it('matches the remaining global action shortcuts', () => {
    expect(matchesShortcut(event({ key: 'd', ctrlKey: true }), SHORTCUT_DELETE_ISSUE)).toBe(true)
    expect(matchesShortcut(event({ key: 'O', metaKey: true, shiftKey: true }), SHORTCUT_SELECT_PROJECT)).toBe(true)
  })

  it('ignores the plain key without a modifier', () => {
    expect(matchesShortcut(event({ key: 'f' }), SHORTCUT_FIND)).toBe(false)
  })

  it('ignores Alt combinations', () => {
    expect(matchesShortcut(event({ key: 'f', ctrlKey: true, altKey: true }), SHORTCUT_FIND)).toBe(false)
  })

  it('rejects an unwanted Shift', () => {
    expect(matchesShortcut(event({ key: 'f', ctrlKey: true, shiftKey: true }), SHORTCUT_FIND)).toBe(false)
  })

  it('requires Shift when the spec asks for it', () => {
    expect(matchesShortcut(event({ key: 'L', ctrlKey: true, shiftKey: true }), SHORTCUT_LOGS)).toBe(true)
    expect(matchesShortcut(event({ key: 'l', ctrlKey: true }), SHORTCUT_LOGS)).toBe(false)
  })

  it('matches punctuation keys', () => {
    expect(matchesShortcut(event({ key: ',', ctrlKey: true }), SHORTCUT_SETTINGS)).toBe(true)
    expect(matchesShortcut(event({ key: ',', ctrlKey: true, shiftKey: true }), SHORTCUT_SETTINGS)).toBe(false)
  })

  it('does not match a different key', () => {
    expect(matchesShortcut(event({ key: 'g', ctrlKey: true }), SHORTCUT_FIND)).toBe(false)
  })

  it('matches shifted digit shortcuts by physical key code', () => {
    expect(matchesShortcut(event({ key: '!', code: 'Digit1', ctrlKey: true, shiftKey: true }), SHORTCUT_DASHBOARD_PANEL)).toBe(true)
    expect(matchesShortcut(event({ key: '@', code: 'Digit2', metaKey: true, shiftKey: true }), SHORTCUT_DETAILS_PANEL)).toBe(true)
  })

  it('rejects the wrong physical digit for panel shortcuts', () => {
    expect(matchesShortcut(event({ key: '!', code: 'Digit2', ctrlKey: true, shiftKey: true }), SHORTCUT_DASHBOARD_PANEL)).toBe(false)
  })

  it('distinguishes focus shortcuts from shifted panel toggles', () => {
    expect(matchesShortcut(event({ key: '1', code: 'Digit1', ctrlKey: true }), SHORTCUT_FOCUS_DASHBOARD)).toBe(true)
    expect(matchesShortcut(event({ key: '2', code: 'Digit2', metaKey: true }), SHORTCUT_FOCUS_DETAILS)).toBe(true)
    expect(matchesShortcut(event({ key: 'q', ctrlKey: true }), SHORTCUT_FOCUS_ISSUE_LIST)).toBe(true)
    expect(matchesShortcut(event({ key: 'Q', metaKey: true }), SHORTCUT_FOCUS_ISSUE_LIST)).toBe(false)
    expect(matchesShortcut(event({ key: 'q', ctrlKey: true, shiftKey: true }), SHORTCUT_FOCUS_ISSUE_LIST)).toBe(false)
    expect(matchesShortcut(event({ key: '!', code: 'Digit1', ctrlKey: true, shiftKey: true }), SHORTCUT_FOCUS_DASHBOARD)).toBe(false)
  })

  it('falls back to shifted characters when event.code is unavailable', () => {
    expect(matchesShortcut(event({ key: '!', ctrlKey: true, shiftKey: true }), SHORTCUT_DASHBOARD_PANEL)).toBe(true)
    expect(matchesShortcut(event({ key: '@', metaKey: true, shiftKey: true }), SHORTCUT_DETAILS_PANEL)).toBe(true)
  })

  it('matches Alt+Enter only for maximize/restore', () => {
    expect(matchesShortcut(event({ key: 'Enter', altKey: true }), SHORTCUT_TOGGLE_MAXIMIZE)).toBe(true)
    expect(matchesShortcut(event({ key: 'Enter' }), SHORTCUT_TOGGLE_MAXIMIZE)).toBe(false)
    expect(matchesShortcut(event({ key: 'Enter', altKey: true, ctrlKey: true }), SHORTCUT_TOGGLE_MAXIMIZE)).toBe(false)
  })
})

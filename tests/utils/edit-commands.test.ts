import { describe, expect, it } from 'vitest'
import { computePaste, selectedText } from '../../app/utils/edit-commands'

describe('computePaste', () => {
  it('inserts at the caret when nothing is selected', () => {
    expect(computePaste({ value: 'abcd', selectionStart: 2, selectionEnd: 2 }, 'XY'))
      .toEqual({ value: 'abXYcd', caret: 4 })
  })

  it('replaces the selected range', () => {
    expect(computePaste({ value: 'abcd', selectionStart: 1, selectionEnd: 3 }, 'Z'))
      .toEqual({ value: 'aZd', caret: 2 })
  })

  it('appends when the selection is unknown', () => {
    expect(computePaste({ value: 'ab', selectionStart: null, selectionEnd: null }, '!'))
      .toEqual({ value: 'ab!', caret: 3 })
  })

  it('handles a reversed selection', () => {
    expect(computePaste({ value: 'abcd', selectionStart: 3, selectionEnd: 1 }, 'Z'))
      .toEqual({ value: 'aZd', caret: 2 })
  })
})

describe('selectedText', () => {
  it('returns the selected slice', () => {
    expect(selectedText({ value: 'hello', selectionStart: 1, selectionEnd: 4 })).toBe('ell')
  })

  it('returns an empty string for a collapsed or missing selection', () => {
    expect(selectedText({ value: 'hello', selectionStart: 2, selectionEnd: 2 })).toBe('')
    expect(selectedText({ value: 'hello', selectionStart: null, selectionEnd: null })).toBe('')
  })
})

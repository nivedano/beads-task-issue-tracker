import { describe, it, expect } from 'vitest'
import { posix, win32 } from 'node:path'
import { isWithinRoot } from '../../server/utils/lexicon-path'

describe('isWithinRoot', () => {
  it('accepts a project that is a Windows drive root', () => {
    expect(
      isWithinRoot('D:\\', 'D:\\models\\architecture\\term-lexicon.yaml', win32),
    ).toBe(true)
  })

  it('rejects a path on another drive from a drive root', () => {
    expect(isWithinRoot('D:\\', 'C:\\models\\architecture\\term-lexicon.yaml', win32)).toBe(false)
  })

  it('accepts a project at the POSIX root', () => {
    expect(isWithinRoot('/', '/models/architecture/term-lexicon.yaml', posix)).toBe(true)
  })

  it('rejects escapes, siblings and similarly named prefixes', () => {
    expect(isWithinRoot('/proj', '/proj/../x', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/other/models/x.yaml', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/project/models/x.yaml', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/proj/models/x.yaml', posix)).toBe(true)
  })
})
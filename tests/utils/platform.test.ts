import { describe, expect, it } from 'vitest'
import { detectPlatform } from '../../app/utils/platform'

describe('detectPlatform', () => {
  it('detects macOS from a WebKit user agent', () => {
    expect(detectPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15')).toBe('macos')
  })

  it('detects Windows from a WebView2 user agent', () => {
    expect(detectPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0')).toBe('windows')
  })

  it('falls back to linux for anything else', () => {
    expect(detectPlatform('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36')).toBe('linux')
    expect(detectPlatform('')).toBe('linux')
  })

  it('is case insensitive', () => {
    expect(detectPlatform('MACINTOSH')).toBe('macos')
    expect(detectPlatform('WINDOWS NT')).toBe('windows')
  })
})

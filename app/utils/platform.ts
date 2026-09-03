/**
 * Platform detection for window-chrome differences (Windows/macOS/Linux).
 *
 * macOS keeps the native application menu and the overlay title bar with its
 * traffic lights; Windows runs undecorated with an in-app title bar. Kept pure
 * so it can be unit-tested without a browser.
 */

export type Platform = 'macos' | 'windows' | 'linux'

export function detectPlatform(userAgent: string): Platform {
  const ua = userAgent.toLowerCase()
  if (ua.includes('mac os') || ua.includes('macintosh')) return 'macos'
  if (ua.includes('windows') || ua.includes('win32') || ua.includes('win64')) return 'windows'
  return 'linux'
}

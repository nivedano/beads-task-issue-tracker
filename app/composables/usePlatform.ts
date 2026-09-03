import { detectPlatform, type Platform } from '~/utils/platform'

// Resolved once — the platform cannot change during a session.
const platform: Platform = import.meta.client ? detectPlatform(navigator.userAgent) : 'linux'

export function usePlatform() {
  return {
    platform,
    isMacOS: platform === 'macos',
    isWindows: platform === 'windows',
    isLinux: platform === 'linux',
  }
}

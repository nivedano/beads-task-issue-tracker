/**
 * Shared display helpers for the GRACE detail components.
 * Presentation only — no data logic.
 */

/** Uppercase sub-heading used inside detail sections. */
export const graceSectionHeading = 'text-[10px] font-medium text-muted-foreground uppercase tracking-wide mb-1'

/** Badge classes for a check/receipt verdict (display-level, no re-verification). */
export function graceVerdictClass(verdict: string): string {
  switch (verdict) {
    case 'passed': return 'bg-green-600 text-white'
    case 'failed': return 'bg-destructive text-destructive-foreground'
    case 'skipped': return 'bg-muted text-muted-foreground'
    default: return 'bg-secondary text-secondary-foreground'
  }
}

/**
 * Shared display helpers for the GRACE detail components.
 * Presentation only — no data logic.
 */

/** Uppercase sub-heading used inside detail sections. */
export const graceSectionHeading = 'text-[10px] font-medium text-muted-foreground uppercase tracking-wide mb-1'

/**
 * Colours for GRACE record badges come from the theme's badge tokens
 * (`bg-status-*-gradient`, `--destructive`, secondary/muted) so they recolour
 * with the active theme exactly like the issue badges. Only the colour class
 * is supplied: record badges stay compact with their own `text-[10px] px-1 py-0`.
 */

/** Badge classes for a check/receipt verdict (display-level, no re-verification). */
export function graceVerdictClass(verdict: string): string {
  switch (verdict) {
    case 'passed': return 'bg-status-in-progress-gradient text-white'
    case 'failed': return 'bg-destructive text-destructive-foreground'
    case 'blocked': return 'bg-status-blocked-gradient text-white'
    case 'interrupted':
    case 'cancelled':
    case 'canceled':
    case 'refused': return 'bg-status-deferred-gradient text-white'
    case 'skipped':
    case 'not-attempted':
    case 'not attempted':
    case 'not_attempted': return 'bg-muted text-muted-foreground'
    default: return 'bg-secondary text-secondary-foreground'
  }
}

/** Badge classes for an approval decision. */
export function graceDecisionClass(decision: string): string {
  switch (decision) {
    case 'approved': return 'bg-status-in-progress-gradient text-white'
    case 'rejected': return 'bg-destructive text-destructive-foreground'
    default: return 'bg-secondary text-secondary-foreground'
  }
}

/** Badge classes for a check-run status. */
export function graceRunStatusClass(status: string): string {
  switch (status) {
    case 'completed': return 'bg-status-open-gradient text-white'
    case 'interrupted': return 'bg-status-deferred-gradient text-white'
    default: return 'bg-secondary text-secondary-foreground'
  }
}

/** Badge classes for a review status. */
export function graceReviewStatusClass(status: string): string {
  switch (status) {
    case 'completed': return 'bg-status-pinned-gradient text-white'
    default: return 'bg-secondary text-secondary-foreground'
  }
}

/** Badge classes for the close record marker. */
export const graceCloseBadgeClass = 'bg-status-closed-gradient text-white'
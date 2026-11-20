<script setup lang="ts">
import type { GraceApproval, GraceRun, GraceReview, GraceClose, GraceBrief } from '~/types/grace'
import { Badge } from '~/components/ui/badge'
import { graceSectionHeading as sectionHeading, graceVerdictClass } from '~/components/grace/grace-display'
import { Button } from '~/components/ui/button'

const props = defineProps<{
  approvals: GraceApproval[]
  runs: GraceRun[]
  reviews: GraceReview[]
  close: GraceClose | null
  briefs: GraceBrief[]
}>()

const emit = defineEmits<{
  'open-log': [log: string]
  'open-file': [relPath: string]
}>()

// section heading + verdict classes come from grace-display

const approvalsSorted = computed(() => [...props.approvals].sort((a, b) => a.revision - b.revision))
const runsSorted = computed(() => [...props.runs].sort((a, b) => a.n - b.n))
const reviewsSorted = computed(() => [...props.reviews].sort((a, b) => a.n - b.n))
const briefsSorted = computed(() => [...props.briefs].sort((a, b) => a.revision - b.revision))

const expandedRuns = ref<Set<number>>(new Set())

const toggleRun = (n: number) => {
  const next = new Set(expandedRuns.value)
  if (next.has(n)) next.delete(n)
  else next.add(n)
  expandedRuns.value = next
}

const decisionClass = (decision: string) => {
  switch (decision) {
    case 'approved': return 'bg-green-600 text-white'
    case 'rejected': return 'bg-destructive text-destructive-foreground'
    default: return 'bg-secondary text-secondary-foreground'
  }
}
</script>

<template>
  <div class="space-y-5">
    <!-- Approvals -->
    <section>
      <h4 :class="sectionHeading">Approvals</h4>
      <p v-if="approvalsSorted.length === 0" class="text-xs text-muted-foreground">No approval records</p>
      <div v-else class="space-y-2">
        <div
          v-for="approval in approvalsSorted"
          :key="approval.file"
          class="rounded border border-border p-2 space-y-1"
        >
          <div class="flex flex-wrap items-center gap-1.5 text-xs">
            <Badge variant="outline" class="text-[10px] px-1 py-0 font-mono">revision {{ approval.revision }}</Badge>
            <Badge :class="decisionClass(approval.decision)" variant="secondary" class="text-[10px] px-1 py-0">
              {{ approval.decision }}
            </Badge>
            <span v-if="approval.date" class="text-[10px] text-muted-foreground">{{ approval.date }}</span>
            <span class="text-[10px] text-muted-foreground font-mono">{{ approval.file }}</span>
          </div>
          <p v-if="approval.statement" class="text-xs whitespace-pre-line">{{ approval.statement }}</p>
          <div class="text-[10px] text-muted-foreground space-y-0.5">
            <div>contract-sha256: <span class="font-mono break-all">{{ approval.contractSha256 }}</span></div>
            <div v-if="approval.briefSha256">brief-sha256: <span class="font-mono break-all">{{ approval.briefSha256 }}</span></div>
          </div>
          <div v-if="approval.bound.length > 0" class="text-[10px] text-muted-foreground space-y-0.5">
            <div class="uppercase tracking-wide">Bound</div>
            <div v-for="bound in approval.bound" :key="bound.check" class="flex items-start gap-1.5">
              <span class="font-mono shrink-0">{{ bound.check }}</span>
              <span class="font-mono break-all">{{ bound.definitionSha256 }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Runs -->
    <section>
      <h4 :class="sectionHeading">Runs</h4>
      <p v-if="runsSorted.length === 0" class="text-xs text-muted-foreground">No check runs recorded</p>
      <div v-else class="space-y-2">
        <div v-for="run in runsSorted" :key="run.file" class="rounded border border-border">
          <button
            class="w-full flex flex-wrap items-center gap-1.5 px-2 py-1.5 text-xs text-left hover:bg-secondary/30"
            @click="toggleRun(run.n)"
          >
            <svg
              class="w-3 h-3 text-muted-foreground transition-transform"
              :class="{ 'rotate-90': expandedRuns.has(run.n) }"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="9 18 15 12 9 6" />
            </svg>
            <span class="font-mono">run {{ run.n }}</span>
            <Badge variant="secondary" class="text-[10px] px-1 py-0">{{ run.status }}</Badge>
            <span v-if="run.start" class="text-[10px] text-muted-foreground">start</span>
            <span v-if="run.dirty" class="text-[10px] text-amber-500">dirty</span>
            <span class="text-[10px] text-muted-foreground">{{ run.started }}</span>
            <span class="text-[10px] text-muted-foreground">{{ run.kernel }}</span>
          </button>

          <div v-if="expandedRuns.has(run.n)" class="border-t border-border p-2 space-y-2">
            <div class="grid gap-1 text-[10px] text-muted-foreground sm:grid-cols-2">
              <div>run-id: <span class="font-mono break-all">{{ run.runId }}</span></div>
              <div>revision: <span class="font-mono">{{ run.revision }}</span></div>
              <div>started: {{ run.started }}</div>
              <div>ended: {{ run.ended }}</div>
              <div>source-revision: <span class="font-mono break-all">{{ run.sourceRevision }}</span></div>
              <div>candidate: <span class="font-mono break-all">{{ run.candidate }}</span></div>
              <div>contract-sha256: <span class="font-mono break-all">{{ run.contractSha256 }}</span></div>
            </div>
            <div v-if="run.preexisting.length > 0" class="text-[10px] text-muted-foreground">
              preexisting: {{ run.preexisting.join(', ') }}
            </div>

            <div class="space-y-1">
              <div
                v-for="result in run.results"
                :key="result.check"
                class="rounded bg-secondary/20 px-2 py-1 text-[10px]"
              >
                <div class="flex flex-wrap items-center gap-1.5">
                  <span class="font-mono text-xs">{{ result.check }}</span>
                  <Badge :class="graceVerdictClass(result.verdict)" variant="secondary" class="text-[10px] px-1 py-0">
                    {{ result.verdict }}
                  </Badge>
                  <span class="text-muted-foreground">{{ result.kind }}</span>
                  <span v-if="!result.required" class="text-muted-foreground">optional</span>
                  <span v-if="result.claim" class="text-muted-foreground">{{ result.claim }}</span>
                  <span v-if="result.exit !== null" class="text-muted-foreground">exit {{ result.exit }}</span>
                  <span v-if="result.elapsedMs !== null" class="text-muted-foreground">{{ result.elapsedMs }} ms</span>
                  <Button
                    v-if="result.log"
                    variant="outline"
                    size="sm"
                    class="h-5 px-1.5 text-[10px]"
                    @click="emit('open-log', result.log!)"
                  >
                    Log
                  </Button>
                </div>
                <div v-if="result.reason" class="text-destructive">reason: {{ result.reason }}</div>
                <div v-if="result.trace" class="text-muted-foreground">
                  trace matched {{ result.trace.matched }}/{{ result.trace.expected }}
                  <span v-if="result.trace.missing.length > 0" class="text-destructive">
                    · missing: {{ result.trace.missing.join(', ') }}
                  </span>
                  <span v-if="result.trace.forbidden.length > 0" class="text-destructive">
                    · forbidden: {{ result.trace.forbidden.join(', ') }}
                  </span>
                </div>
                <div v-if="result.observation" class="text-muted-foreground">
                  observation at {{ result.observation.at }} · build {{ result.observation.build }} ·
                  {{ result.observation.environment }}: {{ result.observation.note }}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Reviews -->
    <section>
      <h4 :class="sectionHeading">Reviews</h4>
      <p v-if="reviewsSorted.length === 0" class="text-xs text-muted-foreground">No reviews recorded</p>
      <div v-else class="space-y-2">
        <div
          v-for="review in reviewsSorted"
          :key="review.file"
          class="rounded border border-border p-2 space-y-1.5"
        >
          <div class="flex flex-wrap items-center gap-1.5 text-xs">
            <span class="font-mono">review {{ review.n }}</span>
            <Badge variant="outline" class="text-[10px] px-1 py-0 font-mono">revision {{ review.revision }}</Badge>
            <Badge variant="secondary" class="text-[10px] px-1 py-0">{{ review.status }}</Badge>
            <span class="text-[10px] text-muted-foreground">{{ review.independence }}</span>
            <span class="text-[10px] text-muted-foreground font-mono">{{ review.file }}</span>
          </div>
          <div class="text-[10px] text-muted-foreground">
            revisions {{ review.startRevision }} → {{ review.endRevision }} · candidate
            <span class="font-mono break-all">{{ review.candidate }}</span>
          </div>

          <div v-if="review.findings.length > 0" class="text-[10px]">
            <div class="uppercase tracking-wide text-muted-foreground mb-0.5">Findings</div>
            <div
              v-for="finding in review.findings"
              :key="finding.id"
              class="flex flex-wrap items-center gap-x-2 gap-y-0.5 py-0.5 border-b border-border/50 last:border-b-0"
            >
              <span class="font-mono">{{ finding.id }}</span>
              <span class="text-muted-foreground">{{ finding.severity }}</span>
              <span>{{ finding.status }}</span>
              <span v-if="finding.ownerDecision" class="text-muted-foreground">
                owner: {{ finding.ownerDecision }}
              </span>
            </div>
          </div>

          <div v-if="review.adequacy.length > 0" class="text-[10px] text-muted-foreground">
            <div class="uppercase tracking-wide mb-0.5">Adequacy</div>
            <div v-for="item in review.adequacy" :key="item.check" class="flex items-center gap-1.5">
              <span class="font-mono">{{ item.check }}</span>
              <span>{{ item.discriminates ? 'discriminates' : 'does not discriminate' }}</span>
            </div>
          </div>

          <div v-if="review.rules.length > 0" class="text-[10px] text-muted-foreground">
            <div class="uppercase tracking-wide mb-0.5">Rules</div>
            <div v-for="rule in review.rules" :key="rule.rule" class="flex items-start gap-1.5">
              <span class="font-mono shrink-0">{{ rule.rule }}</span>
              <span>{{ rule.verdict }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Close -->
    <section v-if="close">
      <h4 :class="sectionHeading">Close</h4>
      <div class="rounded border border-border p-2 space-y-1.5">
        <div class="flex flex-wrap items-center gap-1.5 text-xs">
          <Badge variant="outline" class="text-[10px] px-1 py-0 font-mono">revision {{ close.revision }}</Badge>
          <span class="text-[10px] text-muted-foreground">{{ close.date }}</span>
          <span class="text-[10px] text-muted-foreground">{{ close.kernel }}</span>
          <span class="text-[10px] text-muted-foreground font-mono">{{ close.file }}</span>
        </div>
        <div class="text-[10px] text-muted-foreground">
          approval {{ close.approval }} · review {{ close.review }}
          <span v-if="close.brief"> · brief {{ close.brief }}</span>
        </div>
        <div class="text-[10px] text-muted-foreground">
          revisions {{ close.startRevision }} → {{ close.sourceRevision }} · candidate
          <span class="font-mono break-all">{{ close.candidate }}</span>
        </div>

        <div class="text-[10px]">
          <div class="uppercase tracking-wide text-muted-foreground mb-0.5">Satisfied</div>
          <div
            v-for="item in close.satisfied"
            :key="item.check"
            class="flex flex-wrap items-center gap-x-2 gap-y-0.5 py-0.5 border-b border-border/50 last:border-b-0"
          >
            <span class="font-mono">{{ item.check }}</span>
            <span class="text-muted-foreground">{{ item.kind }}</span>
            <span class="text-muted-foreground">run {{ item.run }}</span>
            <span>{{ item.claim }}</span>
          </div>
        </div>

        <div v-if="close.notRequired.length > 0" class="text-[10px] text-muted-foreground">
          <div class="uppercase tracking-wide mb-0.5">Not required</div>
          <div v-for="item in close.notRequired" :key="item.check" class="flex items-center gap-1.5">
            <span class="font-mono">{{ item.check }}</span>
            <span>{{ item.verdict ?? '—' }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Briefs -->
    <section>
      <h4 :class="sectionHeading">Briefs</h4>
      <p v-if="briefsSorted.length === 0" class="text-xs text-muted-foreground">No briefs recorded</p>
      <div v-else class="space-y-1">
        <div
          v-for="brief in briefsSorted"
          :key="brief.file"
          class="flex items-center gap-2 text-xs"
        >
          <span class="font-mono text-[10px] text-muted-foreground">revision {{ brief.revision }}</span>
          <span class="font-mono truncate">{{ brief.file }}</span>
          <Button
            variant="outline"
            size="sm"
            class="h-5 px-1.5 text-[10px] shrink-0"
            @click="emit('open-file', `records/${brief.file}`)"
          >
            Open externally
          </Button>
        </div>
      </div>
    </section>
  </div>
</template>

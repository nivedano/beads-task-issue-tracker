<script setup lang="ts">
import type { GraceChangeDetail } from '~/types/grace'
import { graceReadFile, graceOpenFile } from '~/utils/bd-api'
import { graceLocation, toGraceRelative } from '~/utils/grace-helpers'
import { CopyableId } from '~/components/ui/copyable-id'
import { Button } from '~/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '~/components/ui/dialog'
import { ScrollArea } from '~/components/ui/scroll-area'
import CollapsibleSection from '~/components/layout/CollapsibleSection.vue'
import GraceStageBadge from '~/components/grace/GraceStageBadge.vue'
import GraceAuthorizationBadge from '~/components/grace/GraceAuthorizationBadge.vue'
import GraceLifecycleStepper from '~/components/grace/GraceLifecycleStepper.vue'
import GraceContractView from '~/components/grace/GraceContractView.vue'
import GraceChecksTable from '~/components/grace/GraceChecksTable.vue'
import GraceRecordsTimeline from '~/components/grace/GraceRecordsTimeline.vue'
import GraceNotesView from '~/components/grace/GraceNotesView.vue'

const props = withDefaults(defineProps<{
  detail: GraceChangeDetail | null
  isLoading: boolean
  pinWarning: string | null
  /** Optional: Task 10 passes grace.error so a failed detail fetch is explainable. */
  error?: string | null
}>(), {
  error: null,
})

const emit = defineEmits<{
  'open-file': [relPath: string]
}>()

const summary = computed(() => props.detail?.summary ?? null)

/**
 * `detail.dir` is the absolute change directory inside `.grace5`. The split
 * into project root + change-relative prefix is pure (and tested) in
 * `~/utils/grace-helpers`; log paths recorded in run results are relative to
 * the change dir.
 */
const location = computed(() => graceLocation(props.detail?.dir))

// Collapsible section state (view-local; not part of the persisted storage contract)
const openSections = reactive({ contract: true, checks: true, records: true, notes: true })

const toggleSection = (section: keyof typeof openSections) => {
  openSections[section] = !openSections[section]
}

// Raw artifacts (contract.xml, notes.md, briefs) open in the system app through
// `grace_open_file`, which enforces .grace5 containment.
// Ownership: this component opens; `open-file` is emitted for observation only —
// Task 10 must NOT open again or every artifact would launch twice.
const { error: notifyError } = useNotification()

const openArtifact = async (changeRelativePath: string) => {
  const relPath = toGraceRelative(location.value, changeRelativePath)

  emit('open-file', changeRelativePath)

  if (!location.value || !relPath) {
    notifyError('Cannot open GRACE artifact', 'The change directory is not inside .grace5')
    return
  }

  try {
    await graceOpenFile(location.value.projectPath, relPath)
  } catch (e) {
    notifyError('Failed to open GRACE artifact', e instanceof Error ? e.message : 'Unknown error')
  }
}

// In-app log dialog
const logDialogOpen = ref(false)
const logTitle = ref('')
const logLoading = ref(false)
const logError = ref<string | null>(null)
const logContent = ref('')
const logTruncated = ref(false)

// Token guard: a log read that settles after another log was opened (or after
// the selection changed) must not render into the dialog.
let logRequestToken = 0

const openLog = async (changeRelativeLog: string) => {
  const token = ++logRequestToken
  const relPath = toGraceRelative(location.value, changeRelativeLog)

  logTitle.value = changeRelativeLog
  logDialogOpen.value = true
  logLoading.value = true
  logError.value = null
  logContent.value = ''
  logTruncated.value = false

  if (!location.value || !relPath) {
    logError.value = 'Cannot resolve the .grace5 root for this change'
    logLoading.value = false
    return
  }

  try {
    const file = await graceReadFile(location.value.projectPath, relPath)
    if (token !== logRequestToken) return
    logContent.value = file.content
    logTruncated.value = file.truncated
  } catch (e) {
    if (token !== logRequestToken) return
    logError.value = e instanceof Error ? e.message : 'Failed to read log'
  } finally {
    if (token === logRequestToken) logLoading.value = false
  }
}

// A different change can never reuse the previous change's log state.
watch(() => props.detail?.summary.id, () => {
  logRequestToken++
  logDialogOpen.value = false
  logLoading.value = false
  logError.value = null
  logContent.value = ''
  logTruncated.value = false
})
</script>

<template>
  <div class="h-full overflow-auto">
    <div v-if="!detail" class="h-full flex items-center justify-center p-6 text-center">
      <div class="space-y-1">
        <p v-if="isLoading" class="text-xs text-muted-foreground">Loading GRACE change…</p>
        <template v-else-if="error">
          <p class="text-sm font-medium">Failed to load change</p>
          <p class="text-xs text-destructive">{{ error }}</p>
        </template>
        <p v-else class="text-xs text-muted-foreground">
          Select a change in the GRACE explorer to inspect its contract and records.
        </p>
      </div>
    </div>

    <template v-else-if="summary">
      <div class="p-4 border-b border-border space-y-3">
        <div class="flex items-start justify-between gap-3">
          <div class="space-y-1 min-w-0">
            <CopyableId :value="summary.id" />
            <span class="ml-2 text-[10px] text-muted-foreground">
              revision {{ summary.revision ?? '—' }}
            </span>
            <h2 class="text-sm font-medium truncate">{{ summary.title ?? 'Untitled change' }}</h2>
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <GraceStageBadge :stage="summary.stage" size="sm" />
            <GraceAuthorizationBadge :authorization="summary.authorization" size="sm" />
          </div>
        </div>

        <p
          v-if="summary.authorizationNote"
          class="text-[11px] text-amber-600 dark:text-amber-400"
        >
          {{ summary.authorizationNote }}
        </p>

        <GraceLifecycleStepper :stage="summary.stage" />

        <div
          v-if="summary.malformed || detail.defects.length > 0"
          class="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive space-y-1"
        >
          <div class="font-medium">Structural defects</div>
          <p v-if="summary.malformed">{{ summary.malformed }}</p>
          <ul v-if="detail.defects.length > 0" class="list-disc pl-4 space-y-0.5">
            <li v-for="defect in detail.defects" :key="defect">{{ defect }}</li>
          </ul>
        </div>

        <div
          v-if="pinWarning"
          class="flex items-start gap-2 rounded border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-600 dark:text-amber-400"
        >
          <svg
            class="w-3.5 h-3.5 mt-0.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
          <span>{{ pinWarning }}</span>
        </div>

        <p class="text-[10px] text-muted-foreground">
          Structural read-only view — grace5 check/status remains authoritative. Authorization
          matches the kernel for schema-valid records; structurally invalid records are flagged,
          not re-validated.
        </p>
      </div>

      <div class="p-4 space-y-3">
        <CollapsibleSection
          title="Contract"
          :is-open="openSections.contract"
          @toggle="toggleSection('contract')"
        >
          <div class="space-y-2">
            <div class="flex justify-end">
              <Button
                variant="outline"
                size="sm"
                class="h-6 px-1.5 text-[10px]"
                @click="openArtifact('contract.xml')"
              >
                Open contract.xml
              </Button>
            </div>
            <GraceContractView v-if="detail.contract" :contract="detail.contract" />
            <p v-else class="text-xs text-muted-foreground">Contract could not be parsed.</p>
          </div>
        </CollapsibleSection>

        <CollapsibleSection
          title="Checks"
          :is-open="openSections.checks"
          :count="detail.contract?.checks.length"
          @toggle="toggleSection('checks')"
        >
          <GraceChecksTable
            v-if="detail.contract && detail.contract.checks.length > 0"
            :checks="detail.contract.checks"
            :latest-results="detail.latestResults"
            @open-log="openLog"
          />
          <p v-else class="text-xs text-muted-foreground">No checks declared.</p>
        </CollapsibleSection>

        <CollapsibleSection
          title="Records"
          :is-open="openSections.records"
          :count="detail.approvals.length + detail.runs.length + detail.reviews.length"
          @toggle="toggleSection('records')"
        >
          <GraceRecordsTimeline
            :approvals="detail.approvals"
            :runs="detail.runs"
            :reviews="detail.reviews"
            :close="detail.close"
            :briefs="detail.briefs"
            @open-log="openLog"
            @open-file="openArtifact"
          />
        </CollapsibleSection>

        <CollapsibleSection
          title="Notes"
          :is-open="openSections.notes"
          @toggle="toggleSection('notes')"
        >
          <div class="space-y-2">
            <div v-if="detail.notesMd !== null" class="flex justify-end">
              <Button
                variant="outline"
                size="sm"
                class="h-6 px-1.5 text-[10px]"
                @click="openArtifact('notes.md')"
              >
                Open notes.md
              </Button>
            </div>
            <GraceNotesView :notes-md="detail.notesMd" :truncated="detail.notesTruncated" />
          </div>
        </CollapsibleSection>
      </div>
    </template>

    <Dialog v-model:open="logDialogOpen">
      <DialogContent class="max-h-[calc(100vh-2rem)] sm:max-w-3xl gap-3">
        <DialogHeader>
          <DialogTitle class="text-sm font-mono truncate">{{ logTitle }}</DialogTitle>
          <DialogDescription class="text-xs">Check log (read-only)</DialogDescription>
        </DialogHeader>

        <p v-if="logLoading" class="text-xs text-muted-foreground">Loading…</p>
        <p v-else-if="logError" class="text-xs text-destructive">{{ logError }}</p>
        <template v-else>
          <p v-if="logTruncated" class="text-xs text-amber-500">Log truncated at 1 MiB.</p>
          <ScrollArea class="max-h-[60vh] rounded border border-border">
            <pre class="p-3 text-[11px] font-mono whitespace-pre-wrap break-all">{{ logContent }}</pre>
          </ScrollArea>
        </template>
      </DialogContent>
    </Dialog>
  </div>
</template>

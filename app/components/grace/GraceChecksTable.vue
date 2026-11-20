<script setup lang="ts">
import type { GraceCheck, GraceRunResult } from '~/types/grace'
import { Badge } from '~/components/ui/badge'
import { Button } from '~/components/ui/button'
import { graceVerdictClass } from '~/components/grace/grace-display'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '~/components/ui/table'

const props = defineProps<{
  checks: GraceCheck[]
  latestResults: GraceRunResult[]
}>()

const emit = defineEmits<{
  'open-log': [log: string]
}>()

// The backend already reduced results to the highest-numbered run containing
// each check (kernel checkState rule); this map is just a lookup.
const resultsByCheck = computed(() => {
  const map = new Map<string, GraceRunResult>()
  for (const result of props.latestResults) map.set(result.check, result)
  return map
})

const resultFor = (checkId: string) => resultsByCheck.value.get(checkId) ?? null
</script>

<template>
  <div class="rounded border border-border overflow-auto">
    <Table>
      <TableHeader>
        <TableRow class="bg-secondary/30 hover:bg-secondary/30">
          <TableHead class="font-medium w-40">Check</TableHead>
          <TableHead class="font-medium">Definition</TableHead>
          <TableHead class="font-medium w-64">Latest</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="checks.length === 0" class="hover:bg-transparent">
          <TableCell :colspan="3" class="h-16 text-center text-xs text-muted-foreground">
            No checks declared
          </TableCell>
        </TableRow>

        <template v-for="check in checks" :key="check.id">
          <TableRow class="align-top">
            <TableCell class="text-xs">
              <div class="flex items-center gap-1.5">
                <span class="font-mono">{{ check.id }}</span>
                <Badge variant="outline" class="text-[10px] px-1 py-0">{{ check.kind }}</Badge>
                <Badge
                  v-if="!check.required"
                  variant="secondary"
                  class="text-[10px] px-1 py-0 text-muted-foreground"
                >
                  optional
                </Badge>
              </div>
              <div class="mt-1 text-[10px] text-muted-foreground space-y-0.5">
                <div v-if="check.environment">env: {{ check.environment }}</div>
                <div>cwd: <span class="font-mono">{{ check.cwd ?? '.' }}</span></div>
                <div v-if="check.budget !== null">budget: {{ check.budget }}s</div>
                <div v-if="check.origin">origin: {{ check.origin }}</div>
              </div>
            </TableCell>

            <TableCell class="text-xs">
              <pre
                v-if="check.command"
                class="font-mono text-[11px] whitespace-pre-wrap break-all"
              >{{ check.command }}</pre>
              <p v-else-if="check.procedure" class="whitespace-pre-line">{{ check.procedure }}</p>
              <p v-else class="text-muted-foreground">—</p>
              <div v-if="check.expects.length > 0" class="mt-1 space-y-0.5">
                <div v-for="(expect, index) in check.expects" :key="index" class="text-[10px] text-muted-foreground">
                  expect {{ expect.min }}<span v-if="expect.max !== null">–{{ expect.max }}</span>: {{ expect.text }}
                </div>
              </div>
              <div v-if="check.forbids.length > 0" class="mt-1 text-[10px] text-muted-foreground">
                forbids: {{ check.forbids.join(', ') }}
              </div>
            </TableCell>

            <TableCell class="text-xs">
              <template v-if="resultFor(check.id)">
                <div class="flex items-center gap-1.5">
                  <Badge :class="graceVerdictClass(resultFor(check.id)!.verdict)" variant="secondary" class="text-[10px] px-1 py-0">
                    {{ resultFor(check.id)!.verdict }}
                  </Badge>
                  <span v-if="resultFor(check.id)!.claim" class="text-[10px] text-muted-foreground">
                    {{ resultFor(check.id)!.claim }}
                  </span>
                </div>
                <div class="mt-1 text-[10px] text-muted-foreground space-y-0.5">
                  <div v-if="resultFor(check.id)!.exit !== null">
                    exit: {{ resultFor(check.id)!.exit }}
                  </div>
                  <div v-if="resultFor(check.id)!.elapsedMs !== null">
                    elapsed: {{ resultFor(check.id)!.elapsedMs }} ms
                  </div>
                  <div v-if="resultFor(check.id)!.environment">
                    env: {{ resultFor(check.id)!.environment }}
                  </div>
                  <div v-if="resultFor(check.id)!.reason">
                    reason: {{ resultFor(check.id)!.reason }}
                  </div>
                </div>
                <Button
                  v-if="resultFor(check.id)!.log"
                  variant="outline"
                  size="sm"
                  class="h-6 mt-1 px-1.5 text-[10px]"
                  @click="emit('open-log', resultFor(check.id)!.log!)"
                >
                  View log
                </Button>
              </template>
              <span v-else class="text-muted-foreground">No run recorded</span>
            </TableCell>
          </TableRow>

          <TableRow
            v-if="resultFor(check.id)?.trace || resultFor(check.id)?.observation"
            class="hover:bg-transparent"
          >
            <TableCell colspan="3" class="text-[10px] text-muted-foreground py-1.5">
              <div v-if="resultFor(check.id)?.trace" class="flex flex-wrap items-center gap-x-3 gap-y-0.5">
                <span class="uppercase tracking-wide">Trace</span>
                <span>
                  matched {{ resultFor(check.id)!.trace!.matched }}/{{ resultFor(check.id)!.trace!.expected }}
                </span>
                <span v-if="resultFor(check.id)!.trace!.origin">
                  origin: {{ resultFor(check.id)!.trace!.origin }}
                </span>
                <span v-if="resultFor(check.id)!.trace!.device">
                  device: {{ resultFor(check.id)!.trace!.device }}
                </span>
                <span v-if="resultFor(check.id)!.trace!.build">
                  build: {{ resultFor(check.id)!.trace!.build }}
                </span>
                <span v-if="resultFor(check.id)!.trace!.missing.length > 0" class="text-destructive">
                  missing: {{ resultFor(check.id)!.trace!.missing.join(', ') }}
                </span>
                <span v-if="resultFor(check.id)!.trace!.forbidden.length > 0" class="text-destructive">
                  forbidden: {{ resultFor(check.id)!.trace!.forbidden.join(', ') }}
                </span>
              </div>
              <div v-if="resultFor(check.id)?.observation" class="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5">
                <span class="uppercase tracking-wide">Observation</span>
                <span>at {{ resultFor(check.id)!.observation!.at }}</span>
                <span>build: {{ resultFor(check.id)!.observation!.build }}</span>
                <span>env: {{ resultFor(check.id)!.observation!.environment }}</span>
                <span v-if="resultFor(check.id)!.observation!.reference">
                  ref: {{ resultFor(check.id)!.observation!.reference }}
                </span>
                <span class="italic">{{ resultFor(check.id)!.observation!.note }}</span>
              </div>
            </TableCell>
          </TableRow>
        </template>
      </TableBody>
    </Table>
  </div>
</template>

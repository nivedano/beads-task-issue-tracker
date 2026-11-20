<script setup lang="ts">
import type { GraceChangeSummary } from '~/types/grace'
import type { ColumnConfig } from '~/types/issue'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '~/components/ui/table'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'
import GraceStageBadge from '~/components/grace/GraceStageBadge.vue'
import GraceAuthorizationBadge from '~/components/grace/GraceAuthorizationBadge.vue'

type GraceSortField = 'id' | 'title' | 'stage' | 'revision'

const props = defineProps<{
  changes: GraceChangeSummary[]
  columns: ColumnConfig[]
  selectedId?: string | null
  sortField: GraceSortField
  sortDirection: 'asc' | 'desc'
}>()

const emit = defineEmits<{
  select: [change: GraceChangeSummary]
  sort: [field: GraceSortField, direction: 'asc' | 'desc']
}>()

// Only the fields the helper can sort by are clickable, even if a stored
// column config claims otherwise.
const sortableFields: GraceSortField[] = ['id', 'title', 'stage', 'revision']

const visibleColumns = computed(() => props.columns.filter(col => col.visible))

const isSortable = (columnId: string) => sortableFields.includes(columnId as GraceSortField)

const toggleSort = (columnId: string) => {
  if (!isSortable(columnId)) return
  const field = columnId as GraceSortField
  if (props.sortField === field) {
    emit('sort', field, props.sortDirection === 'asc' ? 'desc' : 'asc')
  } else {
    emit('sort', field, 'asc')
  }
}
</script>

<template>
  <div class="h-full rounded border border-border overflow-auto outline-none">
    <Table>
      <TableHeader>
        <TableRow class="bg-secondary/30 hover:bg-secondary/30">
          <TableHead
            v-for="col in visibleColumns"
            :key="col.id"
            class="font-medium"
            :class="[
              { 'cursor-pointer select-none hover:bg-secondary/50': isSortable(col.id) },
              col.id === 'revision' ? 'w-20' : '',
            ]"
            @click="toggleSort(col.id)"
          >
            <div class="flex items-center gap-1">
              <span>{{ col.label }}</span>
              <template v-if="isSortable(col.id)">
                <svg
                  v-if="sortField === col.id"
                  class="w-3 h-3 text-primary"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <path v-if="sortDirection === 'asc'" d="M12 19V5M5 12l7-7 7 7" />
                  <path v-else d="M12 5v14M5 12l7 7 7-7" />
                </svg>
                <svg
                  v-else
                  class="w-3 h-3 text-muted-foreground/40"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <path d="M7 15l5 5 5-5M7 9l5-5 5 5" />
                </svg>
              </template>
            </div>
          </TableHead>
        </TableRow>
      </TableHeader>

      <TableBody>
        <TableRow v-if="visibleColumns.length === 0 || changes.length === 0" class="hover:bg-transparent">
          <TableCell
            :colspan="Math.max(visibleColumns.length, 1)"
            class="h-24 text-center text-muted-foreground"
          >
            {{ visibleColumns.length === 0 ? 'No columns selected' : 'No GRACE changes found' }}
          </TableCell>
        </TableRow>

        <template v-if="visibleColumns.length > 0">
          <TableRow
            v-for="change in changes"
            :key="change.id"
            class="cursor-pointer"
            :class="selectedId === change.id ? 'bg-accent/50 hover:bg-accent/70' : 'hover:bg-muted/50'"
            @click="emit('select', change)"
          >
          <TableCell
            v-for="col in visibleColumns"
            :key="col.id"
            class="text-xs"
          >
            <template v-if="col.id === 'id'">
              <div class="flex items-center gap-1.5">
                <Tooltip v-if="change.malformed">
                  <TooltipTrigger as-child>
                    <svg
                      class="w-3.5 h-3.5 shrink-0 text-destructive"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                    >
                      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                      <line x1="12" y1="9" x2="12" y2="13" />
                      <line x1="12" y1="17" x2="12.01" y2="17" />
                    </svg>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p class="text-xs max-w-xs">{{ change.malformed }}</p>
                  </TooltipContent>
                </Tooltip>
                <span class="font-mono whitespace-nowrap">{{ change.id }}</span>
              </div>
            </template>

            <template v-else-if="col.id === 'title'">
              <span class="block truncate max-w-[420px]">{{ change.title ?? '—' }}</span>
            </template>

            <template v-else-if="col.id === 'stage'">
              <GraceStageBadge :stage="change.stage" size="sm" />
            </template>

            <template v-else-if="col.id === 'authorization'">
              <GraceAuthorizationBadge :authorization="change.authorization" size="sm" />
            </template>

            <template v-else-if="col.id === 'checks'">
              <span class="tabular-nums">{{ change.checksPassed }}/{{ change.checksRequired }}</span>
            </template>

            <template v-else-if="col.id === 'review'">
              <span class="text-muted-foreground">
                {{ change.reviewStatus ? `${change.reviewStatus} (${change.reviewOpenFindings})` : '—' }}
              </span>
            </template>

            <template v-else-if="col.id === 'revision'">
              <span class="tabular-nums">{{ change.revision ?? '—' }}</span>
            </template>
          </TableCell>
          </TableRow>
        </template>
      </TableBody>
    </Table>
  </div>
</template>

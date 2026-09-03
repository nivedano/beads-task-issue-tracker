<script setup lang="ts">
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '~/components/ui/dialog'

const open = defineModel<boolean>('open', { default: false })
const { isMacOS } = usePlatform()

const commandKey = computed(() => isMacOS ? '⌘' : 'Ctrl')

const shortcuts = computed(() => [
  { action: 'Open this help', keys: ['F1'] },
  { action: 'Create a new issue', keys: [commandKey.value, 'N'] },
  { action: 'Delete selected issue (asks first)', keys: [commandKey.value, 'D'] },
  { action: 'Select a project folder', keys: [commandKey.value, 'Shift', 'O'] },
  { action: 'Open and focus dashboard', keys: [commandKey.value, '1'] },
  { action: 'Open and focus details', keys: [commandKey.value, '2'] },
  { action: 'Toggle dashboard panel', keys: [commandKey.value, 'Shift', '1'] },
  { action: 'Toggle details panel', keys: [commandKey.value, 'Shift', '2'] },
  { action: 'Focus issue search', keys: [commandKey.value, 'F'] },
  { action: 'Focus issue list', keys: ['Ctrl', 'Q'] },
  { action: 'Open settings', keys: [commandKey.value, ','] },
  { action: 'Toggle debug logs', keys: [commandKey.value, 'Shift', 'L'] },
  { action: 'Maximize or restore window', keys: ['Alt', 'Enter'] },
])

const navigation = [
  { action: 'Switch to a numbered project', keys: ['1', '2', '…'] },
  { action: 'Move through issue results', keys: ['↑', '↓'] },
  { action: 'Open the focused issue', keys: ['Enter'] },
  { action: 'Pin or unpin the focused issue', keys: ['Space'] },
  { action: 'Move from search to results', keys: ['Ctrl', '↓'] },
]
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-xl gap-5">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <svg class="h-5 w-5 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.1 9a3 3 0 1 1 5.83 1c0 2-3 2-3 4" />
            <path d="M12 18h.01" />
          </svg>
          Help &amp; keyboard shortcuts
        </DialogTitle>
        <DialogDescription>
          Keep your hands on the keyboard while reviewing and organizing issues.
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-5">
        <section aria-labelledby="global-shortcuts-heading">
          <h2 id="global-shortcuts-heading" class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Global shortcuts</h2>
          <div class="overflow-hidden rounded-md border border-border">
            <div v-for="shortcut in shortcuts" :key="shortcut.action" class="flex min-h-10 flex-col items-start justify-between gap-1.5 border-b border-border px-3 py-2 last:border-b-0 sm:flex-row sm:items-center sm:gap-4">
              <span class="text-sm text-foreground">{{ shortcut.action }}</span>
              <span class="flex shrink-0 self-end items-center gap-1 sm:self-auto" :aria-label="shortcut.keys.join(' plus ')">
                <kbd v-for="key in shortcut.keys" :key="key" class="min-w-6 rounded border border-border bg-muted px-1.5 py-0.5 text-center font-mono text-[11px] font-medium text-muted-foreground shadow-sm">{{ key }}</kbd>
              </span>
            </div>
          </div>
        </section>

        <section aria-labelledby="list-navigation-heading">
          <h2 id="list-navigation-heading" class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Issue list navigation</h2>
          <div class="grid gap-x-6 gap-y-2 sm:grid-cols-2">
            <div v-for="shortcut in navigation" :key="shortcut.action" class="flex items-center justify-between gap-3 text-sm">
              <span class="text-muted-foreground">{{ shortcut.action }}</span>
              <span class="flex shrink-0 items-center gap-1" :aria-label="shortcut.keys.join(' plus ')">
                <kbd v-for="key in shortcut.keys" :key="key" class="min-w-6 rounded border border-border bg-muted px-1.5 py-0.5 text-center font-mono text-[11px] font-medium text-muted-foreground">{{ key }}</kbd>
              </span>
            </div>
          </div>
        </section>

        <aside class="rounded-md bg-muted/60 px-3 py-2.5 text-xs leading-relaxed text-muted-foreground">
          Tip: click a KPI to filter the issue list, use the column menu to tailor the table, and pin important issues so they stay visible on the dashboard.
        </aside>
      </div>
    </DialogContent>
  </Dialog>
</template>

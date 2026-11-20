<script setup lang="ts">
import { renderMarkdown } from '~/utils/markdown'

const props = defineProps<{
  notesMd: string | null
  truncated?: boolean
}>()

const renderedNotes = computed(() => props.notesMd ? renderMarkdown(props.notesMd) : '')
</script>

<template>
  <div v-if="notesMd">
    <p v-if="truncated" class="text-xs text-amber-500 mb-1">Notes truncated at 1 MiB.</p>
    <div class="markdown-base text-xs" v-html="renderedNotes" />
  </div>
  <p v-else class="text-xs text-muted-foreground">No notes for this change.</p>
</template>

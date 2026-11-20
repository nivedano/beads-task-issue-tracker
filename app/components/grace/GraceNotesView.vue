<script setup lang="ts">
import { renderMarkdown } from '~/utils/markdown'

const props = defineProps<{
  notesMd: string | null
}>()

const renderedNotes = computed(() => props.notesMd ? renderMarkdown(props.notesMd) : '')
</script>

<template>
  <div v-if="notesMd">
    <div class="markdown-base text-xs" v-html="renderedNotes" />
  </div>
  <p v-else class="text-xs text-muted-foreground">No notes for this change.</p>
</template>

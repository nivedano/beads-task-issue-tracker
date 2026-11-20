<script setup lang="ts">
import type { GraceStage } from '~/types/grace'
import { Badge } from '~/components/ui/badge'

const props = defineProps<{
  stage: GraceStage
  size?: 'default' | 'sm'
}>()

const stageConfig: Record<GraceStage, { label: string; class: string }> = {
  prepared: { label: 'PREPARED', class: 'bg-secondary text-secondary-foreground' },
  approved: { label: 'APPROVED', class: 'bg-blue-600 text-white' },
  executing: { label: 'EXECUTING', class: 'bg-amber-500 text-black' },
  reviewed: { label: 'REVIEWED', class: 'bg-violet-600 text-white' },
  closed: { label: 'CLOSED', class: 'bg-green-600 text-white' },
}

const config = computed(() => stageConfig[props.stage] ?? stageConfig.prepared)
</script>

<template>
  <Badge :class="[config.class, size === 'sm' ? 'text-[10px] px-1.5 py-0' : '']" variant="secondary">
    {{ config.label }}
  </Badge>
</template>

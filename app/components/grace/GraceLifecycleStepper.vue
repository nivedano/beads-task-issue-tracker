<script setup lang="ts">
import type { GraceStage } from '~/types/grace'
import { graceStageLadder } from '~/utils/grace-helpers'

const props = defineProps<{
  stage: GraceStage
}>()

const nodeLabels: Record<GraceStage, string> = {
  prepared: 'Prepare',
  approved: 'Approve',
  executing: 'Execute',
  reviewed: 'Review',
  closed: 'Close',
}

const currentIndex = computed(() => graceStageLadder.indexOf(props.stage))
</script>

<template>
  <div class="flex items-start">
    <template v-for="(node, index) in graceStageLadder" :key="node">
      <div class="flex flex-col items-center gap-1 min-w-14">
        <div
          class="flex items-center justify-center w-6 h-6 rounded-full border text-[10px] font-medium"
          :class="index <= currentIndex
            ? 'bg-primary border-primary text-primary-foreground'
            : 'border-border text-muted-foreground'"
        >
          <svg
            v-if="index < currentIndex"
            class="w-3 h-3"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="3"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span v-else>{{ index + 1 }}</span>
        </div>
        <span
          class="text-[10px]"
          :class="index <= currentIndex ? 'text-foreground' : 'text-muted-foreground'"
        >
          {{ nodeLabels[node] }}
        </span>
      </div>
      <div
        v-if="index < graceStageLadder.length - 1"
        class="flex-1 h-px mt-3 mx-1"
        :class="index < currentIndex ? 'bg-primary' : 'bg-border'"
      />
    </template>
  </div>
</template>

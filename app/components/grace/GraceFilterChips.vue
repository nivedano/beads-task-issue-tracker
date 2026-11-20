<script setup lang="ts">
import type { GraceStage, GraceAuthorization } from '~/types/grace'
import { Button } from '~/components/ui/button'
import GraceStageBadge from '~/components/grace/GraceStageBadge.vue'
import GraceAuthorizationBadge from '~/components/grace/GraceAuthorizationBadge.vue'

type GraceScope = 'open' | 'closed' | 'all'

const props = defineProps<{
  stageFilters: GraceStage[]
  authorizationFilters: GraceAuthorization[]
  scope: GraceScope
}>()

defineEmits<{
  removeStage: [stage: GraceStage]
  removeAuthorization: [authorization: GraceAuthorization]
  resetScope: []
  clearAll: []
}>()

const hasFilters = computed(
  () =>
    props.stageFilters.length > 0 ||
    props.authorizationFilters.length > 0 ||
    props.scope !== 'all'
)
</script>

<template>
  <div v-if="hasFilters" class="flex flex-wrap items-center gap-1.5">
    <span class="text-[10px] text-muted-foreground uppercase tracking-wide">Filters:</span>

    <div
      v-if="scope !== 'all'"
      class="inline-flex items-center gap-1 cursor-pointer group"
      @click="$emit('resetScope')"
    >
      <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-slate-600 text-white text-[10px] font-medium">
        {{ scope === 'closed' ? 'Closed' : 'Open' }}
      </span>
      <svg class="w-2.5 h-2.5 text-muted-foreground group-hover:text-destructive" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </div>

    <div
      v-for="stage in stageFilters"
      :key="`stage-${stage}`"
      class="inline-flex items-center gap-1 cursor-pointer group"
      @click="$emit('removeStage', stage)"
    >
      <GraceStageBadge :stage="stage" size="sm" />
      <svg class="w-2.5 h-2.5 text-muted-foreground group-hover:text-destructive" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </div>

    <div
      v-for="authorization in authorizationFilters"
      :key="`authorization-${authorization}`"
      class="inline-flex items-center gap-1 cursor-pointer group"
      @click="$emit('removeAuthorization', authorization)"
    >
      <GraceAuthorizationBadge :authorization="authorization" size="sm" />
      <svg class="w-2.5 h-2.5 text-muted-foreground group-hover:text-destructive" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </div>

    <Button variant="ghost" size="sm" class="h-5 px-1.5 text-[10px]" @click="$emit('clearAll')">
      Clear
    </Button>
  </div>
</template>

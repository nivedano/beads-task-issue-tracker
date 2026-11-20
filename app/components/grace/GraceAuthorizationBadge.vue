<script setup lang="ts">
import type { GraceAuthorization } from '~/types/grace'
import { Badge } from '~/components/ui/badge'

const props = defineProps<{
  authorization: GraceAuthorization
  size?: 'default' | 'sm'
}>()

const authorizationConfig: Record<GraceAuthorization, { label: string; class: string }> = {
  approved: { label: 'APPROVED', class: 'bg-green-600 text-white' },
  rejected: { label: 'REJECTED', class: 'bg-destructive text-destructive-foreground' },
  'altered after approval': { label: 'ALTERED AFTER APPROVAL', class: 'bg-amber-500 text-white' },
  unapproved: { label: 'UNAPPROVED', class: 'bg-muted text-muted-foreground' },
}

const config = computed(() => authorizationConfig[props.authorization] ?? authorizationConfig.unapproved)
</script>

<template>
  <Badge :class="[config.class, size === 'sm' ? 'text-[10px] px-1.5 py-0' : '']" variant="secondary">
    {{ config.label }}
  </Badge>
</template>

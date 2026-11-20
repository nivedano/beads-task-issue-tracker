<script setup lang="ts">
import type { GraceAuthorization } from '~/types/grace'
import { Badge } from '~/components/ui/badge'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'

const props = defineProps<{
  authorization: GraceAuthorization
  size?: 'default' | 'sm'
  /** Reason behind the value; shown as a tooltip when present. */
  note?: string | null
}>()

const authorizationConfig: Record<GraceAuthorization, { label: string; class: string }> = {
  approved: { label: 'APPROVED', class: 'bg-green-600 text-white' },
  rejected: { label: 'REJECTED', class: 'bg-destructive text-destructive-foreground' },
  'altered after approval': { label: 'ALTERED AFTER APPROVAL', class: 'bg-amber-500 text-black' },
  unapproved: { label: 'UNAPPROVED', class: 'bg-muted text-muted-foreground' },
}

const config = computed(() => authorizationConfig[props.authorization] ?? authorizationConfig.unapproved)
</script>

<template>
  <Tooltip v-if="note">
    <TooltipTrigger as-child>
      <Badge
        :class="[config.class, size === 'sm' ? 'text-[10px] px-1.5 py-0' : '', 'cursor-help']"
        variant="secondary"
      >
        {{ config.label }}
      </Badge>
    </TooltipTrigger>
    <TooltipContent side="top">
      <p class="text-xs max-w-xs">{{ note }}</p>
    </TooltipContent>
  </Tooltip>
  <Badge v-else :class="[config.class, size === 'sm' ? 'text-[10px] px-1.5 py-0' : '']" variant="secondary">
    {{ config.label }}
  </Badge>
</template>

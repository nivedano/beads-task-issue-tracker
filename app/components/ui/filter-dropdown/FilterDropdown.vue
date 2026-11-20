<script setup lang="ts">
import { Button } from '~/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from '~/components/ui/dropdown-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'
import { cn } from '~/lib/utils'

/**
 * Shared filter-dropdown shell for every toolbar facet control.
 *
 * Open state is CONTROLLED by the toolbar through `open`; the shell never
 * forwards Reka's `update:open` (its trigger click-toggle emission would fight
 * the toolbar's exclusive activeFilter group). Instead it requests changes
 * through two explicit events:
 * - `open`  — keyboard request to open the currently-closed menu
 * - `close` — Escape, radio selection, or keyboard request to close
 *
 * Modes:
 * - `checkbox`: items toggle with `@select.prevent`; the menu stays open.
 * - `radio`: selecting an item closes the menu (the item click, pointer or
 *   Enter/Space, bubbles to the content wrapper below).
 * - `panel`: arbitrary content (collapsibles, buttons); stays open until
 *   Escape, an outside click, or a consumer-requested close.
 */
type FilterDropdownMode = 'checkbox' | 'radio' | 'panel'

const props = withDefaults(defineProps<{
  open: boolean
  mode: FilterDropdownMode
  /** Visible trigger text; omit for icon-only triggers. */
  label?: string
  tooltip?: string
  /** Count pill; hidden when zero/absent. */
  count?: number
  /** Icon-only trigger with an overlaid count badge (Exclusion filters). */
  iconOnly?: boolean
  /** Screen-reader text for icon-only triggers. */
  srLabel?: string
  align?: 'start' | 'center' | 'end'
  /** Width/utility classes for the menu content. */
  contentClass?: string
}>(), {
  align: 'start',
})

const emit = defineEmits<{
  open: []
  close: []
}>()

// Keyboard parity with pointer opening. ArrowDown on an OPEN menu must not
// close it — that is Reka's standard move-focus-into-items key — so only
// Enter/Space toggle a close.
const handleTriggerKeydown = (key: 'enter' | 'space' | 'down') => {
  if (key === 'down' && props.open) return
  if (props.open) emit('close')
  else emit('open')
}

const handleContentClick = () => {
  if (props.mode === 'radio') emit('close')
}

const triggerClass = computed(() => props.iconOnly
  ? 'h-8 w-8 relative'
  : 'h-8 text-xs gap-1')
</script>

<template>
  <div
    @keydown.enter.prevent="handleTriggerKeydown('enter')"
    @keydown.space.prevent="handleTriggerKeydown('space')"
    @keydown.down.prevent="handleTriggerKeydown('down')"
  >
    <Tooltip>
      <DropdownMenu :open="open" :modal="false">
        <TooltipTrigger as-child>
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              :size="iconOnly ? 'icon' : 'sm'"
              :class="cn(triggerClass)"
            >
              <slot name="icon" />
              <template v-if="!iconOnly">
                {{ label }}
                <span
                  v-if="(count ?? 0) > 0"
                  class="ml-0.5 rounded-full bg-primary text-primary-foreground text-[10px] px-1.5 min-w-[18px] text-center"
                >
                  {{ count }}
                </span>
              </template>
              <template v-else>
                <span
                  v-if="(count ?? 0) > 0"
                  class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary text-[10px] font-medium text-primary-foreground px-1"
                >
                  {{ count }}
                </span>
                <span class="sr-only">{{ srLabel }}</span>
              </template>
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{{ tooltip }}</TooltipContent>
        <DropdownMenuContent
          :align="align"
          :class="contentClass"
          @escape-key-down="emit('close')"
        >
          <!-- Radio selection is a close request regardless of input method:
               Reka selects the item from both pointer clicks and Enter/Space,
               and the resulting click bubbles here. -->
          <div @click="handleContentClick">
            <slot :close="() => emit('close')" />
          </div>
        </DropdownMenuContent>
      </DropdownMenu>
    </Tooltip>
  </div>
</template>

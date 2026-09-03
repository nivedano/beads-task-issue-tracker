<script setup lang="ts">
  import { TooltipProvider } from '~/components/ui/tooltip'
  import { NotificationToast } from '~/components/ui/notification-toast'
  import { matchesShortcut, SHORTCUT_HELP, SHORTCUT_TOGGLE_MAXIMIZE } from '~/utils/shortcuts'

  const { isDark } = useTheme()
  const { showUpdateDialog, showAboutDialog, showSettingsDialog, showHelpDialog, openHelp, initializeMenu } = useAppMenu()
  const { toggleMaximizeWindow } = useTauriWindow()

  const handleAppShortcut = (event: KeyboardEvent) => {
    if (matchesShortcut(event, SHORTCUT_HELP)) {
      event.preventDefault()
      openHelp()
      return
    }

    if (matchesShortcut(event, SHORTCUT_TOGGLE_MAXIMIZE)) {
      event.preventDefault()
      toggleMaximizeWindow()
    }
  }

  useHead({
    title: 'Beads Task-Issue Tracker',
    meta: [
      { name: 'description', content: 'Beads Task / Issue Tracking Manager' },
      { name: 'theme-color', content: () => isDark.value ? '#1e1e1e' : '#ffffff' },
    ],
    htmlAttrs: {
      lang: 'en',
      class: () => isDark.value ? 'dark' : '',
    },
  })

  onMounted(() => {
    initializeMenu()
    window.addEventListener('keydown', handleAppShortcut)
  })

  onBeforeUnmount(() => window.removeEventListener('keydown', handleAppShortcut))
</script>

<template>
  <TooltipProvider>
    <NuxtPage />
    <LayoutUpdateDialog v-model:open="showUpdateDialog" />
    <LayoutAboutDialog v-model:open="showAboutDialog" />
    <LayoutSettingsDialog v-model:open="showSettingsDialog" />
    <LayoutHelpDialog v-model:open="showHelpDialog" />
    <NotificationToast />
  </TooltipProvider>
</template>

<script setup lang="ts">
/**
 * In-app menu bar for platforms without a native application menu (Windows,
 * Linux). It carries the same commands as the macOS menu, with the app submenu
 * renamed to "File", and lives inside the title bar row of AppHeader.
 */
import {
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarPortal,
  MenubarRoot,
  MenubarSeparator,
  MenubarTrigger,
} from 'reka-ui'
import { computePaste, selectedText } from '~/utils/edit-commands'
import { logFrontend } from '~/utils/bd-api'

const { openAbout, openSettings, openUpdate, toggleLogs } = useAppMenu()
const { minimizeWindow, toggleMaximizeWindow, closeWindow } = useTauriWindow()

type EditableElement = HTMLInputElement | HTMLTextAreaElement | HTMLElement

// Opening a menu steals focus from the field being edited, so remember the last
// editable element and restore focus to it before running an Edit command.
let lastEditable: EditableElement | null = null

const isTextField = (el: unknown): el is HTMLInputElement | HTMLTextAreaElement =>
  el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement

const isEditable = (el: unknown): el is EditableElement =>
  isTextField(el) || (el instanceof HTMLElement && el.isContentEditable)

const handleFocusIn = (event: FocusEvent) => {
  if (isEditable(event.target)) lastEditable = event.target
}

onMounted(() => document.addEventListener('focusin', handleFocusIn))
onBeforeUnmount(() => document.removeEventListener('focusin', handleFocusIn))

// Reka closes the menu and returns focus to the trigger; wait for that to settle
// before handing focus back to the field.
const withField = (run: (field: EditableElement | null) => void) => {
  const field = lastEditable
  window.setTimeout(() => {
    const target = field && document.contains(field) ? field : null
    target?.focus()
    run(target)
  }, 60)
}

const applyValue = (field: HTMLInputElement | HTMLTextAreaElement, value: string, caret: number) => {
  field.value = value
  field.setSelectionRange(caret, caret)
  // Keep v-model bindings in sync.
  field.dispatchEvent(new Event('input', { bubbles: true }))
}

const writeClipboard = (text: string) => {
  navigator.clipboard.writeText(text).catch((error) => {
    logFrontend('warn', `[menu] clipboard write denied: ${error}`)
  })
}

const readSelection = (field: HTMLInputElement | HTMLTextAreaElement) => ({
  value: field.value,
  selectionStart: field.selectionStart,
  selectionEnd: field.selectionEnd,
})

const undo = () => withField(() => document.execCommand('undo'))
const redo = () => withField(() => document.execCommand('redo'))

const copy = () => withField((field) => {
  if (isTextField(field)) {
    const text = selectedText(readSelection(field))
    if (text) writeClipboard(text)
    return
  }
  const selection = window.getSelection()?.toString()
  if (selection) writeClipboard(selection)
})

const cut = () => withField((field) => {
  if (!isTextField(field)) {
    document.execCommand('cut')
    return
  }
  const selection = readSelection(field)
  const text = selectedText(selection)
  if (!text) return
  writeClipboard(text)
  const { value, caret } = computePaste(selection, '')
  applyValue(field, value, caret)
})

// execCommand('paste') is blocked in WebView2, so paste goes through the
// clipboard API and writes the value back into the field.
const paste = () => withField(async (field) => {
  let text = ''
  try {
    text = await navigator.clipboard.readText()
  } catch (error) {
    logFrontend('warn', `[menu] clipboard read denied: ${error}`)
    return
  }
  if (!text) return

  if (isTextField(field)) {
    const { value, caret } = computePaste(readSelection(field), text)
    applyValue(field, value, caret)
    return
  }
  if (field?.isContentEditable) document.execCommand('insertText', false, text)
})

const selectAll = () => withField((field) => {
  if (isTextField(field)) {
    field.select()
    return
  }
  document.execCommand('selectAll')
})

const triggerClass = 'app-no-drag select-none rounded px-2 py-1 text-sm text-foreground/80 outline-none hover:bg-accent hover:text-accent-foreground data-[state=open]:bg-accent data-[state=open]:text-accent-foreground'
const contentClass = 'z-50 min-w-[13rem] overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-md'
const itemClass = 'flex cursor-default select-none items-center justify-between gap-6 rounded-sm px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground'
const shortcutClass = 'text-xs tracking-widest text-muted-foreground'
const separatorClass = '-mx-1 my-1 h-px bg-border'
</script>

<template>
  <MenubarRoot class="app-no-drag flex items-center gap-0.5">
    <MenubarMenu value="file">
      <MenubarTrigger :class="triggerClass">File</MenubarTrigger>
      <MenubarPortal>
        <MenubarContent :class="contentClass" align="start" :side-offset="6">
          <MenubarItem :class="itemClass" @select="openAbout">
            About Beads Task-Issue Tracker
          </MenubarItem>
          <MenubarSeparator :class="separatorClass" />
          <MenubarItem :class="itemClass" @select="openSettings">
            Settings...
            <span :class="shortcutClass">Ctrl+,</span>
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="openUpdate">
            Check for Update...
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="toggleLogs">
            Show Logs...
            <span :class="shortcutClass">Ctrl+Shift+L</span>
          </MenubarItem>
          <MenubarSeparator :class="separatorClass" />
          <MenubarItem :class="itemClass" @select="closeWindow">
            Quit
            <span :class="shortcutClass">Alt+F4</span>
          </MenubarItem>
        </MenubarContent>
      </MenubarPortal>
    </MenubarMenu>

    <MenubarMenu value="edit">
      <MenubarTrigger :class="triggerClass">Edit</MenubarTrigger>
      <MenubarPortal>
        <MenubarContent :class="contentClass" align="start" :side-offset="6">
          <MenubarItem :class="itemClass" @select="undo">
            Undo
            <span :class="shortcutClass">Ctrl+Z</span>
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="redo">
            Redo
            <span :class="shortcutClass">Ctrl+Y</span>
          </MenubarItem>
          <MenubarSeparator :class="separatorClass" />
          <MenubarItem :class="itemClass" @select="cut">
            Cut
            <span :class="shortcutClass">Ctrl+X</span>
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="copy">
            Copy
            <span :class="shortcutClass">Ctrl+C</span>
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="paste">
            Paste
            <span :class="shortcutClass">Ctrl+V</span>
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="selectAll">
            Select All
            <span :class="shortcutClass">Ctrl+A</span>
          </MenubarItem>
        </MenubarContent>
      </MenubarPortal>
    </MenubarMenu>

    <MenubarMenu value="window">
      <MenubarTrigger :class="triggerClass">Window</MenubarTrigger>
      <MenubarPortal>
        <MenubarContent :class="contentClass" align="start" :side-offset="6">
          <MenubarItem :class="itemClass" @select="minimizeWindow">
            Minimize
          </MenubarItem>
          <MenubarItem :class="itemClass" @select="toggleMaximizeWindow">
            Maximize
            <span :class="shortcutClass">Alt+Enter</span>
          </MenubarItem>
          <MenubarSeparator :class="separatorClass" />
          <MenubarItem :class="itemClass" @select="closeWindow">
            Close Window
          </MenubarItem>
        </MenubarContent>
      </MenubarPortal>
    </MenubarMenu>
  </MenubarRoot>
</template>

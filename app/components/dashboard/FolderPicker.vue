<script setup lang="ts">
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { ScrollArea } from '~/components/ui/scroll-area'
import { Badge } from '~/components/ui/badge'
import { Separator } from '~/components/ui/separator'
import { fsList, fsRoots, isTauri, type DirectoryEntry } from '~/utils/bd-api'
import { getParentPath, getFolderName, getPathCrumbs, getPathSeparator, isRootPath } from '~/utils/path'

const props = defineProps<{
  open: boolean
  currentPath: string
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  select: [path: string]
}>()

const { projects, addProject, isProject } = useProjects()

// 1344px = 1.5x the previous max-w-4xl (896px); size persists across sessions.
const { style: dialogStyle, startResize, resetSize } = useDialogResize({
  storageKey: 'beads:folderPicker',
  defaultWidth: 1344,
  defaultHeight: 640,
  minWidth: 720,
  minHeight: 440,
})

const PATH_INPUT_ID = 'folder-picker-path'

const currentPath = ref(props.currentPath || '~')
const pathInput = ref('')
const isEditingPath = ref(false)
const entries = ref<DirectoryEntry[]>([])
const roots = ref<DirectoryEntry[]>([])
const hasBeads = ref(false)
const usesDolt = ref(false)
const isLoading = ref(false)
const error = ref<string | null>(null)
const crumbBar = ref<HTMLElement | null>(null)

// Native OS folder dialog is desktop-only; resolved after mount so SSR stays stable.
const canBrowseNative = ref(false)
onMounted(() => {
  canBrowseNative.value = isTauri()
})

// Watch for dialog open to load initial path
watch(() => props.open, (isOpen) => {
  if (isOpen) {
    // Use home if currentPath is empty or "."
    const initialPath = props.currentPath && props.currentPath !== '.' ? props.currentPath : '~'
    currentPath.value = initialPath
    isEditingPath.value = false
    loadDirectory(initialPath)
    loadRoots()
  }
})

// Watch currentPath to update input (immediate so pathInput is set on mount)
watch(currentPath, (path) => {
  pathInput.value = path
  // Keep the tail of long paths visible — the part that identifies the folder.
  nextTick(() => {
    if (crumbBar.value) crumbBar.value.scrollLeft = crumbBar.value.scrollWidth
  })
}, { immediate: true })

// Watch prop changes so opening from a different favorite updates the path
watch(() => props.currentPath, (newPath) => {
  if (props.open && newPath && newPath !== '.') {
    currentPath.value = newPath
    loadDirectory(newPath)
  }
})

const loadRoots = async () => {
  if (roots.value.length > 0) return
  try {
    roots.value = await fsRoots()
  } catch {
    roots.value = []
  }
}

const loadDirectory = async (path: string) => {
  isLoading.value = true
  error.value = null

  try {
    const data = await fsList(path)
    currentPath.value = data.currentPath
    entries.value = data.entries
    hasBeads.value = data.hasBeads
    usesDolt.value = data.usesDolt
    isEditingPath.value = false
  } catch (e) {
    // Keep the current listing on screen: a mistyped path should not blank the
    // browser and strand the user with no way back.
    error.value = e instanceof Error ? e.message : 'Failed to load directory'
    pathInput.value = currentPath.value
  } finally {
    isLoading.value = false
  }
}

const navigateTo = (path: string) => {
  loadDirectory(path)
}

const navigateUp = () => {
  loadDirectory(getParentPath(currentPath.value))
}

const navigateHome = () => {
  loadDirectory('~')
}

const startEditingPath = async () => {
  pathInput.value = currentPath.value
  isEditingPath.value = true
  await nextTick()
  const el = document.getElementById(PATH_INPUT_ID) as HTMLInputElement | null
  el?.focus()
  el?.select()
}

const cancelEditingPath = () => {
  isEditingPath.value = false
  pathInput.value = currentPath.value
}

const handlePathInput = () => {
  if (pathInput.value.trim()) {
    loadDirectory(pathInput.value)
  }
}

/**
 * Open the OS folder dialog (Explorer on Windows, Finder on macOS). The result
 * is routed back through loadDirectory rather than emitted directly, so the
 * .beads detection and the Add Project gating still apply.
 */
const browseNative = async () => {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Beads project folder',
      defaultPath: currentPath.value && currentPath.value !== '~' ? currentPath.value : undefined,
    })
    if (typeof selected === 'string' && selected) {
      loadDirectory(selected)
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Could not open the system folder dialog'
  }
}

const handleSelect = () => {
  emit('select', currentPath.value)
  emit('update:open', false)
}

const handleCancel = () => {
  emit('update:open', false)
}

const handleAddProject = () => {
  addProject(currentPath.value)
  // Also select the folder and close the dialog for better UX
  emit('select', currentPath.value)
  emit('update:open', false)
}

// Get folder name from path
const currentFolderName = computed(() => {
  return getFolderName(currentPath.value) || '/'
})

// Follow the platform's separator rather than hardcoding a Windows example.
const pathPlaceholder = computed(() =>
  getPathSeparator(currentPath.value) === '\\' ? 'C:\\path\\to\\folder' : '/path/to/folder'
)

const crumbs = computed(() => getPathCrumbs(currentPath.value))
const isAtRoot = computed(() => isRootPath(currentPath.value))
const isCurrentProject = computed(() => isProject(currentPath.value))
</script>

<template>
  <Dialog :open="open" @update:open="$emit('update:open', $event)">
    <DialogContent class="flex flex-col overflow-hidden" :style="dialogStyle">
      <DialogHeader class="shrink-0">
        <DialogTitle>Select Beads Project Folder</DialogTitle>
        <DialogDescription>
          Navigate to a folder containing a Beads database (.beads folder)
        </DialogDescription>
      </DialogHeader>

      <div class="flex-1 min-h-0 flex flex-col gap-4 overflow-hidden">
        <!-- Navigation bar -->
        <div class="shrink-0 flex items-center gap-2">
          <Button variant="outline" size="icon" class="shrink-0" title="Home" @click="navigateHome">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
              <polyline points="9 22 9 12 15 12 15 22" />
            </svg>
          </Button>
          <Button
            variant="outline"
            size="icon"
            class="shrink-0"
            :disabled="isAtRoot"
            title="Parent folder"
            @click="navigateUp"
          >
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="18 15 12 9 6 15" />
            </svg>
          </Button>

          <!-- Path area: breadcrumbs, click to edit as text (Explorer behaviour) -->
          <div class="flex-1 min-w-0 flex items-center gap-2">
            <template v-if="isEditingPath">
              <Input
                :id="PATH_INPUT_ID"
                v-model="pathInput"
                class="flex-1 font-mono text-sm h-9"
                spellcheck="false"
                autocomplete="off"
                :placeholder="pathPlaceholder"
                @keyup.enter="handlePathInput"
                @keydown.esc.stop.prevent="cancelEditingPath"
              />
              <Button variant="outline" size="icon" class="shrink-0" title="Go" @click="handlePathInput">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="9 18 15 12 9 6" />
                </svg>
              </Button>
            </template>

            <div
              v-else
              ref="crumbBar"
              class="flex-1 min-w-0 h-9 flex items-center gap-0.5 overflow-x-auto whitespace-nowrap rounded-md border border-input bg-transparent px-1.5 scrollbar-none"
              @click.self="startEditingPath"
            >
              <template v-for="(crumb, index) in crumbs" :key="crumb.path">
                <svg
                  v-if="index > 0"
                  class="w-3 h-3 shrink-0 text-muted-foreground"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <polyline points="9 18 15 12 9 6" />
                </svg>
                <button
                  type="button"
                  class="shrink-0 rounded px-1.5 py-0.5 text-sm hover:bg-secondary transition-colors"
                  :class="index === crumbs.length - 1 ? 'font-medium' : 'text-muted-foreground'"
                  :title="crumb.path"
                  @click="navigateTo(crumb.path)"
                >
                  {{ crumb.name }}
                </button>
              </template>
              <!-- Remaining space stays clickable so the whole bar switches to text entry -->
              <span class="flex-1 min-w-[1rem] self-stretch cursor-text" @click="startEditingPath" />
            </div>

            <Button
              v-if="!isEditingPath"
              variant="outline"
              size="icon"
              class="shrink-0"
              title="Edit path as text"
              @click="startEditingPath"
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4z" />
              </svg>
            </Button>
          </div>

          <Button v-if="canBrowseNative" variant="secondary" class="shrink-0" @click="browseNative">
            <svg class="w-4 h-4 mr-2" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
            Browse…
          </Button>
        </div>

        <Separator />

        <!-- Sidebar + directory listing -->
        <div class="flex-1 min-h-0 flex border border-border rounded-md overflow-hidden">
          <!-- Quick access: known folders, drives, and existing projects -->
          <aside class="w-48 shrink-0 border-r border-border bg-secondary/20">
            <ScrollArea class="h-full">
              <div class="p-2 space-y-4">
                <div v-if="roots.length > 0">
                  <p class="px-2 pb-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Quick access
                  </p>
                  <button
                    v-for="root in roots"
                    :key="root.path"
                    type="button"
                    class="w-full flex items-center gap-2 rounded px-2 py-1.5 text-sm text-left hover:bg-secondary transition-colors"
                    :class="root.path === currentPath ? 'bg-secondary font-medium' : ''"
                    :title="root.path"
                    @click="navigateTo(root.path)"
                  >
                    <svg class="w-4 h-4 shrink-0 text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                    </svg>
                    <span class="truncate">{{ root.name }}</span>
                  </button>
                </div>

                <div v-if="projects.length > 0">
                  <p class="px-2 pb-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Projects
                  </p>
                  <button
                    v-for="project in projects"
                    :key="project.path"
                    type="button"
                    class="w-full flex items-center gap-2 rounded px-2 py-1.5 text-sm text-left hover:bg-secondary transition-colors"
                    :class="project.path === currentPath ? 'bg-secondary font-medium' : ''"
                    :title="project.path"
                    @click="navigateTo(project.path)"
                  >
                    <svg class="w-4 h-4 shrink-0 text-green-500" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M20 6h-8l-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2z" />
                    </svg>
                    <span class="truncate">{{ project.name }}</span>
                  </button>
                </div>
              </div>
            </ScrollArea>
          </aside>

          <div class="flex-1 min-w-0 min-h-0 flex flex-col">
            <!-- Current selection info -->
            <div class="flex items-center justify-between gap-4 px-3 py-2 border-b border-border">
              <div class="flex items-center gap-2 min-w-0">
                <svg
                  class="w-5 h-5 shrink-0"
                  :class="hasBeads ? 'text-green-500' : 'text-muted-foreground'"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M20 6h-8l-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2z" />
                </svg>
                <div class="min-w-0">
                  <p class="font-medium truncate leading-tight">
                    {{ currentFolderName }}
                  </p>
                  <p class="text-xs text-muted-foreground font-mono truncate leading-tight" :title="currentPath">
                    {{ currentPath }}
                  </p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <Badge v-if="hasBeads" class="bg-green-600 text-white shrink-0">
                  Beads Project
                </Badge>
                <Badge v-if="usesDolt" variant="outline" class="text-[#29E3C1] border-[#29E3C1]/50 shrink-0 px-2 py-1">
                  <svg style="width: 2rem; height: 0.65rem;" viewBox="0 0 163 56" fill="none">
                    <path d="M28.87 7.0459V45.8632C28.8654 46.7997 28.498 47.6965 27.8476 48.3591C27.1971 49.0217 26.316 49.3964 25.3957 49.402H10.4953C9.5713 49.402 8.68489 49.0298 8.0299 48.3666C7.3749 47.7035 7.00462 46.8034 7 45.8632V24.7722C7.00462 23.832 7.3749 22.9319 8.0299 22.2688C8.68489 21.6056 9.5713 21.2334 10.4953 21.2334H22.2115" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M156.3 49.4019H145.283" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M156.026 21.5259H134.174" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M145.336 7.0498V49.4024" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M72.2752 7.68311H59.049C56.6669 7.68311 54.7358 9.64808 54.7358 12.072V44.8074C54.7358 47.2313 56.6669 49.1963 59.049 49.1963H72.2752C74.6573 49.1963 76.5884 47.2313 76.5884 44.8074V12.072C76.5884 9.64808 74.6573 7.68311 72.2752 7.68311Z" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M119.586 49.4019H99.418" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M110.344 7.0498V49.4024" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M109.884 7H98.7939" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                </Badge>
                <Badge v-if="isCurrentProject" variant="outline" class="text-primary border-primary/50">
                  <svg class="w-3 h-3 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                  Added
                </Badge>
              </div>
            </div>

            <!-- Error message -->
            <div v-if="error" class="px-3 py-2 text-sm text-destructive bg-destructive/10">
              {{ error }}
            </div>

            <ScrollArea class="flex-1 min-h-0">
              <div v-if="isLoading" class="flex items-center justify-center py-12">
                <span class="text-muted-foreground">Loading...</span>
              </div>

              <div v-else-if="entries.length === 0" class="flex items-center justify-center py-12">
                <span class="text-muted-foreground">No subfolders</span>
              </div>

              <div v-else class="divide-y divide-border">
                <button
                  v-for="entry in entries"
                  :key="entry.path"
                  class="w-full flex items-center gap-3 px-4 py-3 hover:bg-secondary/50 transition-colors text-left"
                  @click="navigateTo(entry.path)"
                >
                  <svg
                    class="w-5 h-5 shrink-0"
                    :class="entry.hasBeads ? 'text-green-500' : 'text-muted-foreground'"
                    viewBox="0 0 24 24"
                    :fill="entry.hasBeads ? 'currentColor' : 'none'"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                  </svg>
                  <span class="flex-1 truncate">{{ entry.name }}</span>
                  <Badge v-if="entry.hasBeads" variant="outline" class="text-green-500 border-green-500/50 text-xs">
                    beads
                  </Badge>
                  <Badge v-if="entry.usesDolt" variant="outline" class="text-[#29E3C1] border-[#29E3C1]/50 text-xs px-1.5 py-0.5">
                    <svg style="width: 1.75rem; height: 0.55rem;" viewBox="0 0 163 56" fill="none">
                      <path d="M28.87 7.0459V45.8632C28.8654 46.7997 28.498 47.6965 27.8476 48.3591C27.1971 49.0217 26.316 49.3964 25.3957 49.402H10.4953C9.5713 49.402 8.68489 49.0298 8.0299 48.3666C7.3749 47.7035 7.00462 46.8034 7 45.8632V24.7722C7.00462 23.832 7.3749 22.9319 8.0299 22.2688C8.68489 21.6056 9.5713 21.2334 10.4953 21.2334H22.2115" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M156.3 49.4019H145.283" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M156.026 21.5259H134.174" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M145.336 7.0498V49.4024" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M72.2752 7.68311H59.049C56.6669 7.68311 54.7358 9.64808 54.7358 12.072V44.8074C54.7358 47.2313 56.6669 49.1963 59.049 49.1963H72.2752C74.6573 49.1963 76.5884 47.2313 76.5884 44.8074V12.072C76.5884 9.64808 74.6573 7.68311 72.2752 7.68311Z" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M119.586 49.4019H99.418" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M110.344 7.0498V49.4024" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M109.884 7H98.7939" stroke="currentColor" stroke-width="12.6599" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  </Badge>
                  <svg class="w-4 h-4 text-muted-foreground shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="9 18 15 12 9 6" />
                  </svg>
                </button>
              </div>
            </ScrollArea>
          </div>
        </div>
      </div>

      <DialogFooter class="mt-4 shrink-0">
        <Button variant="outline" @click="handleCancel">
          Cancel
        </Button>

        <!-- If already a project, show "Open" button -->
        <Button v-if="isCurrentProject" @click="handleSelect">
          <svg class="w-4 h-4 mr-2" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
          Open
        </Button>

        <!-- If not a project yet, show "Add Project" button (disabled if no .beads) -->
        <Button v-else :disabled="!hasBeads" @click="handleAddProject">
          <svg
            class="w-4 h-4 mr-2"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M12 5v14" />
            <path d="M5 12h14" />
          </svg>
          Add Project
        </Button>
      </DialogFooter>

      <!-- Resize handles. Double-click any of them to restore the default size. -->
      <div
        class="absolute top-3 bottom-6 right-0 w-1.5 cursor-ew-resize hover:bg-primary/40 transition-colors"
        title="Drag to resize (double-click to reset)"
        @mousedown="startResize($event, 'e')"
        @dblclick="resetSize"
      />
      <div
        class="absolute left-3 right-6 bottom-0 h-1.5 cursor-ns-resize hover:bg-primary/40 transition-colors"
        title="Drag to resize (double-click to reset)"
        @mousedown="startResize($event, 's')"
        @dblclick="resetSize"
      />
      <div
        class="absolute right-0 bottom-0 w-4 h-4 cursor-nwse-resize text-muted-foreground/60 hover:text-primary transition-colors"
        title="Drag to resize (double-click to reset)"
        @mousedown="startResize($event, 'se')"
        @dblclick="resetSize"
      >
        <svg class="w-4 h-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M15 6 6 15" />
          <path d="M15 11l-4 4" />
        </svg>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
/* The breadcrumb bar scrolls horizontally; a visible bar would eat its height. */
.scrollbar-none {
  scrollbar-width: none;
}
.scrollbar-none::-webkit-scrollbar {
  display: none;
}
</style>

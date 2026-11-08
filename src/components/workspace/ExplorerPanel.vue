<script setup lang="ts">
import {
  computed,
  nextTick,
  ref,
  shallowRef,
  useTemplateRef,
  watch,
  onMounted,
  onUnmounted,
} from 'vue'
import type {
  ContextMenuItem,
  DropdownMenuItem,
  TreeItem,
} from '@nuxt/ui'
import { useSortable } from '@vueuse/integrations/useSortable'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog'
import { join } from '@tauri-apps/api/path'
import { getFileIcon } from "../../file-icons"

interface DirectoryEntry {
  name: string
  path: string
  isDirectory: boolean
}

interface ExplorerItem extends TreeItem {
  path: string
  isDirectory: boolean
  isDraft?: boolean
  children?: ExplorerItem[]
}

interface MovedEntry {
  oldPath: string
  newPath: string
  isDirectory: boolean
}

const props = defineProps<{
  activePath: string | null
}>()

const emit = defineEmits<{
  'project-opened': [path: string]
  'open-file': [path: string]
  'entry-deleted': [path: string]
  'entry-moved': [entry: MovedEntry]
}>()

const tree = useTemplateRef<HTMLElement>('tree')
const explorerContent = useTemplateRef<HTMLElement>('explorerContent')
const draggingPath = ref<string | null>(null)
const dropDirectory = ref<string | null>(null)

const files = shallowRef<ExplorerItem[]>([])
const expanded = ref<string[]>([])
const projectPath = ref<string | null>(null)
const contextTarget = shallowRef<ExplorerItem | null>(null)

const errorMessage = ref('')
const value = ref('')
const creatingFile = ref(false)
const renamingPath = ref<string | null>(null)
const savingNewFile = ref(false)
const movingEntry = ref(false)

const selectedFile = computed(() =>
  props.activePath
    ? findItem(files.value, props.activePath)
    : undefined,
)

function toTreeItem(entry: DirectoryEntry): ExplorerItem {
  return {
    label: entry.name,
    path: entry.path,
    isDirectory: entry.isDirectory,
    icon: entry.isDirectory ? undefined : 'i-tabler-file',
  }
}

function sortEntries(items: ExplorerItem[]) {
  items.sort((a, b) =>
    Number(Boolean(b.isDraft)) - Number(Boolean(a.isDraft)) ||
    Number(b.isDirectory) - Number(a.isDirectory) ||
    String(a.label ?? '').localeCompare(String(b.label ?? ''), undefined, {
      sensitivity: 'base',
      numeric: true,
    }),
  )
  for (const item of items) {
    if (item.children) sortEntries(item.children)
  }
}

function refreshTree() {
  sortEntries(files.value)
  function cloneItems(items: ExplorerItem[]): ExplorerItem[] {
    return items.map(item => ({
      ...item,
      children: item.children
        ? cloneItems(item.children)
        : undefined,
    }))
  }

  files.value = cloneItems(files.value)
}

function findItem(
  items: ExplorerItem[],
  path: string,
): ExplorerItem | undefined {
  for (const item of items) {
    if (item.path === path) return item

    if (item.children) {
      const found = findItem(item.children, path)
      if (found) return found
    }
  }

  return undefined
}

function findParent(
  items: ExplorerItem[],
  path: string,
): ExplorerItem | undefined {
  for (const item of items) {
    if (item.children?.some(child => child.path === path)) {
      return item
    }

    if (item.children) {
      const found = findParent(item.children, path)
      if (found) return found
    }
  }

  return undefined
}

async function openProject() {
  if (creatingFile.value || movingEntry.value || draggingPath.value) return

  errorMessage.value = ''

  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Open project',
    })

    if (typeof selected !== 'string') return

    const entries = await invoke<DirectoryEntry[]>('read_directory', {
      path: selected,
    })

    files.value = entries.map(toTreeItem)
    refreshTree()
    projectPath.value = selected
    emit('project-opened', selected)
    expanded.value = []
    contextTarget.value = null
  } catch (error) {
    errorMessage.value = String(error)
    console.error('Could not open project:', error)
  }
}

async function openFromContext(data: string) {

  try {

    const entries = await invoke<DirectoryEntry[]>('read_directory', {
      path: data,
    })

    files.value = entries.map(toTreeItem)
    refreshTree()
    projectPath.value = data
    emit('project-opened', data)
    expanded.value = []
    contextTarget.value = null
  } catch (error) {
    errorMessage.value = String(error)
    console.error('Could not open project:', error)
  }
}

async function onItemSelect(_event: unknown, item: ExplorerItem,) {
  if (creatingFile.value || item.isDraft || movingEntry.value || draggingPath.value) return

  if (!item.isDirectory) {
    emit('open-file', item.path)
    return
  }

  const current = findItem(files.value, item.path)

  // Already loaded: let UTree handle expanding and collapsing.
  if (!current || current.children !== undefined) return

  try {
    const entries = await invoke<DirectoryEntry[]>('read_directory', {
      path: item.path,
    })

    const parent = findItem(files.value, item.path)
    if (!parent) return

    if (parent.children === undefined) {
      parent.children = entries.map(toTreeItem)
    }

    refreshTree()
    await nextTick()

    if (!expanded.value.includes(item.path)) {
      expanded.value = [...expanded.value, item.path]
    }
  } catch (error) {
    errorMessage.value = String(error)
  }
}

async function createEntry(isDirectory = false) {
  if (!projectPath.value || creatingFile.value || movingEntry.value) return

  const target = contextTarget.value
  const parentPath = target ? destinationFor(target) : projectPath.value
  if (!parentPath) return

  creatingFile.value = true
  errorMessage.value = ''

  try {
    let parent = findItem(files.value, parentPath)
    if (parent && parent.children === undefined) {
      const entries = await invoke<DirectoryEntry[]>('read_directory', { path: parentPath })
      parent = findItem(files.value, parentPath)
      if (!parent) return
      parent.children = entries.map(toTreeItem)
    }

    const draft: ExplorerItem = {
      label: isDirectory ? 'New folder' : 'New file',
      path: `draft:${crypto.randomUUID()}`,
      isDirectory,
      isDraft: true,
    }

    value.value = ''
    const siblings = parent?.children ?? files.value
    siblings.unshift(draft)
    refreshTree()
    await nextTick()

    if (parent && !expanded.value.includes(parent.path)) {
      expanded.value = [...expanded.value, parent.path]
    }
  } catch (error) {
    creatingFile.value = false
    errorMessage.value = String(error)
  }
}

function renameEntry() {
  const target = contextTarget.value
  if (!target || creatingFile.value || movingEntry.value) return

  renamingPath.value = target.path
  value.value = String(target.label ?? '')
  creatingFile.value = true
  errorMessage.value = ''
}

function cancelDraft(draft: ExplorerItem) {
  if (savingNewFile.value) return

  if (!renamingPath.value) {
    const parent = findParent(files.value, draft.path)
    const siblings = parent?.children ?? files.value
    const index = siblings.findIndex(child => child.path === draft.path)
    if (index !== -1) siblings.splice(index, 1)
    refreshTree()
  }

  renamingPath.value = null
  creatingFile.value = false
  value.value = ''
  errorMessage.value = ''
}

async function submitDraft(draft: ExplorerItem) {
  if (savingNewFile.value || !projectPath.value) return

  const parent = findParent(files.value, draft.path)
  const name = value.value.trim()

  if (!name || name === '.' || name === '..' || /[\\/]/.test(name)) {
    errorMessage.value = 'Enter a valid name without slashes.'
    return
  }

  if (renamingPath.value && name === draft.label) {
    cancelDraft(draft)
    return
  }

  savingNewFile.value = true
  errorMessage.value = ''

  try {
    const path = await join(parent?.path ?? projectPath.value, name)
    const oldPath = draft.path
    const renaming = Boolean(renamingPath.value)

    if (renaming) {
      await invoke('move_entry', { oldPath, newPath: path })
    } else {
      await invoke(draft.isDirectory ? 'create_directory' : 'create_file', { path })
    }

    const current = findItem(files.value, oldPath)
    if (current) {
      current.label = name
      current.isDraft = false
      updateEntryPaths(current, oldPath, path)
    }
    expanded.value = expanded.value.map(entry => replacePathPrefix(entry, oldPath, path))
    refreshTree()
    renamingPath.value = null
    creatingFile.value = false
    value.value = ''
    contextTarget.value = null

    if (renaming) {
      emit('entry-moved', { oldPath, newPath: path, isDirectory: draft.isDirectory })
    } else if (!draft.isDirectory) {
      emit('open-file', path)
    }
  } catch (error) {
    errorMessage.value = String(error)
  } finally {
    savingNewFile.value = false
  }
}

async function delete_filedir() {
  const target = contextTarget.value

  if (!target || creatingFile.value || movingEntry.value) return

  errorMessage.value = ''

  try {
    await invoke('delete_filedir', {
      path: target.path,
    })

    const parent = findParent(files.value, target.path)
    const siblings = parent?.children ?? files.value

    const index = siblings.findIndex(
      child => child.path === target.path,
    )

    if (index !== -1) {
      siblings.splice(index, 1)
    }

    expanded.value = expanded.value.filter(
      path =>
        path !== target.path &&
        !isInside(path, target.path),
    )

    contextTarget.value = null
    refreshTree()

    emit('entry-deleted', target.path)
  } catch (error) {
    errorMessage.value = String(error)
  }
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

function samePath(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b)
}

function isInside(path: string, directory: string): boolean {
  return normalizePath(path).startsWith(`${normalizePath(directory)}/`)
}

function replacePathPrefix(path: string, oldPath: string, newPath: string): string {
  if (samePath(path, oldPath)) return newPath
  if (!isInside(path, oldPath)) return path
  const suffix = normalizePath(path).slice(normalizePath(oldPath).length)
  const separator = newPath.includes('\\') ? '\\' : '/'
  return newPath + suffix.replace(/\//g, separator)
}

function updateEntryPaths(item: ExplorerItem, oldPath: string, newPath: string) {
  item.path = replacePathPrefix(item.path, oldPath, newPath)
  for (const child of item.children ?? []) {
    updateEntryPaths(child, oldPath, newPath)
  }
}

function entryInRow(row: HTMLElement): ExplorerItem | undefined {
  const entry = row.matches('[data-entry-path]')
    ? row
    : row.querySelector<HTMLElement>('[data-entry-path]')
  const path = entry?.dataset.entryPath
  return path ? findItem(files.value, path) : undefined
}

function destinationFor(item: ExplorerItem): string | null {
  if (item.isDraft) return null
  return item.isDirectory
    ? item.path
    : findParent(files.value, item.path)?.path ?? projectPath.value
}

function validDestination(path: string): boolean {
  const source = draggingPath.value
    ? findItem(files.value, draggingPath.value)
    : undefined
  if (!source || !projectPath.value) return false
  const currentDirectory = findParent(files.value, source.path)?.path ?? projectPath.value
  if (samePath(currentDirectory, path)) return false
  return !source.isDirectory || (
    !samePath(source.path, path) && !isInside(path, source.path)
  )
}

function setDestination(path: string | null) {
  dropDirectory.value = path && validDestination(path) ? path : null
}

async function moveEntry(oldPath: string, destination: string) {
  if (creatingFile.value || movingEntry.value) return
  const source = findItem(files.value, oldPath)
  if (!source || source.isDraft || !projectPath.value) return

  const currentDirectory = findParent(files.value, oldPath)?.path ?? projectPath.value
  if (samePath(currentDirectory, destination)) return
  if (source.isDirectory && (
    samePath(oldPath, destination) || isInside(destination, oldPath)
  )) return

  movingEntry.value = true
  errorMessage.value = ''

  try {
    const name = oldPath.split(/[\\/]/).pop()
    if (!name) return
    const newPath = await join(destination, name)
    const destinationIsRoot = samePath(destination, projectPath.value)
    let destinationItem = destinationIsRoot
      ? undefined
      : findItem(files.value, destination)
    if (!destinationIsRoot && !destinationItem?.isDirectory) return

    // Read unopened folders before moving; retain their existing entries.
    // Do not change the visible tree until the disk operation succeeds.
    const destinationChildren = destinationIsRoot
      ? files.value
      : destinationItem?.children ?? (
        await invoke<DirectoryEntry[]>('read_directory', { path: destination })
      ).map(toTreeItem)

    if (destinationChildren.some(item => samePath(item.path, newPath))) {
      throw new Error('An entry with that name already exists in the destination.')
    }

    // Rust must return Err if the destination exists, rather than overwrite it.
    await invoke<void>('move_entry', { oldPath, newPath })

    // Re-resolve nodes because refreshTree clones them.
    const sourceParent = findParent(files.value, oldPath)
    const sourceSiblings = sourceParent?.children ?? files.value
    const sourceIndex = sourceSiblings.findIndex(item => item.path === oldPath)
    if (sourceIndex === -1) throw new Error('Moved entry is no longer in the tree.')

    destinationItem = destinationIsRoot
      ? undefined
      : findItem(files.value, destination)
    if (!destinationIsRoot && !destinationItem) {
      throw new Error('Destination is no longer in the tree.')
    }

    const [moved] = sourceSiblings.splice(sourceIndex, 1)
    if (!moved) return
    updateEntryPaths(moved, oldPath, newPath)

    if (destinationIsRoot) {
      files.value.push(moved)
    } else if (destinationItem) {
      destinationItem.children ??= destinationChildren
      destinationItem.children.push(moved)
    }

    expanded.value = expanded.value.map(path =>
      replacePathPrefix(path, oldPath, newPath),
    )
    if (!destinationIsRoot && !expanded.value.includes(destination)) {
      expanded.value = [...expanded.value, destination]
    }

    contextTarget.value = null
    refreshTree()
    emit('entry-moved', { oldPath, newPath, isDirectory: moved.isDirectory })
  } catch (error) {
    errorMessage.value = String(error)
    console.error('Could not move entry:', error)
  } finally {
    movingEntry.value = false
  }
}

type PointerPosition = { clientX: number; clientY: number }
let originalRows: Node[] = []
let lastPointer: PointerPosition | null = null

function pointerPosition(event: Event): PointerPosition | null {
  if ('changedTouches' in event) {
    const touch = (event as TouchEvent).changedTouches[0]
    return touch ? { clientX: touch.clientX, clientY: touch.clientY } : null
  }
  if ('clientX' in event && 'clientY' in event) {
    return { clientX: Number(event.clientX), clientY: Number(event.clientY) }
  }
  return null
}

function updateDropTarget(point: PointerPosition) {
  const content = explorerContent.value
  if (!content) return
  const { clientX: x, clientY: y } = point
  const bounds = content.getBoundingClientRect()
  if (x < bounds.left || x > bounds.right || y < bounds.top || y > bounds.bottom) {
    setDestination(null)
    return
  }

  const hits = document.elementsFromPoint(x, y)

  // Ignore the floating preview and moving placeholder.
  const hit = hits.find(element =>
    content.contains(element) &&
    !element.closest(
      '.explorer-drag-preview, .explorer-drag-ghost',
    ),
  )

  if (!hit) return

  if (hit.closest('[data-project-root-drop]')) {
    setDestination(projectPath.value)
    return
  }

  const row = hit.closest<HTMLElement>('li[role="presentation"]')

  if (row) {
    const item = entryInRow(row)

    setDestination(item ? destinationFor(item) : null)
  }
}

function trackPointer(event: Event) {
  const point = pointerPosition(event)
  if (!point) return
  lastPointer = point
  updateDropTarget(point)
}

function stopTracking() {
  document.removeEventListener('pointermove', trackPointer, true)
  document.removeEventListener('mousemove', trackPointer, true)
  document.removeEventListener('touchmove', trackPointer, true)
}

const { option } = useSortable(tree, files, {
  animation: 180,
  easing: 'cubic-bezier(0.2, 0, 0, 1)',
  draggable: '> li',
  ghostClass: 'explorer-drag-ghost',
  chosenClass: 'explorer-drag-chosen',
  fallbackClass: 'explorer-drag-preview',
  forceFallback: true,
  fallbackOnBody: true,
  fallbackTolerance: 5,
  filter: 'input, textarea, [data-draft]',
  preventOnFilter: false,

  onStart(event) {
    originalRows = Array.from(event.from.childNodes)
    draggingPath.value = entryInRow(event.item)?.path ?? null
    errorMessage.value = ''
    lastPointer = null
    document.addEventListener('pointermove', trackPointer, true)
    document.addEventListener('mousemove', trackPointer, true)
    document.addEventListener('touchmove', trackPointer, true)
  },

  onMove(event, originalEvent) {
    const point = pointerPosition(originalEvent)

    if (point) {
      lastPointer = point
    }

    const hoveredItem = entryInRow(event.related)

    setDestination(
      hoveredItem ? destinationFor(hoveredItem) : null,
    )

    return true
  },

  // Suppress VueUse's flat-array reorder: this is a filesystem tree.
  onUpdate() { },

  onEnd(event) {
    stopTracking()
    const originalEvent = (event as typeof event & { originalEvent?: Event }).originalEvent
    const point = (originalEvent ? pointerPosition(originalEvent) : null) ?? lastPointer
    if (point) updateDropTarget(point)
    const oldPath = draggingPath.value
    const destination = dropDirectory.value

    // Undo Sortable's temporary DOM order before Vue renders the real move.
    for (const row of originalRows) event.from.appendChild(row)
    originalRows = []
    draggingPath.value = null
    dropDirectory.value = null
    lastPointer = null

    if (oldPath && destination) void moveEntry(oldPath, destination)
  },
})

watch([creatingFile, movingEntry], ([creating, moving]) => {
  option('disabled', creating || moving)
})

let unlistenContextMenu: UnlistenFn | null = null;

onMounted(async () => {
  // Listen for the path forwarded from the Windows registry execution
  unlistenContextMenu = await listen('context-menu-path', (event) => {
    const targetPath = event.payload; // This is the string path from Rust
    if (targetPath) {
      console.log(targetPath)
      openFromContext(targetPath as string);
    }
  });
});

onUnmounted(() => {
  if (unlistenContextMenu) unlistenContextMenu();
  stopTracking
});

const items: DropdownMenuItem[][] = [
  [
    {
      label: 'Open Project',
      onSelect: () => openProject(),
    },
  ],
]

const creationItems: ContextMenuItem[] = [
  {
    label: 'Create File',
    icon: 'i-tabler-plus',
    onSelect: () => createEntry(),
  },
  {
    label: 'Create Folder',
    icon: 'i-tabler-folder-plus',
    onSelect: () => createEntry(true),
  },
]

const contextItems: ContextMenuItem[] = [
  ...creationItems,
  {
    label: 'Rename',
    icon: 'i-tabler-pencil',
    onSelect: () => renameEntry(),
  },
  {
    label: 'Delete',
    icon: 'i-tabler-minus',
    onSelect: () => delete_filedir(),
  },
]
</script>

<template>
  <main class="explorer">
    <div class="titlebar">
      <UDropdownMenu :items="items" :content="{ align: 'start', side: 'right' }">
        <UButton color="neutral" variant="link" trailing-icon="i-tabler-menu-2" aria-label="Project menu" />
      </UDropdownMenu>

      <p v-if="errorMessage" class="explorer-error">
        {{ errorMessage }}
      </p>
    </div>

    <div ref="explorerContent" class="explorer-content">
      <UTree ref="tree" trailing-icon="none" v-model:expanded="expanded" :nested="false" :unmount-on-hide="true"
        :items="files" :model-value="selectedFile" :get-key="(item: ExplorerItem) => item.path" @select="onItemSelect">
        <template #item="{ item, expanded: isExpanded }">
          <div v-if="item.isDraft || renamingPath === item.path" class="explorer-entry" data-draft @click.stop @dblclick.stop @pointerdown.stop
            @keydown.stop>
            <UIcon :name="item.isDirectory ? 'i-tabler-folder' : 'i-tabler-file'" class="size-4 shrink-0" />

            <UInput v-model="value" autofocus size="xs" class="min-w-0 flex-1" :disabled="savingNewFile"
              @keydown.enter.prevent="submitDraft(item)" @keydown.esc.prevent="cancelDraft(item)" />
          </div>

          <UContextMenu v-else :items="contextItems">
            <div class="explorer-entry" :data-entry-path="item.path" :class="{
              'explorer-drop-target': item.isDirectory && dropDirectory === item.path,
            }" @contextmenu.capture="contextTarget = item">
              <img :src="getFileIcon(item.label ?? '', item.isDirectory, isExpanded)" alt="" class="size-4 shrink-0" />

              <span class="explorer-entry-label">
                {{ item.label }}
              </span>
            </div>
          </UContextMenu>
        </template>
      </UTree>

      <UContextMenu v-if="projectPath" :items="creationItems">
        <div data-project-root-drop class="explorer-root-drop" :class="{
          'explorer-root-drop-active': draggingPath && dropDirectory === projectPath,
        }" @contextmenu.capture="contextTarget = null">
          <span v-if="draggingPath">Move to project root</span>
        </div>
      </UContextMenu>
    </div>
  </main>
</template>

<style scoped>
.explorer {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  height: 32px;
  margin: 6px 3px 0;
  border-bottom: 1px solid var(--lux-glass-border);
}

.explorer-error {
  min-width: 0;
  margin: 0 8px;
  overflow: hidden;
  color: var(--ui-error);
  font-size: 11px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.explorer-content {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: rgb(129 146 179 / 40%) transparent;
}

.explorer-content::-webkit-scrollbar {
  width: 5px;
  height: 5px;
}

.explorer-content::-webkit-scrollbar-track,
.explorer-content::-webkit-scrollbar-corner {
  background: transparent;
}

.explorer-content::-webkit-scrollbar-thumb {
  background: rgb(129 146 179 / 40%);
  border-radius: 999px;
}

.explorer-content::-webkit-scrollbar-thumb:hover {
  background: rgb(105 165 255 / 65%);
}

.explorer-content::-webkit-scrollbar-button {
  display: none;
  width: 0;
  height: 0;
}

@supports selector(::-webkit-scrollbar) {
  .explorer-content {
    scrollbar-width: auto;
    scrollbar-color: auto;
  }
}

.explorer-entry {
  border-radius: 5px;
  transition: background-color 140ms ease, box-shadow 140ms ease;
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
}

.explorer-entry>.iconify {
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
}

.explorer-drag-preview .explorer-entry>.iconify {
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
}

.explorer-entry-label {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.explorer-content :deep([role="tree"]) {
  flex-shrink: 0;
}

.explorer-content :deep(.explorer-drag-ghost) {
  opacity: 0.25;
}

.explorer-content :deep(.explorer-drag-chosen) {
  cursor: grabbing;
}

.explorer-drop-target {
  background: rgb(105 165 255 / 20%);
  box-shadow:
    inset 0 0 0 1px rgb(129 165 255 / 65%),
    0 0 12px rgb(105 165 255 / 10%);
}

.explorer-root-drop {
  flex: 1;
  min-height: 52px;
  margin: 4px;
  padding: 12px 8px;
  border-radius: 7px;
  color: var(--ui-text-muted);
  font-size: 11px;
  transition: background-color 140ms ease, box-shadow 140ms ease;
}

.explorer-root-drop-active {
  background: rgb(105 165 255 / 9%);
  box-shadow: inset 0 0 0 1px rgb(129 165 255 / 35%);
}
</style>

<!-- Sortable appends the floating preview to body, outside scoped styles. -->
<style>
.explorer-drag-preview {
  pointer-events: none !important;
  opacity: 0.94 !important;
  overflow: hidden;
  color: var(--ui-text, #bac2ce);
  background: var(--lux-glass-bg, rgb(12 16 22 / 75%));
  backdrop-filter: blur(var(--lux-glass-blur, 18px));
  -webkit-backdrop-filter: blur(var(--lux-glass-blur, 18px));
  border: 1px solid var(--lux-glass-border, rgb(255 255 255 / 12%));
  border-radius: 7px;
  box-shadow: 0 8px 24px rgb(0 0 0 / 25%);
  cursor: grabbing;
}

.explorer-drag-preview .explorer-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.explorer-drag-preview .explorer-entry-label {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>

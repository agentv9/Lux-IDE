<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { IconLayoutColumns, IconDots } from '@tabler/icons-vue'
import { monaco, highlightingReady, getLanguage } from '../../monaco'
import { invoke } from '@tauri-apps/api/core'
import { emit as emitEvent } from '@tauri-apps/api/event'
import { connectTypeScript } from '../../typescript'

const props = defineProps<{
    path: string
    active: boolean
    projectPath: string | null
}>()

const emit = defineEmits<{
    activate: []
    'dirty-change': [isUnsaved: boolean]
}>()

interface FileEntry {
    name: string
    path: string
    content: string
}

const filename = computed(
    () => props.path.split(/[\\/]/).pop() || props.path,
)

const editorContainer = ref<HTMLDivElement | null>(null)
const isUnsaved = ref(false)

let editor: monaco.editor.IStandaloneCodeEditor | undefined
let model: monaco.editor.ITextModel | undefined
let changeListener: monaco.IDisposable | undefined
let savedVersion: number | undefined
let unmounted = false
let languageService: monaco.IDisposable | undefined
const languageError = ref('')

async function connectLanguageService() {
    if (!model || unmounted) return
    languageService?.dispose()
    languageService = undefined
    languageError.value = ''
    try {
        languageService = await connectTypeScript(model, props.projectPath, message => { languageError.value = message })
        if (unmounted) languageService?.dispose()
    } catch (error) {
        languageError.value = String(error)
        console.error('Could not start TypeScript:', error)
    }
}

watch(isUnsaved, value => {
    emit('dirty-change', value)
})

watch(
    () => props.active,
    active => {
        if (active) {
            void emitEvent('editor-active-file-changed', {
                path: props.path,
                language: getLanguage(props.path),
            }).catch(console.error)
        }
    },
    { immediate: true },
)

function updateUnsavedState() {
    isUnsaved.value =
        model !== undefined &&
        savedVersion !== undefined &&
        model.getAlternativeVersionId() !== savedVersion
}

// Capture this version along with the text BEFORE starting a save.
function getSaveSnapshot() {
    if (!model) return undefined

    return {
        path: props.path,
        content: model.getValue(),
        version: model.getAlternativeVersionId(),
    }
}

// Call only after Rust successfully writes the snapshot.
function markSaved(version: number) {
    savedVersion = version
    updateUnsavedState()
}

defineExpose({
    getSaveSnapshot,
    markSaved,
})

async function saveFile() {
    const snapshot = getSaveSnapshot()
    if (!snapshot) return

    try {
        await invoke('write_file', {
            path: snapshot.path,
            content: snapshot.content,
        })

        markSaved(snapshot.version)
    } catch (error) {
        console.error('Failed to save file:', error)
    }
}

onMounted(async () => {
    await highlightingReady

    if (!editorContainer.value) return

    try {
        const data = await invoke<FileEntry>('read_file', {
            path: props.path,
        })

        // The tab might have closed while the file was loading.
        if (unmounted || !editorContainer.value) return

        model = monaco.editor.createModel(
            data.content,
            getLanguage(props.path),
            monaco.Uri.file(props.path),
        )

        console.log('Model language:', model.getLanguageId())
        console.log('Shiki ready')
        savedVersion = model.getAlternativeVersionId()

        changeListener = model.onDidChangeContent(updateUnsavedState)

        editor = monaco.editor.create(editorContainer.value, {
            model,
            theme: 'vitesse-dark',
            automaticLayout: true,
            fontSize: 14,
            fontFamily: 'Consolas, "Courier New", monospace',
            minimap: {
                enabled: false,
            },
            scrollBeyondLastLine: false,
            padding: {
                top: 12,
                bottom: 12,
            },
        })

        editor.addCommand(
            monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
            () => {
                void saveFile()
            },
        )

        editor.onDidFocusEditorText(() => emit('activate'))
        await connectLanguageService()
    } catch (error) {
        console.error('Failed to initialize editor:', error)
    }
})

onBeforeUnmount(() => {
    unmounted = true
    changeListener?.dispose()
    languageService?.dispose()
    editor?.dispose()
    model?.dispose()
})
</script>

<template>
    <section class="editor-panel" @pointerdown="emit('activate')">
        <header class="editor-toolbar">
            <div class="editor-filename" :title="props.path">
                {{ filename }}
                <span v-if="isUnsaved" class="unsaved-dot" role="img" aria-label="Unsaved changes"
                    title="Unsaved changes" />
            </div>

            <div class="editor-actions">
                <button type="button" class="editor-action" aria-label="Split editor" title="Split editor">
                    <IconLayoutColumns :size="16" />
                </button>

                <button type="button" class="editor-action" aria-label="Editor options" title="Editor options">
                    <IconDots :size="16" />
                </button>
            </div>
        </header>

        <div class="editor-body">
            <div ref="editorContainer" class="monaco-container" />
        </div>
        <div v-if="languageError" class="language-error" :title="languageError">
            {{ languageError }}
            <button type="button" @click="connectLanguageService">Retry</button>
        </div>
    </section>
</template>

<style scoped>
.language-error {
    padding: 4px 12px;
    color: var(--ui-text-muted);
    font-size: 12px;
    max-height: 100px;
    overflow: auto;
    white-space: pre-wrap;
}

.editor-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;

    color: var(--ui-text);
    background: transparent;
}

.unsaved-dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-left: 8px;
    border-radius: 50%;
    background: var(--ui-primary);
    vertical-align: middle;
}

.editor-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    height: 32px;
    gap: 12px;
    padding: 0 8px 0 12px;

    border-bottom: 1px solid var(--lux-glass-border);
}

.editor-filename {
    min-width: 0;
    overflow: hidden;
    color: var(--ui-text-muted);
    font: 12px / 1.4 system-ui, sans-serif;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.editor-actions {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    gap: 2px;
}

.editor-action {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;

    border: 0;
    border-radius: 6px;
    color: var(--ui-text-muted);
    background: transparent;
    cursor: pointer;

    transition:
        background-color 120ms ease,
        color 120ms ease;
}

.editor-action:hover {
    color: var(--ui-text-highlighted);
    background: var(--lux-hover);
}

.editor-action:focus-visible {
    outline: 1px solid var(--ui-primary);
    outline-offset: -2px;
}

.editor-body {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
}

.monaco-container {
    position: absolute;
    inset: 0;
}
</style>

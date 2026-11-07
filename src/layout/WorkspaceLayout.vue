<script setup lang="ts">
import {
    onMounted,
    onBeforeUnmount,
    ref,
    shallowRef,
    type Component,
} from "vue";
import {
    GoldenLayout,
    ComponentItem,
    RowOrColumn,
    Stack,
    type ContentItem,
    type LayoutConfig,
} from "golden-layout";
import "golden-layout/dist/css/goldenlayout-base.css";
import ExplorerPanel from "../components/workspace/ExplorerPanel.vue";
import EditorPanel from "../components/workspace/EditorPanel.vue";
import TerminalPanel from "../components/workspace/TerminalPanel.vue";
import { emit } from "@tauri-apps/api/event";

interface EditorDestination {
    id: string;
    target: HTMLElement;
    path: string;
}

interface WorkspacePanel {
    id: string;
    target: HTMLElement;
    component: Component;
}

const workspacePanels = shallowRef<WorkspacePanel[]>([]);
const registeredPanels = new Set<string>();
const layoutContainer = ref<HTMLDivElement | null>(null);
const explorerTarget = shallowRef<HTMLElement | null>(null);
const editors = shallowRef<EditorDestination[]>([]);
const activePath = ref<string | null>(null);
const projectPath = ref<string | null>(null);

function activateFile(path: string | null) {
    if (activePath.value === path) return;
    activePath.value = path;
    if (path === null) {
        void emit("editor-active-file-changed", null).catch(console.error);
    }
}

let layout: GoldenLayout | undefined;
let resizeObserver: ResizeObserver | undefined;

function addWorkspacePanel(type: string, component: Component, title: string) {
    if (!layout?.rootItem) return;
    // These names already belong to the existing panel factories below.
    if (type === "editor" || type === "explorer") {
        console.error(`The component type "${type}" is reserved`);
        return;
    }
    const column = layout.rootItem.contentItems.find(
        (item) => item instanceof RowOrColumn && item.isColumn,
    );
    if (!(column instanceof RowOrColumn)) {
        console.error("Could not find the workspace column");
        return;
    }
    if (!registeredPanels.has(type)) {
        layout.registerComponentFactoryFunction(type, (container) => {
            const id = crypto.randomUUID();
            workspacePanels.value = [
                ...workspacePanels.value,
                { id, target: container.element, component },
            ];
            container.on("destroy", () => {
                workspacePanels.value = workspacePanels.value.filter(
                    (panel) => panel.id !== id,
                );
            });
        });
        registeredPanels.add(type);
    }
    const index = column.addItem({
        type: "stack",
        size: "20%",
        header: {
            show: "top",
            popout: false,
            maximise: false,
        },
        content: [
            {
                type: "component",
                componentType: type,
                title,
                isClosable: true,
            },
        ],
    });
    // Golden Layout resets added panels to equal sizes and omits size from its public types.
    const items = column.contentItems as (ContentItem & { size: number })[];
    const remaining = items.reduce((total, item, position) =>
        total + (position === index ? 0 : item.size), 0);
    if (remaining > 0) {
        items.forEach((item, position) => {
            item.size = position === index ? 20 : item.size / remaining * 80;
        });
        column.updateSize(false);
    }
}
// Allows the parent component to call this helper through a template ref.

defineExpose({ addWorkspacePanel });
// Kept outside onMounted so the template can call it.

function findEditor(item: ContentItem): ComponentItem | undefined {
    if (item instanceof ComponentItem && item.componentType === "editor") {
        return item;
    }
    for (const child of item.contentItems) {
        const found = findEditor(child);
        if (found) return found;
    }
    return undefined;
}

function toggleTerminal(event: KeyboardEvent) {
    if (!event.ctrlKey || event.code !== "Backquote" || event.repeat) return;
    event.preventDefault();
    if (!layout?.rootItem) return;
    function find(item: ContentItem): ComponentItem | undefined {
        if (item instanceof ComponentItem && item.componentType === "terminal") return item;
        for (const child of item.contentItems) {
            const terminal = find(child);
            if (terminal) return terminal;
        }
    }
    const terminal = find(layout.rootItem);
    if (terminal) terminal.close();
    else addWorkspacePanel("terminal", TerminalPanel, "Terminal");
}

function openFile(path: string) {
    if (!layout?.rootItem) return;
    const title = path.split(/[\\/]/).pop() || path;
    const editorGroup = findEditor(layout.rootItem)?.parent;
    if (editorGroup instanceof Stack) {
        const existingTab = editorGroup.contentItems.find((item) => {
            if (!(item instanceof ComponentItem)) return false;
            const state = item.container.state;
            return (
                state !== null &&
                typeof state === "object" &&
                "path" in state &&
                state.path === path
            );
        });
        if (existingTab instanceof ComponentItem) {
            editorGroup.setActiveComponentItem(existingTab, true);
            activateFile(path);
            return;
        }
    }
    // Add a new tab to an existing editor group.
    if (editorGroup instanceof Stack) {
        const tab = editorGroup.newItem({
            type: "component",
            componentType: "editor",
            title,
            componentState: { path },
            isClosable: true,
        }) as ComponentItem;
        editorGroup.setActiveComponentItem(tab, true);
        activateFile(path);
        return;
    }
    // The editor row stays empty until a file is opened.
    const editorColumn = layout.rootItem.contentItems.find(
        (item) => item instanceof RowOrColumn && item.isColumn,
    );
    if (!(editorColumn instanceof RowOrColumn)) {
        console.error("Could not find the editor column");
        return;
    }
    const editorArea = editorColumn.contentItems.find(item => item.id === "editors");
    if (!(editorArea instanceof RowOrColumn)) return;
    editorArea.addItem(
        {
            type: "stack",
            header: {
                show: "top",
                popout: false,
                maximise: false,
            },
            content: [
                {
                    type: "component",
                    componentType: "editor",
                    title,
                    componentState: { path },
                    isClosable: true,
                },
            ],
        },
        0,
    );
    activateFile(path);
}

function closeDeletedEditors(deletedPath: string) {
    if (!layout?.rootItem) return;
    const normalize = (path: string) =>
        path.replace(/\\/g, "/").replace(/\/+$/, "");
    const deleted = normalize(deletedPath);
    const matches: ComponentItem[] = [];
    function collect(item: ContentItem) {
        if (item instanceof ComponentItem && item.componentType === "editor") {
            const state = item.container.state;
            if (
                state &&
                typeof state === "object" &&
                "path" in state &&
                typeof state.path === "string"
            ) {
                const path = normalize(state.path);
                if (path === deleted || path.startsWith(`${deleted}/`)) {
                    matches.push(item);
                }
            }
        }
        for (const child of item.contentItems) {
            collect(child);
        }
    }
    collect(layout.rootItem);
    // Collect first: closing tabs changes the layout tree.
    for (const tab of matches) {
        tab.close();
    }
}

onMounted(() => {
    const element = layoutContainer.value;
    if (!element) return;
    layout = new GoldenLayout(element);
    layout.registerComponentFactoryFunction("explorer", (container) => {
        explorerTarget.value = container.element;
        container.on("destroy", () => {
            explorerTarget.value = null;
        });
    });
    // Each editor gets its own Teleport destination.
    layout.registerComponentFactoryFunction("editor", (container, state) => {
        if (
            !state ||
            typeof state !== "object" ||
            !("path" in state) ||
            typeof state.path !== "string"
        ) {
            throw new Error("Editor requires a file path");
        }
        const id = crypto.randomUUID();
        const path = state.path;
        container.on("shown", () => activateFile(path));
        container.on("focus", () => activateFile(path));
        editors.value = [
            ...editors.value,
            {
                id,
                target: container.element,
                path: state.path,
            },
        ];
        container.on("destroy", () => {
            editors.value = editors.value.filter((editor) => editor.id !== id);
            if (activePath.value === path) {
                activateFile(
                    editors.value.find((editor) => editor.target.offsetParent !== null)
                        ?.path ?? null,
                );
            }
        });
    });
    const config: LayoutConfig = {
        settings: {
            reorderEnabled: true,
        },
        dimensions: {
            borderWidth: 8,
            headerHeight: 32,
        },
        header: {
            show: false,
            popout: false,
        },
        root: {
            type: "row",
            content: [
                {
                    type: "stack",
                    size: "17%",
                    content: [
                        {
                            type: "component",
                            componentType: "explorer",
                            title: "Explorer",
                            isClosable: false,
                        },
                    ],
                },
                {
                    type: "column",
                    size: "83%",
                    isClosable: false,
                    content: [{
                        type: "row",
                        id: "editors",
                        size: "80%",
                        isClosable: false,
                        content: [],
                    }],
                },
            ],
        },
    };
    layout.loadLayout(config);

    addWorkspacePanel("terminal", TerminalPanel, "Terminal");
    window.addEventListener('keydown', toggleTerminal);
    resizeObserver = new ResizeObserver(() => {
        layout?.setSize(element.clientWidth, element.clientHeight);
    });
    resizeObserver.observe(element);
});

onBeforeUnmount(() => {
    window.removeEventListener('keydown', toggleTerminal);
    resizeObserver?.disconnect();
    layout?.destroy();
});
</script>

<template>
    <div ref="layoutContainer" class="workspace-layout">
        <Teleport v-if="explorerTarget" :to="explorerTarget">
            <ExplorerPanel :active-path="activePath" @open-file="openFile" @entry-deleted="closeDeletedEditors" @project-opened="projectPath = $event" />
        </Teleport>
        <Teleport v-for="editor in editors" :key="editor.id" :to="editor.target">
            <EditorPanel :path="editor.path" :active="activePath === editor.path" :project-path="projectPath"
                @activate="activateFile(editor.path)" />
        </Teleport>
        <Teleport v-for="panel in workspacePanels" :key="panel.id" :to="panel.target">
            <component :is="panel.component" :project-path="projectPath" />
        </Teleport>
    </div>
</template>

<style scoped>
.workspace-layout {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    color: #bac2ce;
}

/* Keep the wallpaper visible between panel groups. */
.workspace-layout :deep(.lm_goldenlayout),
.workspace-layout :deep(.lm_root),
.workspace-layout :deep(.lm_splitter) {
    background: transparent;
}

/* Frost the whole group, including its tab bar. */
.workspace-layout :deep(.lm_stack) {
    background: var(--lux-glass-bg);
    backdrop-filter: blur(var(--lux-glass-blur));
    border: 1px solid var(--lux-glass-border);
    border-radius: var(--lux-panel-radius);
    overflow: hidden;
}

.workspace-layout :deep(.lm_header) {
    background: rgb(255 255 255 / 3%);
}

.workspace-layout :deep(.lm_items),
.workspace-layout :deep(.lm_item_container),
.workspace-layout :deep(.lm_content) {
    background: transparent;
}

.workspace-layout :deep(.lm_content) {
    border: 0;
}

.workspace-layout :deep(.lm_content) {
    position: relative;
    overflow: hidden;
}

.workspace-layout :deep(.lm_tab) {
    height: 26px;
    margin: 3px 0 0 4px;
    padding: 0 12px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    box-shadow: none;
    color: #8491a3;
    font:
        12px / 26px system-ui,
        sans-serif;
}

.workspace-layout :deep(.lm_tab.lm_active) {
    color: #e0e7f0;
    background: rgb(255 255 255 / 7%);
    box-shadow: none;
}

.workspace-layout :deep(.lm_tab:hover) {
    color: #fff;
    background: rgb(255 255 255 / 10%);
}

.workspace-layout :deep(.panel-placeholder) {
    height: 100%;
    box-sizing: border-box;
    padding: 16px;
    color: #94a3b8;
    font:
        13px / 1.6 system-ui,
        sans-serif;
}

.workspace-layout :deep(.terminal-placeholder) {
    color: #7dd3fc;
    font-family: Consolas, monospace;
}

.workspace-layout :deep(.lm_tab) {
    position: relative;
    padding-right: 30px;
}

.workspace-layout :deep(.lm_close_tab) {
    position: absolute;
    top: 50%;
    right: 5px;
    width: 18px;
    height: 18px;
    transform: translateY(-50%);
    border-radius: 4px;
    background: transparent;
    opacity: 0.6;
    cursor: pointer;
}

.workspace-layout :deep(.lm_close_tab::before),
.workspace-layout :deep(.lm_close_tab::after) {
    content: "";
    position: absolute;
    top: 8px;
    left: 4px;
    width: 10px;
    height: 1px;
    background: currentColor;
}

.workspace-layout :deep(.lm_close_tab::before) {
    transform: rotate(45deg);
}

.workspace-layout :deep(.lm_close_tab::after) {
    transform: rotate(-45deg);
}

.workspace-layout :deep(.lm_close_tab:hover) {
    opacity: 1;
    background: var(--lux-hover);
}
</style>

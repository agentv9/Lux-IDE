<template>
    <header v-if="isMac" class="titlebar">


        <div class="window-controls mac">
            <button class="window-button mac" @click="appWindow.close()" aria-label="Close">
                <span class="status-dot text-error" aria-hidden="true" />
            </button>


            <button class="window-button mac" @click="appWindow.toggleMaximize()" aria-label="Maximize">
                <span class="status-dot text-warning" aria-hidden="true" />
            </button>

            <button class="window-button mac" @click="appWindow.minimize()" aria-label="Minimize">
                <span class="status-dot text-success" aria-hidden="true" />
            </button>
        </div>

        <div class="titlebar-drag" data-tauri-drag-region>
            <span class="titlebar-name-mac">Lux</span>
        </div>
    </header>

    <header v-else class="titlebar">
        <div class="titlebar-drag" data-tauri-drag-region>
            <span class="titlebar-name">Lux</span>
        </div>

        <div class="window-controls">
            <button class="window-button" @click="appWindow.minimize()" aria-label="Minimize">
                <IconMinus />
            </button>

            <button class="window-button" @click="appWindow.toggleMaximize()" aria-label="Maximize">
                <IconSquare />
            </button>

            <button class="window-button window-close" @click="appWindow.close()" aria-label="Close">
                <IconX />
            </button>
        </div>
    </header>
</template>

<script setup lang="ts">
import { IconMinus, IconSquare, IconX } from "@tabler/icons-vue"
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref } from "vue";

const isMac = ref(false)


const appWindow = getCurrentWindow()
</script>

<style scoped>
.status-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    flex-shrink: 0;
    border-radius: 50%;
    background: currentColor;
}

.titlebar {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    z-index: 100;

    display: flex;
    align-items: stretch;
    height: 32px;

    color: #bac2ce;
    background: rgb(12 16 22 / 60%);
    backdrop-filter: blur(18px) saturate(120%);
    -webkit-backdrop-filter: blur(18px) saturate(120%);

    border: 1px solid rgb(255 255 255 / 9%);
    border-radius: 10px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 20%);
    overflow: hidden;
    user-select: none;
}

.titlebar-drag {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    padding-left: 12px;
}

.titlebar-name {
    font: 500 12px / 1 system-ui, sans-serif;
    letter-spacing: 0.04em;
    pointer-events: none;
}

.titlebar-name-mac {
    font: 500 12px / 1 system-ui, sans-serif;
    letter-spacing: 0.04em;
    pointer-events: none;
    margin: auto;
}

.window-controls {
    display: flex;
    height: 100%;
}

.window-controls.mac {
    padding-left: 10px;
}

.window-button.mac {
    width: 20px;
    height: 100%;
    transition: none;
}

.window-button {
    display: grid;
    place-items: center;
    width: 40px;
    height: 100%;
    padding: 0;

    border: 0;
    border-radius: 0;
    color: inherit;
    background: transparent;
    cursor: default;

    transition: background-color 120ms ease, color 120ms ease;
}

.window-button:hover {
    color: #fff;
    background: rgb(255 255 255 / 10%);
}

.window-button.mac:hover {
    background: none;
}

.window-button:active {
    background: rgb(255 255 255 / 16%);
}

.window-button.mac:active {
    background: none;
}

.window-button:focus-visible {
    outline: 1px solid #78baff;
    outline-offset: -3px;
}

.window-button svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
}

.window-close:hover {
    color: #fff;
    background: rgb(210 45 65 / 80%);
}

.window-close:active {
    background: rgb(180 35 50 / 90%);
}
</style>
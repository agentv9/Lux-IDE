<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { Channel, invoke } from '@tauri-apps/api/core'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

const props = defineProps<{
    projectPath: string | null
}>()
const container = ref<HTMLDivElement | null>(null)
const id = crypto.randomUUID()
const terminal = new Terminal({
    allowTransparency: true,
    fontFamily: 'Consolas, "Courier New", monospace',
    fontSize: 13,
    theme: {
        background: '#00000000',
        foreground: '#bac2ce',
        cursor: '#bac2ce',
    },
})
const fit = new FitAddon()
let observer: ResizeObserver | undefined
let ready = false
let unmounted = false
let starting = false
let sessionNumber = 0
let pendingInput = ''

function resize() {
    if (!container.value?.clientWidth || !container.value.clientHeight) return
    fit.fit()
    if (ready) {
        void invoke('terminal_resize', { id, cols: terminal.cols, rows: terminal.rows }).catch(console.error)
    }
}

async function start() {
    if (starting || unmounted) return
    starting = true
    const currentSession = ++sessionNumber
    const cwd = props.projectPath
    ready = false
    pendingInput = ''
    let exited = false
    try {
        await invoke('terminal_close', { id })
        if (unmounted) return
        terminal.reset()
        resize()
        const output = new Channel<number[]>()
        output.onmessage = data => {
            if (unmounted || currentSession !== sessionNumber) return
            if (data.length) terminal.write(new Uint8Array(data))
            else {
                exited = true
                ready = false
                terminal.writeln('\r\n[Shell exited. Press Enter to restart.]')
            }
        }
        await invoke('terminal_open', { id, cwd, cols: terminal.cols, rows: terminal.rows, output })
        if (unmounted) await invoke('terminal_close', { id })
        else if (!exited) {
            ready = true
            if (pendingInput) {
                await invoke('terminal_write', { id, data: pendingInput })
                pendingInput = ''
            }
            resize()
        }
    } catch (error) {
        if (!unmounted) terminal.writeln(`\r\n${String(error)}\r\nPress Enter to retry.`)
    } finally {
        starting = false
        if (!unmounted && cwd !== props.projectPath) void start()
    }
}

watch(() => props.projectPath, () => { void start() })

onMounted(() => {
    if (!container.value) return
    terminal.loadAddon(fit)
    terminal.open(container.value)
    terminal.onData(data => {
        if (ready) void invoke('terminal_write', { id, data }).catch(console.error)
        else if (starting) pendingInput += data
        else if (data === '\r') void start()
    })
    observer = new ResizeObserver(resize)
    observer.observe(container.value)
    void start()
})

onBeforeUnmount(() => {
    unmounted = true
    observer?.disconnect()
    terminal.dispose()
    void invoke('terminal_close', { id }).catch(console.error)
})
</script>

<template>
    <div ref="container" class="terminal-panel" />
</template>

<style>

.xterm .xterm-viewport {
background-color: #00000000 !important;
}

.xterm-viewport::-webkit-scrollbar {
  width: 5px;
  height: 5px;
}

.xterm-viewport::-webkit-scrollbar-track,
.xterm-viewport::-webkit-scrollbar-corner {
  background: transparent;
}

.xterm-viewport::-webkit-scrollbar-thumb {
  background: rgb(129 146 179 / 40%);
  border-radius: 999px;
}

.xterm-viewport::-webkit-scrollbar-thumb:hover {
  background: rgb(105 165 255 / 65%);
}

.xterm-viewport::-webkit-scrollbar-button {
  display: none;
  width: 0;
  height: 0;
}

@supports selector(::-webkit-scrollbar) {
  .xterm-viewport {
    scrollbar-width: auto;
    scrollbar-color: auto;
  }
}


</style>

<style scoped>
.terminal-panel {
    height: 100%;
    box-sizing: border-box;
    padding: 8px;
    overflow: hidden;
}

.terminal-panel :deep(.xterm) {
    height: 100%;
}
</style>

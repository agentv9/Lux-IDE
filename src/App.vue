<script setup lang="ts">
import Titlebar from './components/app/titlebar.vue';
import WorkspaceLayout from './layout/WorkspaceLayout.vue';


import { onMounted, onBeforeUnmount } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

async function handleKeydown(event: KeyboardEvent) {
  if (event.key !== 'F11') return

  event.preventDefault()
  if (event.repeat) return

  try {
    const fullscreen = await appWindow.isFullscreen()
    await appWindow.setFullscreen(!fullscreen)
  } catch (error) {
    console.error('Could not toggle fullscreen:', error)
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <UApp>
    <main class="workspace">
      <Titlebar />
      <WorkspaceLayout />
    </main>
  </UApp>

</template>

<style scoped>
.workspace {
  position: relative;
  width: 100%;
  height: 100dvh;
  box-sizing: border-box;
  padding: 48px 8px 8px;
  background: url('./assets/background.jfif') center / cover no-repeat;
}

.workspace-layout {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}
</style>
<style>
html,
body,
#app {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}
</style>
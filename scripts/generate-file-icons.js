import { cp, mkdir, writeFile } from 'node:fs/promises'
import { generateManifest } from 'material-icon-theme'

const theme = new URL('../node_modules/material-icon-theme/', import.meta.url)
const output = new URL('../public/material-icons/', import.meta.url)

await mkdir(output, { recursive: true })
await cp(new URL('icons/', theme), output, { recursive: true })
await cp(new URL('LICENSE', theme), new URL('LICENSE', output))

await writeFile(
  new URL('../src/file-icon-theme.json', import.meta.url),
  JSON.stringify(generateManifest(), null, 2),
)
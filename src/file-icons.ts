import manifest from './file-icon-theme.json'

const fileNames: Record<string, string> = manifest.fileNames
const fileExtensions: Record<string, string> = manifest.fileExtensions
const folderNames: Record<string, string> = manifest.folderNames
const openFolderNames: Record<string, string> = manifest.folderNamesExpanded
const definitions: Record<string, { iconPath: string }> = manifest.iconDefinitions

export function getFileIcon(
  name: string,
  isDirectory: boolean,
  expanded = false,
) {
  const key = name.toLowerCase()
  let iconId: string

  if (isDirectory) {
    iconId = expanded
      ? openFolderNames[key] ?? manifest.folderExpanded
      : folderNames[key] ?? manifest.folder
  } else {
    const suffixes = key.split('.').slice(1)
    const extensionIcon = suffixes
      .map((_, index) => fileExtensions[suffixes.slice(index).join('.')])
      .find(Boolean)

    iconId = fileNames[key] ?? extensionIcon ?? manifest.file
  }

  const svg = definitions[iconId]?.iconPath.split('/').pop() ?? 'file.svg'
  return `${import.meta.env.BASE_URL}material-icons/${svg}`
}
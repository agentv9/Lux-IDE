import { Channel, invoke } from '@tauri-apps/api/core'
import type { server } from 'typescript'
import { monaco } from './monaco'

// Project information comes from tsserver rather than the browser worker.
monaco.typescript.typescriptDefaults.setModeConfiguration({})
monaco.typescript.javascriptDefaults.setModeConfiguration({})

type Protocol = server.protocol.Response & { event?: string }
type Diagnostic = server.protocol.Diagnostic

interface OpenFile {
    model: monaco.editor.ITextModel
    projectPath: string | null
    onError: (message: string) => void
    diagnostics: Map<string, Diagnostic[]>
}

interface Completion extends monaco.languages.CompletionItem {
    file: string
    position: monaco.Position
    entry: server.protocol.CompletionEntry
}

interface PendingRequest {
    resolve: (body: unknown) => void
    reject: (error: Error) => void
    timer: ReturnType<typeof setTimeout>
}

const files = new Map<string, OpenFile>()
const pending = new Map<number, PendingRequest>()
let sequence = 0
let ready: Promise<void> | undefined
let serverError = ''

function normalizePath(path: string) {
    const normalized = path.replace(/\\/g, '/')
    return /^[a-z]:/i.test(normalized) ? normalized.toLowerCase() : normalized
}

function send(command: string, args: unknown, seq = ++sequence) {
    return invoke('typescript_send', { message: { seq, type: 'request', command, arguments: args } })
}

function request<T>(command: string, args: unknown): Promise<T | undefined> {
    const seq = ++sequence
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            pending.delete(seq)
            reject(new Error(`TypeScript ${command} timed out`))
        }, 15000)
        pending.set(seq, { resolve: body => resolve(body as T | undefined), reject, timer })
        void send(command, args, seq).catch(error => {
            clearTimeout(timer)
            pending.delete(seq)
            reject(error)
        })
    })
}

function receive(message: Protocol) {
    if (message.type === 'response') {
        const callback = pending.get(message.request_seq)
        if (!callback) return
        clearTimeout(callback.timer)
        pending.delete(message.request_seq)
        if (message.success) callback.resolve(message.body)
        else callback.resolve(undefined)
    } else if (message.event === 'serverError') {
        serverError += `${String(message.body)}\n`
        console.error('TypeScript:', message.body)
    } else if (message.event === 'serverExit') {
        ready = undefined
        const error = serverError || 'TypeScript server exited. Retry the connection.'
        for (const callback of pending.values()) {
            clearTimeout(callback.timer)
            callback.reject(new Error(error))
        }
        pending.clear()
        for (const file of files.values()) file.onError(error)
        console.error(error)
    } else if (message.event === 'syntaxDiag' || message.event === 'semanticDiag' || message.event === 'suggestionDiag') {
        const body = message.body as server.protocol.DiagnosticEventBody
        const file = files.get(normalizePath(body.file))
        if (!file || file.model.isDisposed()) return
        file.diagnostics.set(message.event, body.diagnostics)
        const markers: monaco.editor.IMarkerData[] = []
        for (const group of file.diagnostics.values()) {
            for (const diagnostic of group) {
                let severity = monaco.MarkerSeverity.Hint
                if (diagnostic.category === 'error') severity = monaco.MarkerSeverity.Error
                if (diagnostic.category === 'warning') severity = monaco.MarkerSeverity.Warning

                markers.push({
                    startLineNumber: diagnostic.start.line,
                    startColumn: diagnostic.start.offset,
                    endLineNumber: diagnostic.end.line,
                    endColumn: diagnostic.end.offset,
                    message: diagnostic.text,
                    code: diagnostic.code?.toString(),
                    source: 'ts',
                    severity,
                })
            }
        }
        monaco.editor.setModelMarkers(file.model, 'typescript', markers)
    }
}

async function start() {
    if (!ready) {
        serverError = ''
        const output = new Channel<Protocol>()
        output.onmessage = receive
        ready = invoke<void>('typescript_start', { output }).then(async () => {
            await request('configure', {
                preferences: { includeCompletionsForModuleExports: true, includeCompletionsWithInsertText: true },
            })
            await send('compilerOptionsForInferredProjects', {
                options: { allowJs: true, checkJs: true, target: 'ESNext', module: 'NodeNext', moduleResolution: 'NodeNext', strict: true },
            })
            for (const { model, projectPath, onError } of files.values()) {
                if (model.isDisposed()) continue
                await send('open', { file: model.uri.fsPath, fileContent: model.getValue(), projectRootPath: projectPath ?? undefined })
                onError('')
            }
            if (files.size) await checkErrors()
        }).catch(error => {
            ready = undefined
            throw error
        })
    }
    await ready
}

function getLocation(model: monaco.editor.ITextModel, position: monaco.Position) {
    return { file: model.uri.fsPath, line: position.lineNumber, offset: position.column }
}

function getRange(span: server.protocol.TextSpan) {
    return new monaco.Range(span.start.line, span.start.offset, span.end.line, span.end.offset)
}

const selector = ['typescript', 'javascript', 'typescriptreact', 'javascriptreact']

function checkErrors() {
    const paths: string[] = []
    for (const file of files.values()) paths.push(file.model.uri.fsPath)
    return send('geterr', { files: paths, delay: 0 })
}

monaco.languages.registerHoverProvider(selector, {
    async provideHover(model, position) {
        if (!ready || !files.has(normalizePath(model.uri.fsPath))) return
        try {
            const info = await request<server.protocol.QuickInfoResponseBody>('quickinfo', getLocation(model, position))
            if (!info) return
            let documentation = ''
            if (typeof info.documentation === 'string') documentation = info.documentation
            else {
                for (const part of info.documentation) documentation += part.text
            }
            return {
                range: getRange(info),
                contents: [
                    { value: `\`\`\`typescript\n${info.displayString}\n\`\`\`` },
                    { value: documentation },
                ],
            }
        } catch (error) {
            console.error(error)
        }
    },
})

monaco.languages.registerCompletionItemProvider(selector, {
    triggerCharacters: ['.', '"', "'", '/', '@'],
    async provideCompletionItems(model, position) {
        if (!ready || !files.has(normalizePath(model.uri.fsPath))) return
        try {
            const info = await request<server.protocol.CompletionInfo>('completionInfo', {
                ...getLocation(model, position), includeExternalModuleExports: true, includeInsertTextCompletions: true,
            })
            const word = model.getWordUntilPosition(position)
            const defaultRange = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn)
            const suggestions: Completion[] = []
            for (const entry of info?.entries ?? []) {
                let kind = monaco.languages.CompletionItemKind.Variable
                if (entry.kind === 'method') kind = monaco.languages.CompletionItemKind.Method
                if (entry.kind === 'function') kind = monaco.languages.CompletionItemKind.Function
                if (entry.kind === 'class') kind = monaco.languages.CompletionItemKind.Class
                if (entry.kind === 'property') kind = monaco.languages.CompletionItemKind.Property
                let replacement = defaultRange
                if (entry.replacementSpan) replacement = getRange(entry.replacementSpan)

                suggestions.push({
                    file: model.uri.fsPath,
                    position,
                    entry,
                    label: entry.name,
                    insertText: entry.insertText ?? entry.name,
                    sortText: entry.sortText,
                    insertTextRules: entry.isSnippet ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet : undefined,
                    kind,
                    range: replacement,
                })
            }
            return { suggestions }
        } catch (error) {
            console.error(error)
        }
    },
    async resolveCompletionItem(item) {
        const completion = item as Completion
        if (!ready) return item
        try {
            const details = await request<server.protocol.CompletionEntryDetails[]>('completionEntryDetails', {
                file: completion.file,
                line: completion.position.lineNumber,
                offset: completion.position.column,
                entryNames: [{ name: completion.entry.name, source: completion.entry.source, data: completion.entry.data }],
            })
            const detail = details?.[0]
            if (!detail) return item
            item.detail = ''
            for (const part of detail.displayParts) item.detail += part.text
            let documentation = ''
            for (const part of detail.documentation ?? []) documentation += part.text
            item.documentation = { value: documentation }
            item.additionalTextEdits = []
            for (const action of detail.codeActions ?? []) {
                for (const change of action.changes) {
                    if (normalizePath(change.fileName) !== normalizePath(completion.file)) continue
                    for (const edit of change.textChanges) {
                        item.additionalTextEdits.push({ range: getRange(edit), text: edit.newText })
                    }
                }
            }
        } catch (error) {
            console.error(error)
        }
        return item
    },
})

export async function connectTypeScript(model: monaco.editor.ITextModel, projectPath: string | null, onError: (message: string) => void) {
    if (!selector.includes(model.getLanguageId())) return
    await start()
    if (model.isDisposed()) return
    const file = model.uri.fsPath
    const path = normalizePath(file)
    files.set(path, { model, projectPath, onError, diagnostics: new Map() })
    let closed = false
    let timer: ReturnType<typeof setTimeout> | undefined

    async function check() {
        if (closed || model.isDisposed() || !ready) return
        await checkErrors()
    }

    await send('open', { file, fileContent: model.getValue(), projectRootPath: projectPath ?? undefined })
    void check().catch(console.error)
    const changes = model.onDidChangeContent(event => {
        if (!ready) return
        // Monaco offsets refer to the text before this edit; apply from the end.
        const sortedChanges = [...event.changes].sort((a, b) => b.rangeOffset - a.rangeOffset)
        const textChanges: server.protocol.CodeEdit[] = []
        for (const change of sortedChanges) {
            textChanges.push({
                start: { line: change.range.startLineNumber, offset: change.range.startColumn },
                end: { line: change.range.endLineNumber, offset: change.range.endColumn },
                newText: change.text,
            })
        }
        void send('updateOpen', { changedFiles: [{ fileName: file, textChanges }] }).catch(console.error)
        clearTimeout(timer)
        timer = setTimeout(() => { void check().catch(console.error) }, 350)
    })

    return {
        dispose() {
            closed = true
            clearTimeout(timer)
            changes.dispose()
            files.delete(path)
            if (!model.isDisposed()) monaco.editor.setModelMarkers(model, 'typescript', [])
            if (ready) void send('close', { file }).catch(console.error)
        },
    }
}

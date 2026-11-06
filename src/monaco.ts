import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/language/json/json.worker?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker?worker";
import CssWorker from "monaco-editor/language/css/css.worker?worker";
import { shikiToMonaco } from "@shikijs/monaco";
import { createHighlighter, type BundledLanguage } from "shiki";
import definitions from "./language-definitions.json";

interface LanguageDefinition {
  id: string;
  grammar: BundledLanguage;
  extensions: string[];
  filenames?: string[];
}

const languages = definitions as LanguageDefinition[];

self.MonacoEnvironment = {
  getWorker(_moduleId, label) {
    if (label === "json") return new JsonWorker();
    if (label === "html" || label === "handlebars" || label === "razor") return new HtmlWorker();
    if (label === "css" || label === "scss" || label === "less") return new CssWorker();
    return new EditorWorker();
  },
};

const highlightingReady = (async () => {
  const registered = new Set(
    monaco.languages.getLanguages().map((language) => language.id),
  );

  for (const language of languages) {
    if (!registered.has(language.id)) {
      monaco.languages.register({
        id: language.id,
        extensions: language.extensions,
        filenames: language.filenames,
      });

      registered.add(language.id);
    }
  }

  const highlighter = await createHighlighter({
    themes: ["vitesse-dark", "vitesse-light"],
    langs: [...new Set(languages.map((language) => language.grammar))],
  });

  shikiToMonaco(highlighter, monaco);
})();

const extensions = languages
  .flatMap((language) =>
    language.extensions.map((extension) => ({
      extension: extension.toLowerCase(),
      id: language.id,
    })),
  )
  .sort((a, b) => b.extension.length - a.extension.length);

function getLanguage(path: string): string {
  const filename = path.split(/[\\/]/).pop()?.toLowerCase() ?? "";

  const exact = languages.find((language) =>
    language.filenames?.some((name) => name.toLowerCase() === filename),
  );

  if (exact) return exact.id;

  return (
    extensions.find(({ extension }) => filename.endsWith(extension))?.id ??
    "plaintext"
  );
}

export { monaco, highlightingReady, getLanguage };

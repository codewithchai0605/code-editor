import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";

import hljs from "highlight.js/lib/core";

import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import css from "highlight.js/lib/languages/css";
import json from "highlight.js/lib/languages/json";
import yaml from "highlight.js/lib/languages/yaml";
import env from "highlight.js/lib/languages/ini";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import bash from "highlight.js/lib/languages/bash";
import markdown from "highlight.js/lib/languages/markdown";

hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("jsx", javascript);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("tsx", typescript);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("css", css);
hljs.registerLanguage("json", json);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("env", env);
hljs.registerLanguage("ini", env);
hljs.registerLanguage("python", python);
hljs.registerLanguage("py", python);

hljs.registerLanguage("rust", rust);
hljs.registerLanguage("rs", rust);
hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);

const md = new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true,
    highlight(str, lang) {
        if (lang) {
            const language = lang.trim().toLowerCase();
            if (hljs.getLanguage(language)) {
                try {
                    return hljs.highlight(str, {
                        language,
                        ignoreIllegals: true,
                    }).value;
                } catch {
                    // Fall through to MarkdownIt's normal code rendering.
                }
            }
        }
        // Returning an empty string tells markdown-it to use its
        // normal escaped <code> output instead.
        return "";
    },
});

/**
 * Render Markdown into sanitized HTML.
 *
 * This function is intended for use with Svelte's:
 *
 *     {@html renderMarkdownToSafeHtml(markdown)}
 *
 * Markdown may come from arbitrary files on disk, so the generated HTML
 * must be sanitized before it is inserted into the DOM.
 */
export function renderMarkdownToSafeHtml(source: string): string {
    const rendered = md.render(source);

    return DOMPurify.sanitize(rendered);
}

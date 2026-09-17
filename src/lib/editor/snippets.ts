import {
    snippetCompletion,
    completeFromList,
    type Completion,
    type CompletionSource,
} from "@codemirror/autocomplete";

/**
 * Hand-picked starter snippets per language group — not exhaustive, just
 * the handful of constructs that are tedious to retype by hand. `${}`
 * marks a tab stop; `${name}` a named, pre-selected tab stop (see
 * `snippetCompletion`'s docs) so Tab/Shift-Tab jump between them and the
 * first one starts pre-selected for immediate typing.
 *
 * Grouped by language family rather than by file extension, then mapped
 * onto extensions below — every JS-flavored extension (.js/.jsx/.ts/.tsx/…)
 * shares one set rather than four near-duplicate copies.
 */
const javascriptSnippets: Completion[] = [
    snippetCompletion("console.log(${})", { label: "clg", detail: "console.log(…)" }),
    snippetCompletion("function ${name}(${params}) {\n\t${}\n}", {
        label: "func",
        detail: "function",
    }),
    snippetCompletion("(${params}) => {\n\t${}\n}", { label: "afunc", detail: "arrow function" }),
    snippetCompletion('import { ${names} } from "${module}";', {
        label: "imp",
        detail: "named import",
    }),
    snippetCompletion("export default ${name};", { label: "exp", detail: "default export" }),
    snippetCompletion("try {\n\t${}\n} catch (${err}) {\n\t${}\n}", {
        label: "trycatch",
        detail: "try/catch",
    }),
    snippetCompletion("if (${cond}) {\n\t${}\n}", { label: "if", detail: "if block" }),
    snippetCompletion("class ${Name} {\n\tconstructor(${params}) {\n\t\t${}\n\t}\n}", {
        label: "class",
        detail: "class",
    }),
];

const pythonSnippets: Completion[] = [
    snippetCompletion("def ${name}(${params}):\n\t${}", { label: "def", detail: "function" }),
    snippetCompletion("class ${Name}:\n\tdef __init__(self${params}):\n\t\t${}", {
        label: "class",
        detail: "class",
    }),
    snippetCompletion('if __name__ == "__main__":\n\t${}', { label: "main", detail: "main guard" }),
    snippetCompletion("try:\n\t${}\nexcept ${Exception} as e:\n\t${}", {
        label: "trycatch",
        detail: "try/except",
    }),
    snippetCompletion("for ${item} in ${iterable}:\n\t${}", { label: "for", detail: "for loop" }),
];

const rustSnippets: Completion[] = [
    snippetCompletion("fn ${name}(${args}) -> ${ReturnType} {\n\t${}\n}", {
        label: "fn",
        detail: "function",
    }),
    snippetCompletion("struct ${Name} {\n\t${}\n}", { label: "struct", detail: "struct" }),
    snippetCompletion("impl ${Name} {\n\t${}\n}", { label: "impl", detail: "impl block" }),
    snippetCompletion("#[derive(${Debug, Clone})]", {
        label: "derive",
        detail: "derive attribute",
    }),
    snippetCompletion("#[test]\nfn ${name}() {\n\t${}\n}", {
        label: "test",
        detail: "test function",
    }),
    snippetCompletion("match ${expr} {\n\t${pattern} => ${result},\n}", {
        label: "match",
        detail: "match expression",
    }),
];

const javaSnippets: Completion[] = [
    snippetCompletion("System.out.println(${});", { label: "sout", detail: "println" }),
    snippetCompletion("public static void main(String[] args) {\n\t${}\n}", {
        label: "psvm",
        detail: "main method",
    }),
    snippetCompletion("public class ${Name} {\n\t${}\n}", { label: "class", detail: "class" }),
];

const cLikeSnippets: Completion[] = [
    snippetCompletion("int main() {\n\t${}\n\treturn 0;\n}", {
        label: "main",
        detail: "main function",
    }),
    snippetCompletion('printf("${text}\\n");', { label: "printf", detail: "printf" }),
];

const cppSnippets: Completion[] = [
    ...cLikeSnippets,
    snippetCompletion("std::cout << ${text} << std::endl;", { label: "cout", detail: "cout" }),
];

const SNIPPETS_BY_EXT: Record<string, Completion[]> = {
    js: javascriptSnippets,
    mjs: javascriptSnippets,
    cjs: javascriptSnippets,
    jsx: javascriptSnippets,
    ts: javascriptSnippets,
    tsx: javascriptSnippets,
    py: pythonSnippets,
    rs: rustSnippets,
    java: javaSnippets,
    c: cLikeSnippets,
    h: cLikeSnippets,
    cpp: cppSnippets,
    cc: cppSnippets,
    hpp: cppSnippets,
};

/** A completion source for the given extension's snippet set, or `null`
 *  if there isn't one — callers should skip attaching anything in that
 *  case rather than wiring up a source with nothing to offer. */
export function snippetSourceFor(extension: string): CompletionSource | null {
    const list = SNIPPETS_BY_EXT[extension];
    return list ? completeFromList(list) : null;
}

import { RangeSetBuilder, type Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";

const HEX_COLOR_RE = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b/g;
const RGB_COLOR_RE = /\brgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(?:,\s*[\d.]+\s*)?\)/g;

/** Caps how many swatches render per viewport redraw — a pathological
 *  file (e.g. a generated palette dump) shouldn't be able to make this
 *  extension the bottleneck. */
const MAX_CHIPS = 500;

class ColorSwatchWidget extends WidgetType {
  constructor(private readonly color: string) {
    super();
  }

  eq(other: ColorSwatchWidget): boolean {
    return other.color === this.color;
  }

  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-color-chip";
    span.style.backgroundColor = this.color;
    span.setAttribute("aria-hidden", "true");
    return span;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

function findMatches(text: string, re: RegExp, offset: number, out: { pos: number; color: string }[]): void {
  re.lastIndex = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    out.push({ pos: offset + m.index, color: m[0] });
  }
}

function buildDecorations(view: EditorView): DecorationSet {
  const matches: { pos: number; color: string }[] = [];

  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to);
    findMatches(text, HEX_COLOR_RE, from, matches);
    findMatches(text, RGB_COLOR_RE, from, matches);
  }

  // Both regexes ran over the same ranges independently, so results need
  // an explicit sort — RangeSetBuilder requires strictly non-decreasing
  // `from` positions and will throw otherwise.
  matches.sort((a, b) => a.pos - b.pos);

  const builder = new RangeSetBuilder<Decoration>();
  for (const { pos, color } of matches.slice(0, MAX_CHIPS)) {
    builder.add(pos, pos, Decoration.widget({ widget: new ColorSwatchWidget(color), side: -1 }));
  }
  return builder.finish();
}

const colorChipPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }

    update(update: ViewUpdate): void {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (value) => value.decorations },
);

const colorChipTheme = EditorView.baseTheme({
  ".cm-color-chip": {
    display: "inline-block",
    width: "0.8em",
    height: "0.8em",
    marginRight: "3px",
    borderRadius: "2px",
    border: "1px solid rgba(255, 255, 255, 0.3)",
    verticalAlign: "middle",
  },
});

export function colorChips(): Extension {
  return [colorChipPlugin, colorChipTheme];
}

export default colorChips;

/**
 * Resolves a CSS custom property to its current literal value.
 *
 * Needed anywhere a value has to be handed to something that isn't part of
 * the DOM style cascade — e.g. xterm.js's `fontFamily` option, which it
 * feeds straight into a `<canvas>` 2D context's `font` property. Canvas
 * font parsing has no notion of `var(...)`: unlike an actual CSS rule
 * (which the browser resolves against `:root` normally), a raw
 * `"var(--font-mono)"` string handed to `ctx.font` is simply invalid and
 * gets silently ignored, falling back to the canvas default font. Reading
 * the variable's resolved value via `getComputedStyle` first and passing
 * that literal string instead sidesteps the problem entirely.
 */
export function resolveCssVar(name: string, fallback = ""): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

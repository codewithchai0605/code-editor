export type PaletteKind = "quickOpen" | "commandPalette" | "theme" | null;

class PaletteState {
  active = $state<PaletteKind>(null);

  open(kind: PaletteKind): void {
    this.active = kind;
  }

  close(): void {
    this.active = null;
  }

  toggle(kind: PaletteKind): void {
    this.active = this.active === kind ? null : kind;
  }
}

export const paletteState = new PaletteState();

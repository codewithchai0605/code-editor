export interface Command {
  id: string;
  title: string;
  /** Display-only hint (e.g. "Ctrl+B") — see the note in coreCommands.ts
   *  about why this doesn't also *bind* the key. */
  keybinding?: string;
  run: () => void | Promise<void>;
}

class CommandRegistry {
  commands = $state<Command[]>([]);

  /** Returns an unregister function, so contributors (including a future
   *  extension host) can cleanly remove their own commands later. */
  register(command: Command): () => void {
    this.commands.push(command);
    return () => {
      const idx = this.commands.findIndex((c) => c.id === command.id);
      if (idx !== -1) this.commands.splice(idx, 1);
    };
  }
}

export const commandRegistry = new CommandRegistry();

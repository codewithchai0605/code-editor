export interface Toast {
    id: number;
    message: string;
    kind: "info" | "success" | "error";
}

const DEFAULT_DURATION: Record<Toast["kind"], number> = {
    info: 3000,
    success: 3000,
    error: 6000,
};

/** Lightweight, app-wide toast notifications — a small stack in the
 *  corner rather than a modal, since none of what triggers these (a git
 *  action succeeding/failing, a file operation failing) needs to block
 *  anything. Auto-dismisses after a duration that scales with how urgent
 *  the kind is; the person can also dismiss one early. */
class ToastStore {
    toasts = $state<Toast[]>([]);
    private counter = 0;
    private timers = new Map<number, ReturnType<typeof setTimeout>>();

    private push(message: string, kind: Toast["kind"], durationMs: number): void {
        const id = ++this.counter;
        this.toasts.push({ id, message, kind });
        this.timers.set(
            id,
            setTimeout(() => this.dismiss(id), durationMs),
        );
    }

    info(message: string, durationMs = DEFAULT_DURATION.info): void {
        this.push(message, "info", durationMs);
    }

    success(message: string, durationMs = DEFAULT_DURATION.success): void {
        this.push(message, "success", durationMs);
    }

    error(message: string, durationMs = DEFAULT_DURATION.error): void {
        this.push(message, "error", durationMs);
    }

    dismiss(id: number): void {
        const idx = this.toasts.findIndex((t) => t.id === id);
        if (idx !== -1) this.toasts.splice(idx, 1);
        const timer = this.timers.get(id);
        if (timer) {
            clearTimeout(timer);
            this.timers.delete(id);
        }
    }
}

export const toastStore = new ToastStore();

import type { UpdateInfo } from "../../lib/commands";
import { useOpenExternal } from "./useOpenExternal";

interface UpdateBannerProps {
  update: UpdateInfo;
  onDismiss: () => void;
}

export function UpdateBanner({ update, onDismiss }: UpdateBannerProps) {
  const { open, element: linkFailure } = useOpenExternal();

  return (
    <div className="flex items-center justify-between gap-3 border-b border-[var(--color-border)] bg-[var(--color-accent)]/10 px-4 py-2 text-sm">
      {linkFailure}
      <span>
        Amanuensis v{update.version} is available —{" "}
        <button
          onClick={() => open(update.url)}
          className="font-medium text-[var(--color-accent)] underline hover:no-underline"
        >
          View Release
        </button>
      </span>
      <button
        onClick={onDismiss}
        className="text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
        aria-label="Dismiss"
      >
        ✕
      </button>
    </div>
  );
}

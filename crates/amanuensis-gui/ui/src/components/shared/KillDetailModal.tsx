import { useState } from "react";
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";
import { getCreatureImageUrl } from "../../lib/bestiary";

interface KillDetailModalProps {
  kill: Kill;
  onClose: () => void;
}

export function KillDetailModal({ kill, onClose }: KillDetailModalProps) {
  const entry = useStore((s) => s.bestiaryByName[kill.creature_name]);
  const imgUrl = getCreatureImageUrl(kill.creature_name);
  const [imgFailed, setImgFailed] = useState(false);

  const totalKills =
    kill.killed_count +
    kill.slaughtered_count +
    kill.vanquished_count +
    kill.dispatched_count +
    kill.assisted_kill_count +
    kill.assisted_slaughter_count +
    kill.assisted_vanquish_count +
    kill.assisted_dispatch_count;

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-2xl rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="mb-3 flex items-start gap-3">
          {imgUrl && !imgFailed && (
            <img
              src={imgUrl}
              alt={kill.creature_name}
              width={entry?.static_width ?? undefined}
              height={entry?.static_height ?? undefined}
              className="rounded border border-[var(--color-border)]"
              onError={() => setImgFailed(true)}
            />
          )}
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-bold">{kill.creature_name}</h2>
            {entry && (
              <p className="text-xs text-[var(--color-text-muted)]">
                {entry.family_canonical ?? entry.family ?? "Unknown family"} ·{" "}
                {entry.rarity_canonical ?? "Unknown"}
              </p>
            )}
          </div>
        </header>

        {entry ? (
          <div className="grid grid-cols-1 gap-x-6 gap-y-1 text-sm md:grid-cols-2">
            <Field label="Exp / taxidermy" value={`${entry.exp_taxidermy}`} />
            {entry.location && <Field label="Location" value={entry.location} />}
            {entry.difficulty && <Field label="Difficulty" value={entry.difficulty} long />}
            <Stat label="Attack" value={entry.attack} measured={entry.attack_measured} />
            <Stat label="Defense" value={entry.defense} measured={entry.defense_measured} />
            <Stat label="Damage" value={entry.damage} measured={entry.damage_measured} />
            <Stat label="Health" value={entry.health} measured={entry.health_measured} />
            {entry.frames_per_swing != null && (
              <Field label="Frames / swing" value={`${entry.frames_per_swing}`} />
            )}
            {entry.luck_hits != null && (
              <Field label="Luck hits" value={`${entry.luck_hits}%`} />
            )}
            {entry.is_seasonal && <Field label="Seasonal" value="yes" />}
          </div>
        ) : (
          <p className="text-sm text-[var(--color-text-muted)]">
            No bestiary record for &quot;{kill.creature_name}&quot;.
          </p>
        )}

        <footer className="mt-4 flex items-center justify-between">
          <p className="text-xs text-[var(--color-text-muted)]">
            Killed {totalKills} times total
          </p>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-1 text-sm hover:bg-[var(--color-bg-hover)]"
          >
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}

function Field({ label, value, long }: { label: string; value: string; long?: boolean }) {
  return (
    <div className={long ? "md:col-span-2" : ""}>
      <span className="text-xs text-[var(--color-text-muted)]">{label}: </span>
      <span>{value}</span>
    </div>
  );
}

function Stat({
  label,
  value,
  measured,
}: {
  label: string;
  value: number | undefined;
  measured: boolean;
}) {
  if (value == null) return null;
  return (
    <div>
      <span className="text-xs text-[var(--color-text-muted)]">{label}: </span>
      <span>{value}</span>
      {measured && (
        <span className="ml-1 text-[10px] text-[var(--color-accent)]">&#10003; measured</span>
      )}
    </div>
  );
}

"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

import { apiFetch, apiError } from "@/lib/client-api";

const MOTIFS_UTILISATEUR = [
  ["arnaque_suspectee", "Arnaque suspectée"],
  ["comportement_inapproprie", "Comportement inapproprié"],
  ["contournement_plateforme", "Incite à troquer hors plateforme"],
  ["usurpation_faux_profil", "Usurpation ou faux profil"],
  ["autre", "Autre"],
] as const;

/**
 * Bloquer / signaler un troqueur depuis son profil (F5.2). Le blocage est
 * discret : l'autre n'est jamais prévenu.
 */
export function ProfileActions({
  pseudo,
  userId,
  initiallyBlocked,
}: {
  pseudo: string;
  userId: string;
  initiallyBlocked: boolean;
}) {
  const router = useRouter();
  const [blocked, setBlocked] = useState(initiallyBlocked);
  const [reporting, setReporting] = useState(false);
  const [reason, setReason] = useState("");
  const [comment, setComment] = useState("");
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function toggleBlock() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/users/${encodeURIComponent(pseudo)}/block`, {
        method: blocked ? "DELETE" : "POST",
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setBlocked(!blocked);
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  async function submitReport() {
    if (busy || !reason) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch("/reports", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          target_type: "utilisateur",
          target_id: userId,
          reason,
          comment: comment.trim() || null,
        }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setReporting(false);
      setDone(true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-3 text-xs text-neutre-700">
        <button
          onClick={toggleBlock}
          disabled={busy}
          className="cursor-pointer underline hover:text-terracotta-700"
        >
          {blocked ? `Débloquer ${pseudo}` : `Bloquer ${pseudo}`}
        </button>
        {done ? (
          <span className="text-sauge-800">Signalement transmis — merci.</span>
        ) : (
          <button
            onClick={() => setReporting(!reporting)}
            className="cursor-pointer underline hover:text-terracotta-700"
          >
            Signaler ce profil
          </button>
        )}
      </div>
      {reporting ? (
        <div className="flex w-full max-w-md flex-col gap-2 rounded-3xl bg-creme p-4">
          <select
            aria-label="Motif du signalement"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2.5 text-sm"
          >
            <option value="">Choisis un motif…</option>
            {MOTIFS_UTILISATEUR.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
          <textarea
            aria-label="Précisions"
            placeholder={reason === "autre" ? "Dis-nous ce qui ne va pas (obligatoire)" : "Précisions (facultatif)"}
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            maxLength={1000}
            rows={2}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
          />
          <button
            onClick={submitReport}
            disabled={busy || !reason || (reason === "autre" && comment.trim().length === 0)}
            className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme disabled:opacity-50"
          >
            Envoyer le signalement
          </button>
        </div>
      ) : null}
      {error ? <p className="text-xs text-terracotta-800">{error}</p> : null}
    </div>
  );
}

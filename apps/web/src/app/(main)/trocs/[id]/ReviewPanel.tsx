"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";

/**
 * Évaluations (F5.1) : après la finalisation, chaque partie note l'autre.
 * Publication simultanée anti-représailles — la note reste sous embargo
 * tant que l'autre n'a pas noté (ou 14 jours).
 */
export function ReviewPanel({
  trade,
  otherPseudo,
}: {
  trade: TradeDetailResponse;
  otherPseudo: string;
}) {
  const router = useRouter();
  const [rating, setRating] = useState(0);
  const [comment, setComment] = useState("");
  const [reply, setReply] = useState("");
  const [replying, setReplying] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (trade.status !== "finalise") return null;
  const { mine, received } = trade.reviews;

  async function submit() {
    if (busy || rating === 0) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/trades/${trade.id}/review`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ rating, comment: comment.trim() || null }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  async function sendReply() {
    if (busy || !received || reply.trim().length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/reviews/${received.id}/reply`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ reply: reply.trim() }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setReplying(false);
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <h2 className="font-display text-xl">Et ce troc, alors ?</h2>

      {!mine ? (
        <div className="flex flex-col gap-2">
          <p className="text-sm text-neutre-700">
            Note ton échange avec {otherPseudo} — la réputation, c&apos;est la monnaie du troc.
          </p>
          <div className="flex items-center gap-1" role="radiogroup" aria-label="Note sur 5">
            {[1, 2, 3, 4, 5].map((n) => (
              <button
                key={n}
                role="radio"
                aria-checked={rating === n}
                aria-label={`${n} étoile${n > 1 ? "s" : ""}`}
                onClick={() => setRating(n)}
                className={`cursor-pointer text-3xl transition-transform hover:scale-110 ${
                  n <= rating ? "text-terracotta-500" : "text-neutre-300"
                }`}
              >
                ★
              </button>
            ))}
          </div>
          <textarea
            aria-label="Commentaire"
            placeholder="Un mot sur l'échange ? (facultatif)"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            maxLength={500}
            rows={3}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
          />
          <button
            onClick={submit}
            disabled={busy || rating === 0}
            className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Publier ma note
          </button>
        </div>
      ) : (
        <div className="flex flex-col gap-1 rounded-3xl bg-creme p-4">
          <p className="text-sm font-semibold">
            Ta note : <Stars rating={mine.rating} />
          </p>
          {mine.comment ? (
            <p className="text-sm text-neutre-700">« {mine.comment} »</p>
          ) : null}
          {!mine.published ? (
            <p className="text-xs text-neutre-700">
              Elle sera visible quand {otherPseudo} aura noté à son tour — ou sous 14 jours.
              Personne ne voit la note de l&apos;autre avant d&apos;avoir donné la sienne.
            </p>
          ) : null}
        </div>
      )}

      {received ? (
        <div className="flex flex-col gap-1 rounded-3xl bg-sauge-100 p-4">
          <p className="text-sm font-semibold text-sauge-800">
            La note de {otherPseudo} : <Stars rating={received.rating} />
          </p>
          {received.comment ? (
            <p className="text-sm text-sauge-800">« {received.comment} »</p>
          ) : null}
          {received.reply ? (
            <p className="text-xs text-sauge-800">Ta réponse : « {received.reply} »</p>
          ) : replying ? (
            <div className="flex flex-col gap-2">
              <textarea
                aria-label="Ta réponse publique"
                value={reply}
                onChange={(e) => setReply(e.target.value)}
                maxLength={500}
                rows={2}
                className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
              />
              <div className="flex items-center gap-2">
                <button
                  onClick={sendReply}
                  disabled={busy || reply.trim().length === 0}
                  className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme disabled:opacity-50"
                >
                  Répondre
                </button>
                <button
                  onClick={() => setReplying(false)}
                  className="text-sm text-neutre-700 hover:underline"
                >
                  Annuler
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setReplying(true)}
              className="w-fit text-xs text-sauge-800 underline hover:text-terracotta-700"
            >
              Répondre publiquement (une seule fois)
            </button>
          )}
        </div>
      ) : null}

      {error ? (
        <p className="rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}
    </section>
  );
}

function Stars({ rating }: { rating: number }) {
  return (
    <span aria-label={`${rating} sur 5`} className="text-terracotta-500">
      {"★".repeat(rating)}
      <span className="text-neutre-300">{"★".repeat(5 - rating)}</span>
    </span>
  );
}

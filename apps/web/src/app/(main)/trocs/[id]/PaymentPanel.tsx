"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";
import { useRealtime } from "@/lib/realtime";

/**
 * Règlement de la soulte (F4.2) : le troc est accepté mais attend la
 * préautorisation du payeur. Bêta fermée : PSP simulé, aucune vraie carte.
 */
export function PaymentPanel({ trade }: { trade: TradeDetailResponse }) {
  const router = useRouter();
  const payment = trade.payment;
  const [card, setCard] = useState("4970 0000 0000 0000");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Le bénéficiaire voit l'écran basculer dès que le payeur a réglé.
  useRealtime((event) => {
    if (event.type === "trade_updated" && event.trade_id === trade.id) {
      router.refresh();
    }
  });

  if (!payment) return null;

  const euros = Math.round(payment.amount_cents / 100);
  const deadline = new Date(payment.deadline);
  const deadlineLabel = deadline.toLocaleString("fr-FR", {
    day: "numeric",
    month: "long",
    hour: "2-digit",
    minute: "2-digit",
  });

  async function pay() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/trades/${trade.id}/pay`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ card_number: card }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        router.refresh();
        return;
      }
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  if (!payment.i_am_payer) {
    return (
      <section className="mt-4 flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">Troc accepté — soulte en cours de règlement</h2>
        <p className="text-sm text-neutre-700">
          L&apos;autre partie doit sécuriser la soulte de <strong>{euros} €</strong> avant le{" "}
          {deadlineLabel}. Les objets sont réservés en attendant ; tu recevras un e-mail dès
          que c&apos;est fait. Sans règlement dans les temps, le troc sera annulé
          automatiquement.
        </p>
      </section>
    );
  }

  return (
    <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-xl">Sécurise la soulte de {euros} €</h2>
        <p className="text-sm text-neutre-700">
          Le montant est simplement <strong>bloqué</strong> sur ta carte : il ne sera débité
          qu&apos;à la remise des objets, confirmée par vos deux codes. Si le troc est annulé,
          rien n&apos;est débité. À régler avant le {deadlineLabel}, sinon le troc sera annulé.
        </p>
      </div>

      <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
        <label htmlFor="card-number" className="text-sm font-semibold">
          Numéro de carte
        </label>
        <input
          id="card-number"
          inputMode="numeric"
          autoComplete="cc-number"
          value={card}
          onChange={(e) => setCard(e.target.value.replace(/[^\d ]/g, ""))}
          className="rounded-full border border-neutre-300 bg-creme px-4 py-2.5 font-display text-lg tracking-wider outline-none transition-colors focus:border-terracotta-500"
        />
        <p className="text-xs text-neutre-700">
          🧪 Paiement simulé pendant la bêta — aucune vraie carte n&apos;est débitée. Garde le
          numéro proposé pour un règlement réussi, ou termine-le par 0002 pour simuler un refus.
        </p>
        <button
          onClick={pay}
          disabled={busy}
          className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {busy ? "Préautorisation…" : `Bloquer ${euros} € sur ma carte`}
        </button>
      </div>

      {error ? (
        <p className="rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}
    </section>
  );
}

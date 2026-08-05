"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";
import { useRealtime } from "@/lib/realtime";

const euros = (cents: number) =>
  (cents / 100).toLocaleString("fr-FR", {
    minimumFractionDigits: cents % 100 === 0 ? 0 : 2,
    maximumFractionDigits: 2,
  });

/**
 * Règlement du troc (F4.2/F4.3) : soulte et/ou frais d'envoi, préautorisés
 * jusqu'à l'aboutissement. Bêta fermée : PSP simulé, aucune vraie carte.
 */
export function PaymentPanel({
  trade,
  shippingConfigured,
}: {
  trade: TradeDetailResponse;
  shippingConfigured?: boolean;
}) {
  const router = useRouter();
  const payment = trade.payment;
  const [card, setCard] = useState("4970 0000 0000 0000");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // L'écran bascule dès que l'autre partie agit (paiement, colis…).
  useRealtime((event) => {
    if (event.type === "trade_updated" && event.trade_id === trade.id) {
      router.refresh();
    }
  });

  if (!payment) return null;

  const isShipping = trade.delivery_mode === "envoi";
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

  // Main propre : le bénéficiaire attend simplement le payeur.
  if (!payment.i_am_payer) {
    return (
      <section className="mt-4 flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">Troc accepté — soulte en cours de règlement</h2>
        <p className="text-sm text-neutre-700">
          L&apos;autre partie doit sécuriser la soulte de{" "}
          <strong>{euros(payment.amount_cents)} €</strong> avant le {deadlineLabel}. Les objets
          sont réservés en attendant ; tu recevras un e-mail dès que c&apos;est fait. Sans
          règlement dans les temps, le troc sera annulé automatiquement.
        </p>
      </section>
    );
  }

  // Mode envoi : mon règlement est fait, j'attends celui de l'autre.
  if (payment.status === "sequestre") {
    return (
      <section className="mt-4 flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">✓ Ton règlement est sécurisé</h2>
        <p className="text-sm text-neutre-700">
          {euros(payment.amount_cents)} € sont bloqués sur ta carte — débités seulement quand
          l&apos;échange aura abouti.{" "}
          {trade.other_payment_status !== "sequestre"
            ? "On attend maintenant le règlement de l'autre partie pour lancer les étiquettes."
            : ""}
        </p>
      </section>
    );
  }

  const rows: Array<[string, number]> = isShipping
    ? [
        ["Transport de mon colis", payment.shipping_cents],
        ["Frais de service", payment.service_cents],
        ...(payment.cash_cents > 0 ? ([["Soulte", payment.cash_cents]] as Array<[string, number]>) : []),
      ]
    : [["Soulte", payment.cash_cents]];

  return (
    <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-xl">
          {isShipping
            ? `Règle tes frais d'envoi${payment.cash_cents > 0 ? " et ta soulte" : ""}`
            : `Sécurise la soulte de ${euros(payment.amount_cents)} €`}
        </h2>
        <p className="text-sm text-neutre-700">
          Le montant est simplement <strong>bloqué</strong> sur ta carte : il ne sera débité
          qu&apos;une fois l&apos;échange abouti. Si le troc est annulé, rien n&apos;est débité.
          À régler avant le {deadlineLabel}, sinon le troc sera annulé.
        </p>
      </div>

      {isShipping && !shippingConfigured ? (
        <p className="rounded-3xl bg-terracotta-100/60 px-4 py-2.5 text-sm text-terracotta-800">
          Choisis d&apos;abord le format de ton colis et ton point relais ci-dessus — ils fixent
          le montant.
        </p>
      ) : (
        <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
          <ul className="flex flex-col gap-0.5 text-sm text-neutre-700">
            {rows.map(([label, cents]) => (
              <li key={label} className="flex justify-between">
                <span>{label}</span>
                <span>{euros(cents)} €</span>
              </li>
            ))}
            <li className="mt-1 flex justify-between border-t border-neutre-300/60 pt-1 font-semibold text-encre">
              <span>Total à bloquer</span>
              <span>{euros(payment.amount_cents)} €</span>
            </li>
          </ul>
          <label htmlFor="card-number" className="mt-2 text-sm font-semibold">
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
            numéro proposé pour un règlement réussi, ou termine-le par 0002 pour simuler un
            refus.
          </p>
          <button
            onClick={pay}
            disabled={busy}
            className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy ? "Préautorisation…" : `Bloquer ${euros(payment.amount_cents)} € sur ma carte`}
          </button>
        </div>
      )}

      {error ? (
        <p className="rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}
    </section>
  );
}

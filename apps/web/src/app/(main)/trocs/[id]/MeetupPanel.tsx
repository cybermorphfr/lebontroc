"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import QRCode from "qrcode";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";

/**
 * L'écran de rendez-vous (F4.1) : mon code à montrer (QR + 6 chiffres),
 * saisie du code de l'autre, double confirmation → troc finalisé.
 */
export function MeetupPanel({ trade }: { trade: TradeDetailResponse }) {
  const router = useRouter();
  const [qr, setQr] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cancelStep, setCancelStep] = useState(false);

  useEffect(() => {
    if (!trade.my_code) return;
    QRCode.toDataURL(trade.my_code, { margin: 1, width: 200, color: { dark: "#201e1d", light: "#f5ead8" } })
      .then(setQr)
      .catch(() => setQr(null));
  }, [trade.my_code]);

  async function confirm() {
    if (busy || code.trim().length < 6) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/trades/${trade.id}/confirm`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: code.trim() }),
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

  async function cancel() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/trades/${trade.id}/cancel`, { method: "POST" });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setCancelStep(false);
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  if (trade.status === "finalise") {
    return (
      <section className="mt-4 flex flex-col items-start gap-2 rounded-[32px] bg-sauge-100 p-6">
        <h2 className="font-display text-xl text-sauge-800">🎉 Troc finalisé !</h2>
        <p className="text-sm text-sauge-800">
          Les objets ont changé de mains — ils sortent des dressings. Merci d&apos;avoir troqué
          plutôt qu&apos;acheté.
        </p>
      </section>
    );
  }
  if (trade.status === "annule") {
    return (
      <section className="mt-4 flex flex-col items-start gap-2 rounded-[32px] bg-sable p-6">
        <h2 className="font-display text-xl">Troc annulé</h2>
        <p className="text-sm text-neutre-700">
          Les objets sont de nouveau disponibles. Rien de grave — le fil regorge d&apos;autres
          trouvailles.
        </p>
      </section>
    );
  }

  return (
    <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-xl">Organisez la remise</h2>
        <p className="text-sm text-neutre-700">
          Convenez d&apos;un rendez-vous dans la conversation — privilégiez un lieu public
          (gare, café, hall de mairie). Sans double confirmation sous 14 jours, le troc sera
          annulé automatiquement.
        </p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div className="flex flex-col items-center gap-2 rounded-3xl bg-creme p-4">
          <h3 className="text-sm font-semibold">Ton code à montrer</h3>
          {qr ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={qr} alt={`QR code ${trade.my_code ?? ""}`} className="size-40 rounded-xl" />
          ) : null}
          <p data-testid="mon-code" className="font-display text-3xl tracking-[0.3em]">{trade.my_code}</p>
          {trade.other_confirmed ? (
            <p className="text-xs text-sauge-800">✓ L&apos;autre partie a saisi ton code</p>
          ) : (
            <p className="text-xs text-neutre-700">En attente de l&apos;autre partie</p>
          )}
        </div>

        <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
          <h3 className="text-sm font-semibold">Le code de l&apos;autre</h3>
          {trade.i_confirmed ? (
            <p className="text-sm text-sauge-800">✓ Tu as confirmé la remise</p>
          ) : (
            <>
              <p className="text-xs text-neutre-700">
                Au rendez-vous, saisis les 6 chiffres affichés sur son écran.
              </p>
              <input
                aria-label="Code de l'autre partie"
                inputMode="numeric"
                maxLength={6}
                placeholder="000000"
                value={code}
                onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
                className="rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-center font-display text-xl tracking-[0.3em] outline-none transition-colors focus:border-terracotta-500"
              />
              <button
                onClick={confirm}
                disabled={busy || code.length < 6}
                className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Confirmer la remise
              </button>
            </>
          )}
        </div>
      </div>

      {error ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}

      <div className="flex flex-col gap-2 border-t border-neutre-300/60 pt-3">
        {trade.cancel_requested_by_other ? (
          <>
            <p className="text-sm text-terracotta-800">
              L&apos;autre partie souhaite annuler le troc. Si tu es d&apos;accord, les objets
              redeviendront disponibles.
            </p>
            <button
              onClick={cancel}
              disabled={busy}
              className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-5 font-display text-sm text-creme disabled:opacity-60"
            >
              Accepter l&apos;annulation
            </button>
          </>
        ) : trade.cancel_requested_by_me ? (
          <p className="text-sm text-neutre-700">
            Ta demande d&apos;annulation attend l&apos;accord de l&apos;autre partie.
          </p>
        ) : cancelStep ? (
          <div className="flex items-center gap-2">
            <button
              onClick={cancel}
              disabled={busy}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-5 font-display text-sm text-creme disabled:opacity-60"
            >
              Oui, demander l&apos;annulation
            </button>
            <button
              onClick={() => setCancelStep(false)}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full px-4 text-sm text-neutre-700 hover:bg-encre/5"
            >
              Non, on continue
            </button>
          </div>
        ) : (
          <button
            onClick={() => setCancelStep(true)}
            className="w-fit text-xs text-neutre-700 underline hover:text-terracotta-700"
          >
            Annuler le troc d&apos;un commun accord
          </button>
        )}
      </div>
    </section>
  );
}

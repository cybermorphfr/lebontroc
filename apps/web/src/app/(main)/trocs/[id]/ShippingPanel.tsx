"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";

type Shipment = TradeDetailResponse["shipments"][number];
type Relay = { code: string; name: string; address: string };

const FORMATS = [
  { value: "s", label: "S — jusqu'à 1 kg", hint: "boîte à chaussures", price: "4,50 €" },
  { value: "m", label: "M — jusqu'à 3 kg", hint: "carton 40 cm", price: "6,90 €" },
  { value: "l", label: "L — jusqu'à 10 kg", hint: "gros carton", price: "9,90 €" },
];

const STATUT_COLIS: Record<string, string> = {
  preparation: "En préparation",
  etiquette: "Étiquette prête — à déposer",
  depose: "Déposé",
  transit: "En transit",
  arrive: "Arrivé au point relais",
  retire: "Retiré",
  confirme: "Confirmé ✓",
  incident: "Problème signalé",
  annule: "Annulé",
};

/**
 * L'envoi croisé (F4.3) : configuration format + relais, étiquette avec
 * code de dépôt, suivi des deux colis, confirmation ou signalement.
 */
export function ShippingPanel({ trade }: { trade: TradeDetailResponse }) {
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mine = trade.shipments.find((s) => s.i_am_sender);
  const incoming = trade.shipments.find((s) => !s.i_am_sender);
  const configured = Boolean(mine?.format && incoming?.relay_code);
  const configurable =
    trade.status === "attente_paiement" &&
    trade.payment?.i_am_payer &&
    ["en_attente", "echoue"].includes(trade.payment.status);

  async function post(path: string, body?: unknown) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(path, {
        method: "POST",
        ...(body !== undefined
          ? { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }
          : {}),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
      }
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  if (!mine || !incoming) return null;

  return (
    <>
      {trade.status === "attente_paiement" && (configurable || configured) ? (
        <SetupCard
          trade={trade}
          mine={mine}
          incoming={incoming}
          editable={Boolean(configurable)}
          busy={busy}
          onSubmit={(format, relayCode) =>
            post(`/trades/${trade.id}/shipping`, { format, relay_code: relayCode })
          }
        />
      ) : null}

      {trade.status === "accepte" ? (
        <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
          <div className="flex flex-col gap-1">
            <h2 className="font-display text-xl">Vos colis</h2>
            <p className="text-sm text-neutre-700">
              Chacun dépose son colis en point relais sous 5 jours (rappels à J+2 et J+4).
              Le troc se finalise quand les deux colis sont reçus et confirmés — ou 72 h
              après leur retrait sans signalement.
            </p>
          </div>

          <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
            <h3 className="text-sm font-semibold">📦 Ton colis à envoyer</h3>
            <p className="text-sm text-neutre-700">
              {STATUT_COLIS[mine.status] ?? mine.status}
              {mine.relay_name ? ` — vers « ${mine.relay_name} »` : ""}
            </p>
            {mine.status === "etiquette" ? (
              <>
                <p className="text-sm text-neutre-700">
                  Présente ce code de dépôt au comptoir de n&apos;importe quel point relais
                  (simulation bêta) :
                </p>
                <p data-testid="code-depot" className="font-display text-2xl tracking-[0.2em]">
                  {mine.drop_code}
                </p>
                <button
                  onClick={() => post(`/shipments/${mine.id}/drop`)}
                  disabled={busy}
                  className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:opacity-50"
                >
                  J&apos;ai déposé mon colis
                </button>
              </>
            ) : null}
          </div>

          <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
            <h3 className="text-sm font-semibold">🎁 Le colis que tu reçois</h3>
            <p className="text-sm text-neutre-700">
              {STATUT_COLIS[incoming.status] ?? incoming.status}
              {incoming.relay_name ? ` — au relais « ${incoming.relay_name} »` : ""}
            </p>
            {incoming.status === "arrive" ? (
              <button
                onClick={() => post(`/shipments/${incoming.id}/pickup`)}
                disabled={busy}
                className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:opacity-50"
              >
                Je l&apos;ai récupéré
              </button>
            ) : null}
            {incoming.status === "retire" ? (
              <ConfirmOrReport
                shipment={incoming}
                busy={busy}
                onConfirm={() => post(`/shipments/${incoming.id}/confirm`)}
                onReport={(reason) => post(`/shipments/${incoming.id}/report`, { reason })}
              />
            ) : null}
            {incoming.status === "confirme" ? (
              <p className="text-xs text-sauge-800">✓ Tu as confirmé la réception</p>
            ) : null}
          </div>

          {error ? (
            <p className="rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
              {error}
            </p>
          ) : null}
        </section>
      ) : null}

      {trade.status === "litige_gele" ? (
        <section className="mt-4 flex flex-col items-start gap-2 rounded-[32px] bg-terracotta-100/60 p-6">
          <h2 className="font-display text-xl text-terracotta-800">Troc gelé — on regarde</h2>
          <p className="text-sm text-terracotta-800">
            Quelque chose s&apos;est mal passé (colis manquant ou problème signalé). L&apos;équipe
            examine la situation et revient vers vous — les paiements restent en sécurité en
            attendant.
          </p>
        </section>
      ) : null}
    </>
  );
}

function SetupCard({
  trade,
  mine,
  incoming,
  editable,
  busy,
  onSubmit,
}: {
  trade: TradeDetailResponse;
  mine: Shipment;
  incoming: Shipment;
  editable: boolean;
  busy: boolean;
  onSubmit: (format: string, relayCode: string) => void;
}) {
  const [format, setFormat] = useState(mine.format ?? "");
  const [relays, setRelays] = useState<Relay[]>([]);
  const [relayCode, setRelayCode] = useState(incoming.relay_code ?? "");

  useEffect(() => {
    if (!editable) return;
    apiFetch(`/trades/${trade.id}/relays`)
      .then((r) => (r.ok ? r.json() : []))
      .then(setRelays)
      .catch(() => setRelays([]));
  }, [trade.id, editable]);

  if (!editable) {
    return (
      <section className="mt-4 flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">Ton envoi est prêt</h2>
        <p className="text-sm text-neutre-700">
          Colis {mine.format?.toUpperCase()} — tu recevras le sien au relais «{" "}
          {incoming.relay_name} ». Les étiquettes partiront quand les deux règlements seront
          sécurisés.
        </p>
      </section>
    );
  }

  return (
    <section className="mt-4 flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-xl">Prépare ton envoi</h2>
        <p className="text-sm text-neutre-700">
          Choisis le format de ton colis (il fixe ton transport) et le point relais où tu
          récupéreras celui de l&apos;autre partie.
        </p>
      </div>

      <fieldset className="flex flex-col gap-2">
        <legend className="mb-1 text-sm font-semibold">Format de ton colis</legend>
        {FORMATS.map((f) => (
          <label
            key={f.value}
            className={`flex cursor-pointer items-center justify-between rounded-3xl border px-4 py-2.5 text-sm transition-colors ${
              format === f.value
                ? "border-terracotta-500 bg-terracotta-100/60"
                : "border-neutre-300 bg-creme"
            }`}
          >
            <span className="flex items-center gap-2">
              <input
                type="radio"
                name="format"
                value={f.value}
                checked={format === f.value}
                onChange={() => setFormat(f.value)}
                className="accent-[#c67139]"
              />
              <span>
                {f.label} <span className="text-xs text-neutre-700">({f.hint})</span>
              </span>
            </span>
            <span className="font-display">{f.price}</span>
          </label>
        ))}
      </fieldset>

      <div className="flex flex-col gap-1">
        <label htmlFor="relay" className="text-sm font-semibold">
          Ton point relais de réception
        </label>
        <select
          id="relay"
          value={relayCode}
          onChange={(e) => setRelayCode(e.target.value)}
          className="rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-sm outline-none focus:border-terracotta-500"
        >
          <option value="">Choisis un relais…</option>
          {relays.map((relay) => (
            <option key={relay.code} value={relay.code}>
              {relay.name} — {relay.address}
            </option>
          ))}
        </select>
      </div>

      <button
        onClick={() => onSubmit(format, relayCode)}
        disabled={busy || !format || !relayCode}
        className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        Valider mon envoi
      </button>
    </section>
  );
}

function ConfirmOrReport({
  shipment,
  busy,
  onConfirm,
  onReport,
}: {
  shipment: Shipment;
  busy: boolean;
  onConfirm: () => void;
  onReport: (reason: string) => void;
}) {
  const [reporting, setReporting] = useState(false);
  const [reason, setReason] = useState("");
  const deadline = shipment.confirmation_deadline
    ? new Date(shipment.confirmation_deadline).toLocaleString("fr-FR", {
        day: "numeric",
        month: "long",
        hour: "2-digit",
        minute: "2-digit",
      })
    : null;

  if (reporting) {
    return (
      <div className="flex flex-col gap-2">
        <label htmlFor="issue" className="text-xs text-neutre-700">
          Décris le problème (objet abîmé, non conforme, manquant…)
        </label>
        <textarea
          id="issue"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          maxLength={500}
          rows={3}
          className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
        />
        <div className="flex items-center gap-2">
          <button
            onClick={() => onReport(reason)}
            disabled={busy || reason.trim().length === 0}
            className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-5 font-display text-sm text-creme disabled:opacity-60"
          >
            Envoyer le signalement
          </button>
          <button
            onClick={() => setReporting(false)}
            className="flex min-h-11 cursor-pointer items-center justify-center rounded-full px-4 text-sm text-neutre-700 hover:bg-encre/5"
          >
            Annuler
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {deadline ? (
        <p className="text-xs text-neutre-700">
          Sans nouvelle de ta part avant le {deadline}, la réception sera confirmée
          automatiquement.
        </p>
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={onConfirm}
          disabled={busy}
          className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:opacity-50"
        >
          Tout est OK ✓
        </button>
        <button
          onClick={() => setReporting(true)}
          className="text-xs text-neutre-700 underline hover:text-terracotta-700"
        >
          Signaler un problème
        </button>
      </div>
    </div>
  );
}

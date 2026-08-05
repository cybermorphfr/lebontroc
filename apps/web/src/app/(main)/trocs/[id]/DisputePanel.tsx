"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import type { TradeDetailResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";

const MOTIFS_ENVOI = [
  ["non_conforme", "L'objet ne correspond pas à l'annonce"],
  ["abime", "L'objet est arrivé abîmé"],
  ["manquant", "Il manque l'objet ou un accessoire"],
  ["contrefacon", "C'est une contrefaçon"],
] as const;

const MOTIFS_MAIN_PROPRE_APRES = [
  ["non_conforme", "L'objet ne correspond pas à ce qui était convenu"],
  ["abime", "Un défaut caché est apparu"],
  ["contrefacon", "C'est une contrefaçon"],
] as const;

const STATUT_DOSSIER: Record<string, string> = {
  ouvert: "Dossier ouvert — en attente de la version de l'autre partie (72 h)",
  en_examen: "Dossier en cours d'examen par l'équipe (réponse sous 7 jours)",
  tranche: "Dossier tranché",
};

const ISSUE_DOSSIER: Record<string, string> = {
  capture: "le troc est validé, les règlements bloqués sont débités.",
  liberation: "les règlements bloqués sont annulés — rien n'est débité.",
  rejet: "classé sans suite, le troc reprend son cours.",
};

/**
 * Dossier de litige (F5.2) : ouverture avec pièces, contradictoire 72 h,
 * suivi. Fenêtres : envoi dès l'arrivée du colis et avant confirmation ;
 * main propre à J+3 (no-show) ou sous 48 h après la remise (vice).
 */
export function DisputePanel({
  trade,
  otherPseudo,
}: {
  trade: TradeDetailResponse;
  otherPseudo: string;
}) {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState("");
  const [description, setDescription] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const [responseText, setResponseText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const dispute = trade.dispute;

  // Fenêtres d'ouverture, calquées sur le domaine serveur.
  const windows = useMemo(() => {
    if (dispute) return null;
    const now = Date.now();
    if (trade.delivery_mode === "envoi" && trade.status === "accepte") {
      const receivable = trade.shipments.some(
        (s) => !s.i_am_sender && (s.status === "arrive" || s.status === "retire"),
      );
      return receivable ? { motifs: MOTIFS_ENVOI, noShow: false } : null;
    }
    if (trade.delivery_mode === "main_propre" && trade.status === "accepte") {
      const j3 = new Date(trade.accepted_at).getTime() + 3 * 86_400_000;
      return now >= j3 ? { motifs: [] as never[], noShow: true } : null;
    }
    if (trade.delivery_mode === "main_propre" && trade.status === "finalise" && trade.finalized_at) {
      const limite = new Date(trade.finalized_at).getTime() + 48 * 3_600_000;
      return now < limite ? { motifs: MOTIFS_MAIN_PROPRE_APRES, noShow: false } : null;
    }
    return null;
  }, [dispute, trade]);

  async function uploadPieces(): Promise<string[]> {
    if (files.length === 0) return [];
    const presign = await apiFetch(`/trades/${trade.id}/dispute/presign`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        files: files.map((f) => ({ content_type: f.type, size: f.size })),
      }),
    });
    if (!presign.ok) throw new Error((await apiError(presign)).message);
    const targets = (await presign.json()) as { photo_id: string; upload_url: string }[];
    const keys: string[] = [];
    for (const [index, target] of targets.entries()) {
      const file = files[index];
      const put = await fetch(target.upload_url, {
        method: "PUT",
        headers: { "Content-Type": file.type },
        body: file,
      });
      if (!put.ok) throw new Error("L'envoi d'une photo a échoué. Réessaie.");
      keys.push(
        `litiges/${trade.id}/${target.photo_id}.${file.type === "image/webp" ? "webp" : "jpg"}`,
      );
    }
    return keys;
  }

  async function submitDispute(motif: string) {
    if (busy || !motif || description.trim().length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const photo_keys = await uploadPieces();
      const response = await apiFetch(`/trades/${trade.id}/dispute`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ reason: motif, description: description.trim(), photo_keys }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setOpen(false);
      router.refresh();
    } catch (thrown) {
      setError(thrown instanceof Error ? thrown.message : "Une erreur est survenue.");
    } finally {
      setBusy(false);
    }
  }

  async function submitResponse() {
    if (busy || !dispute || responseText.trim().length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const photo_keys = await uploadPieces();
      const response = await apiFetch(`/disputes/${dispute.id}/respond`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ response: responseText.trim(), photo_keys }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      router.refresh();
    } catch (thrown) {
      setError(thrown instanceof Error ? thrown.message : "Une erreur est survenue.");
    } finally {
      setBusy(false);
    }
  }

  // ——— Rendu ———

  if (dispute) {
    return (
      <section id="litige" className="mt-4 flex flex-col gap-3 rounded-[32px] bg-terracotta-100 p-6 shadow-sm">
        <h2 className="font-display text-xl">Dossier de litige</h2>
        <p className="text-sm font-semibold text-terracotta-800">
          {STATUT_DOSSIER[dispute.status] ?? dispute.status}
          {dispute.status === "tranche" && dispute.outcome
            ? ` — ${ISSUE_DOSSIER[dispute.outcome] ?? dispute.outcome}`
            : null}
        </p>
        <p className="text-sm text-neutre-700">
          {dispute.opened_by_me ? "Ouvert par toi" : `Ouvert par ${otherPseudo}`} ·{" "}
          motif « {dispute.reason} » : « {dispute.description} »
        </p>
        {dispute.response ? (
          <p className="text-sm text-neutre-700">Réponse : « {dispute.response} »</p>
        ) : null}
        {dispute.can_respond ? (
          <div className="flex flex-col gap-2 rounded-3xl bg-creme p-4">
            <p className="text-sm font-semibold">
              Ta version compte : réponds sous 72 h, photos à l&apos;appui.
            </p>
            <textarea
              aria-label="Ta version"
              value={responseText}
              onChange={(e) => setResponseText(e.target.value)}
              maxLength={1000}
              rows={3}
              className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
            />
            <input
              type="file"
              aria-label="Tes photos"
              accept="image/jpeg,image/webp"
              multiple
              onChange={(e) => setFiles([...(e.target.files ?? [])].slice(0, 5))}
              className="text-sm"
            />
            <button
              onClick={submitResponse}
              disabled={busy || responseText.trim().length === 0}
              className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme disabled:opacity-50"
            >
              Verser ma version au dossier
            </button>
          </div>
        ) : null}
        {error ? <p className="text-sm text-terracotta-800">{error}</p> : null}
      </section>
    );
  }

  if (!windows) return null;

  return (
    <section id="litige" className="mt-4 flex flex-col gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
      {!open ? (
        <button
          onClick={() => {
            setOpen(true);
            if (windows.noShow) setReason("jamais_venu");
          }}
          className="w-fit cursor-pointer text-sm text-neutre-700 underline hover:text-terracotta-700"
        >
          {windows.noShow
            ? `${otherPseudo} n'est jamais venu ? Signale-le`
            : "Un problème avec cet échange ? Ouvre un dossier"}
        </button>
      ) : (
        <div className="flex flex-col gap-2">
          <h2 className="font-display text-xl">
            {windows.noShow ? "Rendez-vous manqué" : "Signaler un problème"}
          </h2>
          {!windows.noShow ? (
            <select
              aria-label="Motif"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2.5 text-sm"
            >
              <option value="">Choisis un motif…</option>
              {windows.motifs.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          ) : (
            <p className="text-sm text-neutre-700">
              Le troc sera suspendu et l&apos;équipe tranchera. {otherPseudo} pourra donner sa
              version pendant 72 h.
            </p>
          )}
          <textarea
            aria-label="Décris le problème"
            placeholder="Raconte ce qui s'est passé (obligatoire)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            maxLength={1000}
            rows={3}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
          />
          {!windows.noShow ? (
            <input
              type="file"
              aria-label="Photos du problème"
              accept="image/jpeg,image/webp"
              multiple
              onChange={(e) => setFiles([...(e.target.files ?? [])].slice(0, 5))}
              className="text-sm"
            />
          ) : null}
          <div className="flex items-center gap-3">
            <button
              onClick={() => submitDispute(windows.noShow ? "jamais_venu" : reason)}
              disabled={busy || (!windows.noShow && !reason) || description.trim().length === 0}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme disabled:cursor-not-allowed disabled:opacity-50"
            >
              Ouvrir le dossier
            </button>
            <button
              onClick={() => setOpen(false)}
              className="text-sm text-neutre-700 hover:underline"
            >
              Annuler
            </button>
          </div>
          {error ? <p className="text-sm text-terracotta-800">{error}</p> : null}
        </div>
      )}
    </section>
  );
}

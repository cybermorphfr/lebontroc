"use client";

import { useState } from "react";

import { apiFetch, apiError } from "@/lib/client-api";

/**
 * Signalement d'une annonce, d'un profil ou d'un message — un seul
 * composant pour les trois cibles que l'API accepte. Les motifs sont
 * ceux du domaine (crates/domain/src/dispute.rs) : les changer ici sans
 * les changer là-bas donne un 400 « motif inconnu ».
 */

export const MOTIFS = {
  objet: [
    ["annonce_trompeuse", "Annonce trompeuse"],
    ["contrefacon", "Contrefaçon"],
    ["interdit_vente", "Objet interdit à l'échange"],
    ["contenu_inapproprie", "Photos ou texte inappropriés"],
    ["spam_doublon", "Spam ou doublon"],
  ],
  utilisateur: [
    ["arnaque_suspectee", "Arnaque suspectée"],
    ["comportement_inapproprie", "Comportement inapproprié"],
    ["contournement_plateforme", "Incite à troquer hors plateforme"],
    ["usurpation_faux_profil", "Usurpation ou faux profil"],
    ["autre", "Autre"],
  ],
  message: [
    ["harcelement_insultes", "Harcèlement ou insultes"],
    ["tentative_arnaque", "Tentative d'arnaque"],
    ["contournement_masquage", "Contourne le masquage des coordonnées"],
    ["contenu_inapproprie", "Contenu inapproprié"],
  ],
} as const;

export type CibleSignalement = keyof typeof MOTIFS;

export function ReportButton({
  cible,
  targetId,
  libelle,
}: {
  cible: CibleSignalement;
  targetId: string;
  /** Texte du déclencheur, ex. « Signaler cette annonce ». */
  libelle: string;
}) {
  const [ouvert, setOuvert] = useState(false);
  const [motif, setMotif] = useState("");
  const [precisions, setPrecisions] = useState("");
  const [envoye, setEnvoye] = useState(false);
  const [occupe, setOccupe] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  async function envoyer() {
    if (occupe || !motif) return;
    setOccupe(true);
    setErreur(null);
    try {
      const response = await apiFetch("/reports", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          target_type: cible,
          target_id: targetId,
          reason: motif,
          comment: precisions.trim() || null,
        }),
      });
      if (!response.ok) {
        setErreur((await apiError(response)).message);
        return;
      }
      setOuvert(false);
      setEnvoye(true);
    } finally {
      setOccupe(false);
    }
  }

  if (envoye) {
    return (
      <p className="text-xs text-sauge-800">
        Signalement transmis — merci, l&apos;équipe le regarde.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <button
        onClick={() => setOuvert(!ouvert)}
        aria-expanded={ouvert}
        className="w-fit cursor-pointer text-xs text-neutre-700 underline hover:text-terracotta-700"
      >
        {libelle}
      </button>
      {ouvert ? (
        <div className="flex w-full max-w-md flex-col gap-2 rounded-3xl bg-creme p-4">
          <select
            aria-label="Motif du signalement"
            value={motif}
            onChange={(e) => setMotif(e.target.value)}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2.5 text-sm"
          >
            <option value="">Choisis un motif…</option>
            {MOTIFS[cible].map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
          <textarea
            aria-label="Précisions"
            placeholder={
              motif === "autre"
                ? "Dis-nous ce qui ne va pas (obligatoire)"
                : "Précisions (facultatif)"
            }
            value={precisions}
            onChange={(e) => setPrecisions(e.target.value)}
            maxLength={1000}
            rows={2}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none focus:border-terracotta-500"
          />
          <button
            onClick={envoyer}
            disabled={occupe || !motif || (motif === "autre" && precisions.trim().length === 0)}
            className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme disabled:cursor-not-allowed disabled:opacity-50"
          >
            Envoyer le signalement
          </button>
          {erreur ? <p className="text-xs text-terracotta-800">{erreur}</p> : null}
        </div>
      ) : null}
    </div>
  );
}

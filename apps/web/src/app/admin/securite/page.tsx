"use client";

import { useCallback, useEffect, useState } from "react";

import { apiFetch, apiError } from "@/lib/client-api";

// Sécurité du compte administrateur : la double authentification, de
// l'enrôlement (QR + confirmation) aux codes de secours.

type Statut = {
  enabled: boolean;
  pending: boolean;
  session_verified: boolean;
  recovery_left: number;
};

export default function AdminSecuritePage() {
  const [statut, setStatut] = useState<Statut | null>(null);
  const [enrolement, setEnrolement] = useState<{ secret: string; qr_svg: string } | null>(null);
  const [code, setCode] = useState("");
  const [secours, setSecours] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  const recharger = useCallback(() => {
    apiFetch("/me/totp")
      .then((r) => (r.ok ? r.json() : null))
      .then(setStatut)
      .catch(() => {});
  }, []);
  useEffect(recharger, [recharger]);

  async function demarrer() {
    setBusy(true);
    setErreur(null);
    try {
      const response = await apiFetch("/me/totp/start", { method: "POST" });
      if (!response.ok) {
        setErreur((await apiError(response)).message);
        return;
      }
      setEnrolement(await response.json());
    } finally {
      setBusy(false);
    }
  }

  async function confirmer() {
    if (busy || code.trim().length === 0) return;
    setBusy(true);
    setErreur(null);
    try {
      const response = await apiFetch("/me/totp/enable", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: code.trim() }),
      });
      if (!response.ok) {
        setErreur((await apiError(response)).message);
        return;
      }
      const { recovery_codes } = (await response.json()) as { recovery_codes: string[] };
      setSecours(recovery_codes);
      setEnrolement(null);
      setCode("");
      recharger();
    } finally {
      setBusy(false);
    }
  }

  async function desactiver() {
    if (busy || code.trim().length === 0) return;
    setBusy(true);
    setErreur(null);
    try {
      const response = await apiFetch("/me/totp/disable", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: code.trim() }),
      });
      if (!response.ok) {
        setErreur((await apiError(response)).message);
        return;
      }
      setCode("");
      setSecours(null);
      recharger();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <section className="flex flex-col gap-3 rounded-[28px] bg-sable p-5 shadow-sm">
        <h2 className="font-display text-lg">Double authentification</h2>

        {statut === null ? (
          <p className="text-sm text-neutre-700">Chargement…</p>
        ) : statut.enabled ? (
          <>
            <p className="rounded-2xl bg-sauge-100 px-4 py-2.5 text-sm text-sauge-800">
              ✓ Active — chaque nouvelle session devra présenter un code de ton application
              d&apos;authentification. Codes de secours restants : {statut.recovery_left}.
            </p>
            <div className="flex flex-wrap items-center gap-2">
              <input
                aria-label="Code de vérification"
                placeholder="Code à 6 chiffres"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                inputMode="numeric"
                className="w-40 rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500"
              />
              <button
                onClick={desactiver}
                disabled={busy || code.trim().length === 0}
                className="flex min-h-10 cursor-pointer items-center rounded-full border border-terracotta-500 px-4 text-sm text-terracotta-800 transition-colors hover:bg-terracotta-100 disabled:opacity-50"
              >
                Désactiver la 2FA
              </button>
            </div>
          </>
        ) : enrolement ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-neutre-700">
              1. Scanne ce QR code avec ton application (Google Authenticator, Aegis,
              1Password…) — ou saisis le secret à la main.
            </p>
            <div
              className="w-fit rounded-2xl bg-white p-3"
              // SVG généré par notre API — pas de contenu externe.
              dangerouslySetInnerHTML={{ __html: enrolement.qr_svg }}
            />
            <p className="break-all rounded-2xl bg-creme px-3 py-2 font-mono text-xs">
              {enrolement.secret}
            </p>
            <p className="text-sm text-neutre-700">2. Saisis le code affiché pour confirmer :</p>
            <div className="flex flex-wrap items-center gap-2">
              <input
                aria-label="Premier code de confirmation"
                placeholder="Code à 6 chiffres"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                inputMode="numeric"
                className="w-40 rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500"
              />
              <button
                onClick={confirmer}
                disabled={busy || code.trim().length === 0}
                className="flex min-h-10 cursor-pointer items-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme hover:bg-terracotta-600 disabled:opacity-50"
              >
                Activer
              </button>
            </div>
          </div>
        ) : (
          <>
            <p className="text-sm text-neutre-700">
              Un code à 6 chiffres depuis ton téléphone, exigé à chaque nouvelle session
              d&apos;administration. C&apos;est la protection principale du panneau.
            </p>
            <button
              onClick={demarrer}
              disabled={busy}
              className="flex min-h-10 w-fit cursor-pointer items-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme hover:bg-terracotta-600 disabled:opacity-50"
            >
              Activer la double authentification
            </button>
          </>
        )}

        {erreur ? (
          <p className="rounded-2xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
            {erreur}
          </p>
        ) : null}
      </section>

      {secours ? (
        <section className="flex flex-col gap-3 rounded-[28px] bg-terracotta-100 p-5 shadow-sm">
          <h2 className="font-display text-lg text-terracotta-800">
            Tes codes de secours — notés maintenant, montrés jamais plus
          </h2>
          <p className="text-sm text-terracotta-800">
            Chacun ne sert qu&apos;une fois, si tu perds ton téléphone. Range-les hors ligne.
          </p>
          <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-5">
            {secours.map((c) => (
              <span key={c} className="rounded-xl bg-creme px-2 py-1.5 text-center font-mono text-xs">
                {c}
              </span>
            ))}
          </div>
        </section>
      ) : null}

      <p className="text-xs text-neutre-700">
        Téléphone perdu et codes épuisés ? La récupération passe par le compte maître
        (réinitialisation de la 2FA depuis l&apos;écran Équipe).
      </p>
    </div>
  );
}

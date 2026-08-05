"use client";

import { useEffect, useState } from "react";

import { apiFetch } from "@/lib/client-api";

type Prefs = {
  proposition_recue: boolean;
  proposition_cloturee: boolean;
  message_recu: boolean;
  evaluation: boolean;
  favori: boolean;
};

const LIGNES: { key: keyof Prefs; label: string; hint: string }[] = [
  {
    key: "proposition_recue",
    label: "Propositions reçues",
    hint: "Nouvelle proposition ou contre-proposition",
  },
  {
    key: "proposition_cloturee",
    label: "Propositions clôturées",
    hint: "Refusée, expirée ou devenue caduque",
  },
  {
    key: "message_recu",
    label: "Rappel de messages non lus",
    hint: "Un e-mail au plus par conversation, après 24 h",
  },
  { key: "evaluation", label: "Évaluations", hint: "Quand vos notes sont publiées" },
  {
    key: "favori",
    label: "Favoris",
    hint: "Objet favori réservé ou de nouveau disponible",
  },
];

/** Préférences e-mail (F5.3) — l'in-app n'est pas débrayable. */
export default function ReglagesNotificationsPage() {
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    apiFetch("/me/preferences/notifications")
      .then((r) => (r.ok ? r.json() : null))
      .then(setPrefs)
      .catch(() => {});
  }, []);

  async function toggle(key: keyof Prefs) {
    if (!prefs) return;
    const next = { ...prefs, [key]: !prefs[key] };
    setPrefs(next);
    setSaved(false);
    const response = await apiFetch("/me/preferences/notifications", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(next),
    });
    if (response.ok) setSaved(true);
  }

  return (
    <main className="mx-auto flex w-full max-w-xl flex-col gap-4 px-6 pb-16">
      <h1 className="font-display text-2xl">Notifications par e-mail</h1>
      <p className="text-sm text-neutre-700">
        Tout reste visible dans ton centre de notifications ; ici tu choisis ce qui arrive aussi
        par e-mail.
      </p>

      <section className="flex flex-col gap-1 rounded-[32px] bg-sable p-6 shadow-sm">
        {prefs
          ? LIGNES.map(({ key, label, hint }) => (
              <label
                key={key}
                className="flex cursor-pointer items-center justify-between gap-4 rounded-2xl px-3 py-3 transition-colors hover:bg-creme"
              >
                <span className="flex flex-col">
                  <span className="text-sm font-semibold">{label}</span>
                  <span className="text-xs text-neutre-700">{hint}</span>
                </span>
                <input
                  type="checkbox"
                  role="switch"
                  checked={prefs[key]}
                  onChange={() => toggle(key)}
                  className="size-5 accent-[#c67139]"
                />
              </label>
            ))
          : null}
      </section>

      <p className="rounded-3xl bg-creme px-4 py-3 text-xs text-neutre-700">
        Paiements, colis, remises, litiges et sécurité du compte : toujours envoyés — c&apos;est la
        sécurité de tes trocs.
      </p>
      {saved ? <p className="text-xs text-sauge-800">✓ Préférences enregistrées</p> : null}
    </main>
  );
}

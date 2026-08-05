"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

import { apiFetch, apiError } from "@/lib/client-api";

/** RGPD (F6.3) : export de ses données + suppression de compte. */
export function DataPrivacy() {
  const router = useRouter();
  const [deleting, setDeleting] = useState(false);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function download() {
    const response = await apiFetch("/me/export");
    if (!response.ok) return;
    const blob = new Blob([JSON.stringify(await response.json(), null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "mes-donnees-lebontroc.json";
    a.click();
    URL.revokeObjectURL(url);
  }

  async function deleteAccount() {
    if (busy || password.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch("/me", {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      router.push("/");
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="flex flex-col gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
      <h2 className="font-display text-xl">Tes données</h2>
      <button
        onClick={download}
        className="w-fit cursor-pointer text-sm underline hover:text-terracotta-700"
      >
        Télécharger toutes mes données (JSON)
      </button>
      {!deleting ? (
        <button
          onClick={() => setDeleting(true)}
          className="w-fit cursor-pointer text-sm text-terracotta-800 underline"
        >
          Supprimer mon compte
        </button>
      ) : (
        <div className="flex flex-col gap-2 rounded-3xl bg-terracotta-100 p-4">
          <p className="text-sm text-terracotta-800">
            Ton profil et tes objets disparaîtront. Tes trocs finalisés avec soulte restent en
            base sous forme anonymisée (obligations comptables). C&apos;est définitif.
          </p>
          <input
            type="password"
            aria-label="Mot de passe"
            placeholder="Ton mot de passe pour confirmer"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="rounded-2xl border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none"
          />
          <div className="flex items-center gap-2">
            <button
              onClick={deleteAccount}
              disabled={busy || password.length === 0}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-5 font-display text-sm text-creme disabled:opacity-50"
            >
              Supprimer définitivement
            </button>
            <button onClick={() => setDeleting(false)} className="text-sm underline">
              Annuler
            </button>
          </div>
          {error ? <p className="text-sm text-terracotta-800">{error}</p> : null}
        </div>
      )}
    </section>
  );
}

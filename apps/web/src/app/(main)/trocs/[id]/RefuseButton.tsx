"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { apiFetch } from "@/lib/client-api";

/** Refus d'une proposition par le destinataire, avec confirmation légère. */
export function RefuseButton({ proposalId }: { proposalId: string }) {
  const router = useRouter();
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);

  async function refuse() {
    setBusy(true);
    try {
      const response = await apiFetch(`/proposals/${proposalId}/refuse`, { method: "POST" });
      if (response.ok) router.refresh();
    } finally {
      setBusy(false);
    }
  }

  if (!confirm) {
    return (
      <button
        onClick={() => setConfirm(true)}
        className="flex min-h-11 w-fit cursor-pointer items-center justify-center rounded-full border border-terracotta-500 px-6 text-sm text-terracotta-700 transition-colors hover:bg-terracotta-500/10"
      >
        Refuser la proposition
      </button>
    );
  }
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={refuse}
        disabled={busy}
        className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-6 font-display text-sm text-creme disabled:opacity-60"
      >
        Oui, je refuse
      </button>
      <button
        onClick={() => setConfirm(false)}
        className="flex min-h-11 cursor-pointer items-center justify-center rounded-full px-5 text-sm text-neutre-700 hover:bg-encre/5"
      >
        Annuler
      </button>
    </div>
  );
}

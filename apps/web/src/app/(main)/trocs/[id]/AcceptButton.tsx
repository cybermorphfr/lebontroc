"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { BottomSheet } from "@/components/ui/BottomSheet";
import { apiFetch, apiError } from "@/lib/client-api";

const MODES = [
  {
    value: "main_propre",
    label: "En main propre",
    hint: "Vous convenez d'un rendez-vous — lieu public recommandé.",
  },
  {
    value: "envoi",
    label: "Par envoi",
    hint: "Chacun expédie son colis (les détails arrivent bientôt).",
  },
];

/** Acceptation d'une proposition : choix du mode de remise puis transaction. */
export function AcceptButton({ proposalId }: { proposalId: string }) {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function accept(mode: string) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await apiFetch(`/proposals/${proposalId}/accept`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ delivery_mode: mode }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      setOpen(false);
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
      >
        Accepter
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)} title="Comment vous remettez-vous les objets ?">
        <div className="flex flex-col gap-3">
          <p className="text-sm text-neutre-700">
            En acceptant, tous les objets de l&apos;échange passent en « réservé » et les autres
            propositions qui les visaient deviennent caduques.
          </p>
          {MODES.map((mode) => (
            <button
              key={mode.value}
              onClick={() => accept(mode.value)}
              disabled={busy}
              className="flex flex-col items-start gap-0.5 rounded-3xl bg-sable p-4 text-left transition-colors hover:bg-terracotta-100/60 disabled:opacity-60"
            >
              <span className="font-display text-base">{mode.label}</span>
              <span className="text-xs text-neutre-700">{mode.hint}</span>
            </button>
          ))}
          {error ? (
            <p className="rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
              {error}
            </p>
          ) : null}
        </div>
      </BottomSheet>
    </>
  );
}

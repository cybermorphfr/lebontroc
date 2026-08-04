"use client";

import { useEffect, useState } from "react";

import { apiFetch } from "@/lib/client-api";

/**
 * Lien « Renvoyer l'e-mail » avec cooldown de 60 s.
 * `asButton` : rendu bouton secondaire (écran /verification) au lieu du lien.
 */
export function ResendVerification({ asButton = false }: { asButton?: boolean }) {
  const [cooldown, setCooldown] = useState(0);
  const [sent, setSent] = useState(false);

  useEffect(() => {
    if (cooldown <= 0) return;
    const timer = setTimeout(() => setCooldown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [cooldown]);

  async function resend() {
    const response = await apiFetch("/auth/resend-verification", { method: "POST" });
    if (response.ok || response.status === 429) {
      setSent(true);
      setCooldown(60);
    }
  }

  if (sent && cooldown > 0) {
    return (
      <span className={asButton ? "text-sm text-sauge-700" : "text-terracotta-800"}>
        C&apos;est renvoyé&nbsp;! Regarde ta boîte dans une minute.
      </span>
    );
  }

  if (asButton) {
    return (
      <button
        onClick={resend}
        className="inline-flex cursor-pointer items-center justify-center rounded-full border border-neutre-300 px-4 py-2 font-display text-sm text-encre transition-colors hover:bg-encre/7"
      >
        Renvoyer l&apos;e-mail
      </button>
    );
  }

  return (
    <button onClick={resend} className="cursor-pointer font-semibold underline">
      Renvoyer l&apos;e-mail
    </button>
  );
}

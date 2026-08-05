"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

/**
 * Bannière d'information cookies/mesure d'audience (F6.3). Lebontroc
 * n'utilise que des cookies de session et une mesure d'audience
 * pseudonymisée côté serveur — pas de traceur publicitaire.
 */
export function ConsentBanner() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    try {
      setVisible(localStorage.getItem("lbt_consent") === null);
    } catch {
      // localStorage indisponible : pas de bannière.
    }
  }, []);

  if (!visible) return null;

  function accept() {
    try {
      localStorage.setItem("lbt_consent", "ok");
    } catch {
      // ignore
    }
    setVisible(false);
  }

  return (
    <div className="fixed inset-x-4 bottom-4 z-50 mx-auto flex max-w-xl flex-wrap items-center justify-between gap-3 rounded-3xl bg-encre p-4 text-creme shadow-lg">
      <p className="text-xs">
        🍪 Ici, pas de pub ni de pistage : des cookies de connexion et une mesure d&apos;audience
        anonymisée, c&apos;est tout.{" "}
        <Link href="/confidentialite" className="underline">
          En savoir plus
        </Link>
      </p>
      <button
        onClick={accept}
        className="cursor-pointer rounded-full bg-creme px-4 py-1.5 font-display text-xs text-encre"
      >
        Compris
      </button>
    </div>
  );
}

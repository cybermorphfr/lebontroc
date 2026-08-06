"use client";

import { useEffect, useRef } from "react";
import { useRouter } from "next/navigation";

import { useRealtime } from "@/lib/realtime";

/**
 * La page du troc est rendue côté serveur : sans cela, un changement
 * d'état (acceptation, contre-proposition, refus, paiement, colis) ne
 * serait visible qu'après rechargement manuel. Ce composant réagit aux
 * événements temps réel qui concernent CE troc et rafraîchit la page —
 * ou suit le fil quand une contre-proposition l'a déplacé.
 */
export function RealtimeRefresher({
  proposalId,
  tradeId,
}: {
  proposalId: string;
  tradeId?: string | null;
}) {
  const router = useRouter();
  const dernier = useRef(0);

  useRealtime((event) => {
    if (event.type !== "trade_updated") return;
    const concerne =
      ("proposal_id" in event && event.proposal_id === proposalId) ||
      ("trade_id" in event && tradeId != null && event.trade_id === tradeId);
    if (!concerne) return;

    // Le fil a déménagé vers la contre-proposition : on suit.
    const suivante = (event as { new_proposal_id?: string }).new_proposal_id;
    if (suivante) {
      router.replace(`/trocs/${suivante}`);
      return;
    }
    // Anti-rafale : au plus un rafraîchissement par seconde.
    const maintenant = Date.now();
    if (maintenant - dernier.current < 1000) return;
    dernier.current = maintenant;
    router.refresh();
  });

  // Filet : au retour sur l'onglet, on resynchronise.
  useEffect(() => {
    function onVisible() {
      if (document.visibilityState === "visible") router.refresh();
    }
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  }, [router]);

  return null;
}

"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";

/**
 * Au chargement, si l'access token (15 min) a expiré mais que le refresh est
 * encore valable, rafraîchit la session puis re-rend le SSR.
 * Le cookie refresh est confiné à /api/auth : seul le navigateur peut le jouer.
 */
export function SessionKeeper({ loggedOut }: { loggedOut: boolean }) {
  const router = useRouter();

  useEffect(() => {
    if (!loggedOut) return;
    let cancelled = false;
    fetch("/api/auth/refresh", { method: "POST" }).then((response) => {
      if (!cancelled && response.ok) router.refresh();
    });
    return () => {
      cancelled = true;
    };
  }, [loggedOut, router]);

  return null;
}

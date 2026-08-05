"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

import { apiFetch } from "@/lib/client-api";

/**
 * Cœur favori en overlay de carte (pattern Vinted). Toggle optimiste ;
 * un visiteur est envoyé vers l'inscription.
 */
export function FavoriteHeart({
  itemId,
  initial,
  loggedIn,
}: {
  itemId: string;
  initial: boolean;
  loggedIn: boolean;
}) {
  const router = useRouter();
  const [fav, setFav] = useState(initial);

  async function toggle(event: React.MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (!loggedIn) {
      router.push("/inscription");
      return;
    }
    const next = !fav;
    setFav(next);
    const response = await apiFetch(`/items/${itemId}/favorite`, {
      method: next ? "PUT" : "DELETE",
    });
    if (!response.ok) setFav(!next);
  }

  return (
    <button
      onClick={toggle}
      aria-label={fav ? "Retirer des favoris" : "Ajouter aux favoris"}
      aria-pressed={fav}
      className="absolute right-2 top-2 flex size-8 cursor-pointer items-center justify-center rounded-full bg-creme/90 shadow-sm transition-transform hover:scale-110"
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill={fav ? "#c67139" : "none"}
        stroke={fav ? "#c67139" : "#201e1d"}
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden
      >
        <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" />
      </svg>
    </button>
  );
}

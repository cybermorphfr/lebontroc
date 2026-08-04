"use client";

import Link from "next/link";
import { useState } from "react";

import { apiFetch } from "@/lib/client-api";

/** Cœur de la fiche objet : pose/retire un favori, compteur optimiste. */
export function FavoriteButton({
  itemId,
  initialCount,
  initialFavorited,
  loggedIn,
}: {
  itemId: string;
  initialCount: number;
  initialFavorited: boolean;
  loggedIn: boolean;
}) {
  const [favorited, setFavorited] = useState(initialFavorited);
  const [count, setCount] = useState(initialCount);
  const [busy, setBusy] = useState(false);

  if (!loggedIn) {
    return (
      <Link
        href="/connexion"
        className="inline-flex items-center gap-1.5 rounded-full border border-neutre-300 px-4 py-2 text-sm text-neutre-700 transition-colors hover:bg-encre/7"
      >
        <HeartIcon filled={false} />
        {count > 0 ? count : "Favori"}
      </Link>
    );
  }

  async function toggle() {
    if (busy) return;
    setBusy(true);
    const next = !favorited;
    setFavorited(next);
    setCount((c) => c + (next ? 1 : -1));
    try {
      const response = await apiFetch(`/items/${itemId}/favorite`, {
        method: next ? "PUT" : "DELETE",
      });
      if (!response.ok) {
        setFavorited(!next);
        setCount((c) => c + (next ? -1 : 1));
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      onClick={toggle}
      aria-pressed={favorited}
      aria-label={favorited ? "Retirer des favoris" : "Ajouter aux favoris"}
      className={`inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-4 py-2 text-sm transition-colors ${
        favorited
          ? "border-terracotta-500 bg-terracotta-100/60 text-terracotta-800"
          : "border-neutre-300 text-neutre-700 hover:bg-encre/7"
      }`}
    >
      <HeartIcon filled={favorited} />
      {count > 0 ? count : "Favori"}
    </button>
  );
}

function HeartIcon({ filled }: { filled: boolean }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth="2.25"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" />
    </svg>
  );
}

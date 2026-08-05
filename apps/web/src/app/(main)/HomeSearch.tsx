"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

/** Barre de recherche proéminente de la home (pattern Leboncoin). */
export function HomeSearch() {
  const router = useRouter();
  const [q, setQ] = useState("");

  function submit(event: React.FormEvent) {
    event.preventDefault();
    router.push(q.trim() ? `/recherche?q=${encodeURIComponent(q.trim())}` : "/recherche");
  }

  return (
    <form onSubmit={submit} className="flex w-full items-center gap-2">
      <div className="relative flex-1">
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
          className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-neutre-700"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        <input
          type="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Une poussette, une perceuse, un vélo…"
          aria-label="Rechercher un objet à troquer"
          className="h-12 w-full rounded-full border border-neutre-300 bg-creme pl-11 pr-4 text-sm outline-none transition-colors focus:border-terracotta-500"
        />
      </div>
      <button
        type="submit"
        className="flex h-12 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
      >
        Chercher
      </button>
    </form>
  );
}

"use client";

import { useState } from "react";
import type { WishlistEntry } from "@lebontroc/api-client";

import { apiFetch } from "@/lib/client-api";

type Row = { category_id: string; keywords: string };

function toRows(entries: WishlistEntry[]): Row[] {
  const rows = entries.map((e) => ({
    category_id: e.category_id != null ? String(e.category_id) : "",
    keywords: e.keywords,
  }));
  while (rows.length < 3) rows.push({ category_id: "", keywords: "" });
  return rows.slice(0, 3);
}

/** Les 3 lignes « ce que je cherche » — le carburant du futur matching. */
export function WishlistForm({
  initial,
  roots,
}: {
  initial: WishlistEntry[];
  roots: { id: number; label: string }[];
}) {
  const [rows, setRows] = useState<Row[]>(() => toRows(initial));
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function update(index: number, patch: Partial<Row>) {
    setRows((current) => current.map((row, i) => (i === index ? { ...row, ...patch } : row)));
    setSaved(false);
  }

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const entries = rows
        .filter((row) => row.category_id !== "" || row.keywords.trim() !== "")
        .map((row) => ({
          category_id: row.category_id === "" ? null : Number(row.category_id),
          keywords: row.keywords.trim(),
        }));
      const response = await apiFetch("/me/wishlist", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ entries }),
      });
      if (!response.ok) {
        setError("On n'a pas réussi à enregistrer. Réessaie dans un instant.");
        return;
      }
      setSaved(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-neutre-700">
        Dis-nous ce que tu cherches — ça nous aidera bientôt à te proposer les bons trocs.
      </p>
      {rows.map((row, index) => (
        <div key={index} className="flex flex-col gap-2 sm:flex-row">
          <select
            aria-label={`Catégorie de l'envie ${index + 1}`}
            value={row.category_id}
            onChange={(e) => update(index, { category_id: e.target.value })}
            className="rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-sm sm:w-56"
          >
            <option value="">Toute catégorie</option>
            {roots.map((root) => (
              <option key={root.id} value={root.id}>
                {root.label}
              </option>
            ))}
          </select>
          <input
            aria-label={`Mots-clés de l'envie ${index + 1}`}
            placeholder="Poussette yoyo, vélo 16 pouces…"
            value={row.keywords}
            maxLength={120}
            onChange={(e) => update(index, { keywords: e.target.value })}
            className="flex-1 rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-sm outline-none transition-colors focus:border-terracotta-500"
          />
        </div>
      ))}
      {error ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}
      <div className="flex items-center gap-3">
        <button
          onClick={save}
          disabled={saving}
          className="inline-flex cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:opacity-60"
        >
          {saving ? "Enregistrement…" : "Enregistrer mes envies"}
        </button>
        {saved ? <span className="text-sm text-sauge-800">C&apos;est noté !</span> : null}
      </div>
    </div>
  );
}

"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { FeedCard, SearchResponse } from "@lebontroc/api-client";

import { ItemCard } from "@/components/ItemCard";
import { BottomSheet } from "@/components/ui/BottomSheet";
import { Segmented } from "@/components/ui/Segmented";
import { apiFetch } from "@/lib/client-api";

const HISTORY_KEY = "lbt_historique_recherche";
const HISTORY_MAX = 8;

const CONDITIONS = [
  { value: "", label: "Tous" },
  { value: "neuf", label: "Neuf" },
  { value: "tres_bon_etat", label: "Très bon" },
  { value: "bon_etat", label: "Bon" },
  { value: "correct", label: "Correct" },
];

const REMISES = [
  { value: "", label: "Tous" },
  { value: "main_propre", label: "Main propre" },
  { value: "envoi", label: "Envoi" },
];

const DISTANCES = [
  { value: "", label: "Peu importe" },
  { value: "5", label: "5 km" },
  { value: "10", label: "10 km" },
  { value: "25", label: "25 km" },
  { value: "50", label: "50 km" },
];

const TRIS = [
  { value: "pertinence", label: "Pertinence" },
  { value: "distance", label: "Distance" },
  { value: "recence", label: "Récence" },
];

type Filters = {
  categoryId: string;
  condition: string;
  delivery: string;
  maxKm: string;
  soulte: boolean;
  /** D'où l'on mesure les distances. Vide = la commune du profil. */
  codePostal: string;
};

const AUCUN_FILTRE: Filters = {
  categoryId: "",
  condition: "",
  delivery: "",
  maxKm: "",
  soulte: false,
  codePostal: "",
};

function buildParams(q: string, filters: Filters, sort: string, page: number): string {
  const params = new URLSearchParams();
  if (q.trim()) params.set("q", q.trim());
  if (filters.categoryId) params.set("category_id", filters.categoryId);
  if (filters.condition) params.set("condition", filters.condition);
  if (filters.delivery) params.set("delivery", filters.delivery);
  if (filters.maxKm) params.set("max_km", filters.maxKm);
  if (filters.soulte) params.set("soulte", "true");
  // Cinq chiffres seulement : sinon l'API répond « code postal inconnu »
  // à chaque frappe intermédiaire.
  if (/^\d{5}$/.test(filters.codePostal)) params.set("postal_code", filters.codePostal);
  params.set("sort", sort);
  params.set("page", String(page));
  return params.toString();
}

export function SearchClient({
  roots,
  initialQuery,
  initialCategoryId = "",
  loggedIn,
  codePostalProfil = "",
}: {
  roots: { id: number; label: string }[];
  initialQuery: string;
  initialCategoryId?: string;
  loggedIn: boolean;
  /** Code postal du profil : le point de référence par défaut. */
  codePostalProfil?: string;
}) {
  const [q, setQ] = useState(initialQuery);
  const [filters, setFilters] = useState<Filters>({
    ...AUCUN_FILTRE,
    categoryId: initialCategoryId,
    codePostal: codePostalProfil,
  });
  // La commune renvoyée par l'API : « distances depuis Nantes ».
  const [reference, setReference] = useState<string | null>(null);
  const [sort, setSort] = useState("pertinence");
  const [items, setItems] = useState<FeedCard[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [history, setHistory] = useState<string[]>([]);
  const sentinel = useRef<HTMLDivElement | null>(null);
  const requestId = useRef(0);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(HISTORY_KEY);
      if (raw) setHistory((JSON.parse(raw) as string[]).slice(0, HISTORY_MAX));
    } catch {
      // localStorage indisponible : tant pis pour l'historique.
    }
  }, []);

  function remember(term: string) {
    const clean = term.trim();
    if (!clean) return;
    setHistory((current) => {
      const next = [clean, ...current.filter((t) => t !== clean)].slice(0, HISTORY_MAX);
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      } catch {
        // ignore
      }
      return next;
    });
  }

  function clearHistory() {
    setHistory([]);
    try {
      localStorage.removeItem(HISTORY_KEY);
    } catch {
      // ignore
    }
  }

  // Recherche débouncée à chaque changement de requête, filtre ou tri.
  useEffect(() => {
    const id = ++requestId.current;
    setLoading(true);
    const timer = setTimeout(async () => {
      try {
        const response = await apiFetch(`/search?${buildParams(q, filters, sort, 1)}`);
        if (!response.ok || id !== requestId.current) return;
        const data = (await response.json()) as SearchResponse;
        if (id !== requestId.current) return;
        setItems(data.items);
        setPage(1);
        setHasMore(data.has_more);
        setReference(data.reference ?? null);
        if (data.items.length > 0) remember(q);
      } finally {
        if (id === requestId.current) setLoading(false);
      }
    }, 350);
    return () => clearTimeout(timer);
  }, [q, filters, sort]);

  const loadMore = useCallback(async () => {
    if (loading || !hasMore) return;
    const id = requestId.current;
    const response = await apiFetch(`/search?${buildParams(q, filters, sort, page + 1)}`);
    if (!response.ok || id !== requestId.current) return;
    const data = (await response.json()) as SearchResponse;
    setItems((current) => {
      const seen = new Set(current.map((i) => i.id));
      return [...current, ...data.items.filter((i) => !seen.has(i.id))];
    });
    setPage(data.page);
    setHasMore(data.has_more);
  }, [loading, hasMore, q, filters, sort, page]);

  useEffect(() => {
    const node = sentinel.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void loadMore();
      },
      { rootMargin: "600px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [loadMore]);

  function trackClick(item: FeedCard, position: number) {
    void apiFetch("/analytics/track", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "search_result_clicked", item_id: item.id, position }),
    });
  }

  const activeFilters = [
    filters.categoryId,
    filters.condition,
    filters.delivery,
    filters.maxKm,
    filters.soulte ? "oui" : "",
  ].filter(Boolean).length;

  return (
    <div className="flex flex-col gap-4">
      <search role="search" className="flex flex-col gap-3">
        <label htmlFor="recherche" className="sr-only">
          Rechercher un objet
        </label>
        <input
          id="recherche"
          type="search"
          autoFocus
          placeholder="Poussette, vélo, manteau…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          className="w-full rounded-full border border-neutre-300 bg-creme px-5 py-3 text-sm outline-none transition-colors focus:border-terracotta-500"
        />
        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={() => setSheetOpen(true)}
            className="inline-flex items-center gap-1.5 rounded-full border border-neutre-300 px-4 py-1.5 text-[13px] transition-colors hover:bg-encre/7"
          >
            Filtres
            {activeFilters > 0 ? (
              <span className="flex size-5 items-center justify-center rounded-full bg-[#c67139] text-[11px] text-creme">
                {activeFilters}
              </span>
            ) : null}
          </button>
          <Segmented
            options={loggedIn ? TRIS : TRIS.filter((t) => t.value !== "distance")}
            value={sort}
            onChange={setSort}
          />
        </div>
      </search>

      {q.trim() === "" && history.length > 0 ? (
        <section className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-neutre-700">Tes dernières recherches</h2>
            <button onClick={clearHistory} className="text-xs text-terracotta-700 hover:underline">
              Effacer
            </button>
          </div>
          <div className="flex flex-wrap gap-2">
            {history.map((term) => (
              <button
                key={term}
                onClick={() => setQ(term)}
                className="rounded-full bg-sable px-4 py-1.5 text-[13px] transition-colors hover:bg-encre/7"
              >
                {term}
              </button>
            ))}
          </div>
        </section>
      ) : null}

      {loading ? (
        <p className="py-8 text-center text-sm text-neutre-700">On cherche…</p>
      ) : items.length === 0 ? (
        <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">Aucun objet trouvé</h2>
          <p className="text-sm text-neutre-700">
            Essaie avec moins de filtres, un mot plus court — ou reviens bientôt, le catalogue
            grandit chaque jour.
          </p>
        </section>
      ) : (
        <>
          <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3 lg:grid-cols-4">
            {items.map((item, index) => (
              <li key={item.id}>
                <ItemCard
                  item={item}
                  source="search"
                  onClick={() => trackClick(item, index)}
                />
              </li>
            ))}
          </ul>
          <div ref={sentinel} aria-hidden />
        </>
      )}

      <BottomSheet open={sheetOpen} onClose={() => setSheetOpen(false)} title="Filtres">
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label htmlFor="filtre-categorie" className="text-xs text-encre/70">
              Catégorie
            </label>
            <select
              id="filtre-categorie"
              value={filters.categoryId}
              onChange={(e) => setFilters((f) => ({ ...f, categoryId: e.target.value }))}
              className="w-full rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-sm"
            >
              <option value="">Toutes</option>
              {roots.map((root) => (
                <option key={root.id} value={root.id}>
                  {root.label}
                </option>
              ))}
            </select>
          </div>

          <Segmented
            label="État"
            options={CONDITIONS}
            value={filters.condition}
            onChange={(condition) => setFilters((f) => ({ ...f, condition }))}
          />

          <Segmented
            label="Remise"
            options={REMISES}
            value={filters.delivery}
            onChange={(delivery) => setFilters((f) => ({ ...f, delivery }))}
          />

          <div className="flex flex-col gap-2">
            <Segmented
              label="Distance max"
              options={DISTANCES}
              value={filters.maxKm}
              onChange={(maxKm) => setFilters((f) => ({ ...f, maxKm }))}
            />
            <label className="flex flex-wrap items-center gap-2 rounded-3xl bg-sable p-4">
              <span className="text-sm font-semibold">Autour de</span>
              <input
                inputMode="numeric"
                maxLength={5}
                placeholder={codePostalProfil || "code postal"}
                value={filters.codePostal}
                onChange={(e) =>
                  setFilters((f) => ({ ...f, codePostal: e.target.value.replace(/\D/g, "") }))
                }
                aria-label="Code postal de référence"
                className="w-28 rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500"
              />
              {filters.codePostal && filters.codePostal !== codePostalProfil ? (
                <button
                  type="button"
                  onClick={() => setFilters((f) => ({ ...f, codePostal: codePostalProfil }))}
                  className="cursor-pointer text-xs text-terracotta-700 underline"
                >
                  revenir chez moi
                </button>
              ) : null}
              <span className="w-full text-xs text-neutre-700">
                {reference
                  ? `Distances mesurées depuis ${reference}.`
                  : loggedIn
                    ? "Distances mesurées depuis la commune de ton profil."
                    : "Saisis un code postal pour trier et filtrer par distance."}
              </span>
            </label>
          </div>

          <label className="flex cursor-pointer items-center justify-between gap-3 rounded-3xl bg-sable p-4">
            <span className="text-sm font-semibold">Accepte une soulte</span>
            <input
              type="checkbox"
              checked={filters.soulte}
              onChange={(e) => setFilters((f) => ({ ...f, soulte: e.target.checked }))}
              className="size-5 accent-[#c67139]"
            />
          </label>

          <div className="flex gap-2">
            <button
              onClick={() => setFilters(AUCUN_FILTRE)}
              className="flex min-h-11 flex-1 items-center justify-center rounded-full px-5 text-sm text-neutre-700 hover:bg-encre/5"
            >
              Tout effacer
            </button>
            <button
              onClick={() => setSheetOpen(false)}
              className="flex min-h-11 flex-1 items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme hover:bg-terracotta-600"
            >
              Voir les résultats
            </button>
          </div>
        </div>
      </BottomSheet>
    </div>
  );
}

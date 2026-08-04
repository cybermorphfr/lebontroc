"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import type { FeedCard, FeedResponse } from "@lebontroc/api-client";

import { apiFetch } from "@/lib/client-api";
import { distanceLabel } from "@/lib/format";

/** Grille du fil avec défilement infini (IntersectionObserver + /feed?page=n). */
export function FeedGrid({ initial }: { initial: FeedResponse }) {
  const [items, setItems] = useState<FeedCard[]>(initial.items);
  const [page, setPage] = useState(initial.page);
  const [hasMore, setHasMore] = useState(initial.has_more);
  const [loading, setLoading] = useState(false);
  const sentinel = useRef<HTMLDivElement | null>(null);

  const loadMore = useCallback(async () => {
    if (loading || !hasMore) return;
    setLoading(true);
    try {
      const response = await apiFetch(`/feed?page=${page + 1}`);
      if (!response.ok) return;
      const next = (await response.json()) as FeedResponse;
      setItems((current) => {
        // L'offset peut glisser quand des objets arrivent : dédoublonner par id.
        const seen = new Set(current.map((i) => i.id));
        return [...current, ...next.items.filter((i) => !seen.has(i.id))];
      });
      setPage(next.page);
      setHasMore(next.has_more);
    } finally {
      setLoading(false);
    }
  }, [loading, hasMore, page]);

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

  return (
    <>
      <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3 lg:grid-cols-4">
        {items.map((item) => (
          <li key={item.id}>
            <Link
              href={`/objet/${item.id}?source=feed`}
              className="flex flex-col overflow-hidden rounded-3xl bg-sable shadow-sm transition-shadow hover:shadow-md"
            >
              <div className="aspect-square bg-neutre-100">
                {item.photo_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={item.photo_url}
                    alt={item.title}
                    loading="lazy"
                    className="size-full object-cover"
                  />
                ) : null}
              </div>
              <div className="flex flex-col gap-1 p-3">
                <span className="truncate text-sm font-semibold">{item.title}</span>
                <div className="flex items-center justify-between gap-2 text-xs text-neutre-700">
                  <span className="truncate">
                    {item.distance_km != null
                      ? distanceLabel(item.distance_km)
                      : (item.city ?? "")}
                  </span>
                  <span className="shrink-0">~{Math.round(item.value_cents / 100)} €</span>
                </div>
              </div>
            </Link>
          </li>
        ))}
      </ul>
      <div ref={sentinel} aria-hidden />
      {loading ? (
        <p className="py-6 text-center text-sm text-neutre-700">On charge la suite…</p>
      ) : null}
      {!hasMore && items.length > 8 ? (
        <p className="py-6 text-center text-sm text-neutre-700">
          Tu as tout vu — reviens plus tard, ça bouge vite.
        </p>
      ) : null}
    </>
  );
}

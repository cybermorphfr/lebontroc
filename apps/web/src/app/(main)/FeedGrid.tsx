"use client";

import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type { FeedCard, FeedResponse } from "@lebontroc/api-client";

import { ItemCard } from "@/components/ItemCard";
import { apiFetch } from "@/lib/client-api";

import Link from "next/link";

/** Grille du fil avec défilement infini (IntersectionObserver + /feed?page=n). */
export function FeedGrid({
  initial,
  favoriteIds = [],
  loggedIn = false,
  showPublishCard = false,
}: {
  initial: FeedResponse;
  favoriteIds?: string[];
  loggedIn?: boolean;
  showPublishCard?: boolean;
}) {
  const favSet = new Set(favoriteIds);
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
        {items.map((item, index) => (
          <Fragment key={item.id}>
            {showPublishCard && index === 4 ? (
              <li>
                <Link
                  href="/publier"
                  className="flex aspect-square flex-col items-center justify-center gap-2 rounded-3xl bg-terracotta-100 p-4 text-center shadow-sm transition-shadow hover:shadow-md"
                >
                  <span aria-hidden className="text-3xl">📸</span>
                  <span className="font-display text-sm text-terracotta-800">
                    Pour troquer, il faut proposer
                  </span>
                  <span className="text-xs text-terracotta-800">
                    Publie ton premier objet — 2 minutes chrono.
                  </span>
                </Link>
              </li>
            ) : null}
            <li>
              <ItemCard
                item={item}
                source="feed"
                favorite={{ initial: favSet.has(item.id), loggedIn }}
              />
            </li>
          </Fragment>
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

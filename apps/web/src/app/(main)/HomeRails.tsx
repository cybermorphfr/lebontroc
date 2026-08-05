"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import type { FeedCard } from "@lebontroc/api-client";

import { ItemCard } from "@/components/ItemCard";
import { apiFetch } from "@/lib/client-api";

const SEUIL_RAIL = 4;
const RAIL_MAX = 12;

/**
 * Rails personnalisés de la home connectée (pattern Leboncoin « dans vos
 * recherches ») : « Dans tes recherches » (wishlist) puis « Tes favoris
 * toujours dispo ». Un rail sous 4 objets n'existe pas — jamais de rail
 * squelettique sur une plateforme en bêta.
 */
export function HomeRails({ favoriteIds }: { favoriteIds: string[] }) {
  const [wishlistItems, setWishlistItems] = useState<FeedCard[]>([]);
  const [favoriteItems, setFavoriteItems] = useState<FeedCard[]>([]);
  const [hasWishlist, setHasWishlist] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const wishlist = await apiFetch("/me/wishlist").then((r) => (r.ok ? r.json() : []));
        const lignes = wishlist as { keywords: string; category_id: number | null }[];
        setHasWishlist(lignes.length > 0);
        const resultats = await Promise.all(
          lignes.map((ligne) => {
            const params = new URLSearchParams();
            if (ligne.keywords) params.set("q", ligne.keywords);
            if (ligne.category_id != null) params.set("category_id", String(ligne.category_id));
            return apiFetch(`/search?${params}`).then((r) => (r.ok ? r.json() : { items: [] }));
          }),
        );
        const vus = new Set<string>();
        const fusion: FeedCard[] = [];
        for (const resultat of resultats as { items: FeedCard[] }[]) {
          for (const item of resultat.items) {
            if (!vus.has(item.id)) {
              vus.add(item.id);
              fusion.push(item);
            }
          }
        }
        setWishlistItems(fusion.slice(0, RAIL_MAX));

        const favoris = (await apiFetch("/me/favorites").then((r) =>
          r.ok ? r.json() : [],
        )) as FeedCard[];
        // Dédup entre rails : la wishlist prime.
        setFavoriteItems(favoris.filter((f) => !vus.has(f.id)).slice(0, RAIL_MAX));
      } catch {
        // Les rails sont un bonus : en cas d'échec, la grille suffit.
      }
    })();
  }, []);

  const favSet = new Set(favoriteIds);

  return (
    <>
      {wishlistItems.length >= SEUIL_RAIL ? (
        <Rail titre="Dans tes recherches" lienTout="/recherche">
          {wishlistItems.map((item) => (
            <ItemCard
              key={item.id}
              item={item}
              source="rail_wishlist"
              compact
              favorite={{ initial: favSet.has(item.id), loggedIn: true }}
            />
          ))}
        </Rail>
      ) : !hasWishlist ? (
        <Link
          href="/profil"
          className="flex items-center justify-between gap-3 rounded-3xl bg-sable px-5 py-3.5 text-sm shadow-sm transition-colors hover:bg-creme"
        >
          <span>
            🔎 <span className="font-semibold">Dis-nous ce que tu cherches</span> — on te le
            trouve dès que ça se publie près de chez toi.
          </span>
          <span aria-hidden className="text-neutre-700">
            →
          </span>
        </Link>
      ) : null}

      {favoriteItems.length >= SEUIL_RAIL ? (
        <Rail titre="Tes favoris toujours dispo" lienTout="/favoris">
          {favoriteItems.map((item) => (
            <ItemCard
              key={item.id}
              item={item}
              source="rail_favoris"
              compact
              favorite={{ initial: true, loggedIn: true }}
            />
          ))}
        </Rail>
      ) : null}
    </>
  );
}

function Rail({
  titre,
  lienTout,
  children,
}: {
  titre: string;
  lienTout: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between">
        <h2 className="font-display text-xl">{titre}</h2>
        <Link href={lienTout} className="text-xs text-neutre-700 underline">
          Voir tout
        </Link>
      </div>
      <div className="-mx-6 flex gap-3 overflow-x-auto px-6 pb-1 [scrollbar-width:none]">
        {children}
      </div>
    </section>
  );
}

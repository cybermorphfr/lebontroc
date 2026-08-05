import Link from "next/link";
import type { FeedCard } from "@lebontroc/api-client";

import { FavoriteHeart } from "@/components/FavoriteHeart";
import { CONDITION_LABELS, distanceLabel, timeAgo } from "@/lib/format";

/**
 * Carte objet de grille — hiérarchie troc : photo > titre > proximité et
 * fraîcheur > état + valeur indicative (discrète : c'est un ordre de
 * grandeur, pas un prix). Cœur favori en overlay quand `favorite` est fourni.
 */
export function ItemCard({
  item,
  source,
  onClick,
  favorite,
  compact = false,
}: {
  item: FeedCard;
  source: string;
  onClick?: () => void;
  favorite?: { initial: boolean; loggedIn: boolean };
  compact?: boolean;
}) {
  return (
    <Link
      href={`/objet/${item.id}?source=${source}`}
      onClick={onClick}
      className={`flex flex-col overflow-hidden rounded-3xl bg-sable shadow-sm transition-shadow hover:shadow-md ${
        compact ? "w-36 shrink-0 sm:w-40" : ""
      }`}
    >
      <div className="relative aspect-square bg-neutre-100">
        {item.photo_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={item.photo_url}
            alt={item.title}
            loading="lazy"
            className="size-full object-cover"
          />
        ) : null}
        {favorite ? (
          <FavoriteHeart itemId={item.id} initial={favorite.initial} loggedIn={favorite.loggedIn} />
        ) : null}
        <span className="absolute bottom-2 left-2 rounded-full bg-creme/90 px-2 py-0.5 text-[10px] font-semibold text-encre">
          {CONDITION_LABELS[item.condition] ?? item.condition}
        </span>
      </div>
      <div className="flex flex-col gap-0.5 p-3">
        <span className="truncate text-sm font-semibold">{item.title}</span>
        <span className="truncate text-xs text-neutre-700">
          {item.distance_km != null
            ? `${distanceLabel(item.distance_km)}${item.city ? ` · ${item.city}` : ""}`
            : (item.city ?? "")}
        </span>
        <div className="flex items-center justify-between gap-2 text-xs text-neutre-700">
          <span>{timeAgo(item.created_at)}</span>
          <span className="shrink-0 opacity-70">~{Math.round(item.value_cents / 100)} €</span>
        </div>
      </div>
    </Link>
  );
}

import Link from "next/link";
import type { FeedCard } from "@lebontroc/api-client";

import { distanceLabel } from "@/lib/format";

/** Carte objet de grille (fil, recherche) — photo, titre, distance, valeur. */
export function ItemCard({
  item,
  source,
  onClick,
}: {
  item: FeedCard;
  source: string;
  onClick?: () => void;
}) {
  return (
    <Link
      href={`/objet/${item.id}?source=${source}`}
      onClick={onClick}
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
            {item.distance_km != null ? distanceLabel(item.distance_km) : (item.city ?? "")}
          </span>
          <span className="shrink-0">~{Math.round(item.value_cents / 100)} €</span>
        </div>
      </div>
    </Link>
  );
}

import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { createApiClient, type ItemDetailResponse } from "@lebontroc/api-client";

import { AvatarLetter } from "@/components/AvatarLetter";
import { ReportButton } from "@/components/ReportButton";
import { Tag } from "@/components/ui/Tag";
import { ancrage, CONDITION_LABELS, DELIVERY_LABELS, distanceLabel } from "@/lib/format";
import { getCurrentUser } from "@/lib/server-api";

import { FavoriteButton } from "./FavoriteButton";
import { Gallery } from "./Gallery";

export const dynamic = "force-dynamic";

const SOURCES = ["feed", "search", "profile", "favorites"] as const;

async function fetchDetail(
  id: string,
  source: string | undefined,
): Promise<ItemDetailResponse | null> {
  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const validSource = SOURCES.find((s) => s === source);
  const { data } = await client.GET("/items/{id}/public", {
    params: { path: { id }, query: validSource ? { source: validSource } : {} },
    cache: "no-store",
  });
  return data ?? null;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string }>;
}): Promise<Metadata> {
  const { id } = await params;
  const detail = await fetchDetail(id, undefined);
  return { title: detail ? `${detail.item.title} — Lebontroc` : "Objet introuvable — Lebontroc" };
}

export default async function ObjetPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ source?: string }>;
}) {
  const [{ id }, { source }, viewer] = await Promise.all([
    params,
    searchParams,
    getCurrentUser(),
  ]);
  const detail = await fetchDetail(id, source);

  if (detail === null) {
    return (
      <main className="mx-auto flex w-full max-w-xl flex-col items-start gap-4 px-6 py-16">
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h1 className="font-display text-2xl">Cet objet n&apos;existe pas (ou plus).</h1>
          <p className="text-sm text-neutre-700">
            Il a peut-être déjà trouvé preneur — le fil regorge d&apos;autres trouvailles.
          </p>
          <Link
            href="/"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Retour au fil
          </Link>
        </section>
      </main>
    );
  }

  const { item, owner, distance_km, is_owner, favorites_count, is_favorited } = detail;

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      {is_owner && item.status !== "disponible" ? (
        <p className="mb-4 rounded-full bg-sauge-100 px-5 py-2 text-center text-sm text-sauge-800">
          Cet objet est {item.status === "masque" ? "masqué" : item.status} — toi seul le vois.
        </p>
      ) : null}

      <div className="flex flex-col gap-6 md:flex-row">
        <div className="md:w-1/2">
          <Gallery
            itemId={item.id}
            title={item.title}
            photos={item.photos.map((p) => p.url)}
          />
        </div>

        <div className="flex flex-col gap-4 md:w-1/2">
          <div className="flex flex-col gap-2">
            <div className="flex items-start justify-between gap-3">
              <h1 className="font-display text-3xl">{item.title}</h1>
              {is_owner ? (
                favorites_count > 0 ? (
                  <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-terracotta-100/60 px-4 py-2 text-sm text-terracotta-800">
                    ♥ {favorites_count} favori{favorites_count > 1 ? "s" : ""}
                  </span>
                ) : null
              ) : (
                <FavoriteButton
                  itemId={item.id}
                  initialCount={favorites_count}
                  initialFavorited={is_favorited}
                  loggedIn={viewer !== null}
                />
              )}
            </div>
            <p className="font-display text-xl text-terracotta-700">
              ~{Math.round(item.value_cents / 100)} € <span className="text-sm text-neutre-700 font-sans">valeur indicative</span>
            </p>
            <div className="flex flex-wrap gap-2">
              <Tag variant="accent-2">{CONDITION_LABELS[item.condition] ?? item.condition}</Tag>
              <Tag variant="neutral">{DELIVERY_LABELS[item.delivery_pref] ?? item.delivery_pref}</Tag>
              {!item.accepts_soulte ? <Tag variant="neutral">Troc sans argent</Tag> : null}
            </div>
            {owner.city ? (
              <p className="text-sm text-neutre-700">
                {owner.city}
                {distance_km != null ? ` · ${distanceLabel(distance_km)}` : ""}
              </p>
            ) : null}
          </div>

          <section className="flex flex-col gap-2">
            <h2 className="font-display text-lg">Description</h2>
            <p className="whitespace-pre-line text-sm text-neutre-700">{item.description}</p>
          </section>

          {item.exchange_wishes ? (
            <section className="flex flex-col gap-1 rounded-3xl bg-terracotta-100/60 p-4">
              <h2 className="font-display text-base">Recherché en échange</h2>
              <p className="text-sm text-neutre-700">{item.exchange_wishes}</p>
            </section>
          ) : null}

          <Link
            href={`/troqueur/${encodeURIComponent(owner.pseudo)}`}
            className="flex items-center gap-3 rounded-3xl bg-sable p-4 shadow-sm transition-shadow hover:shadow-md"
          >
            <AvatarLetter pseudo={owner.pseudo} size="md" />
            <div className="flex flex-col">
              <span className="font-semibold">{owner.pseudo}</span>
              <span className="text-xs text-neutre-700">
                {owner.city ? `${owner.city} · ` : ""}
                {ancrage(owner.member_since)}
              </span>
            </div>
            <span className="ml-auto text-sm text-terracotta-700">Voir son dressing</span>
          </Link>

          {is_owner ? (
            <Link
              href="/dressing"
              className="inline-flex min-h-12 items-center justify-center rounded-full border border-terracotta-500 px-6 font-display text-sm text-terracotta-700 transition-colors hover:bg-terracotta-500/10"
            >
              C&apos;est ton objet — le gérer depuis ton dressing
            </Link>
          ) : (
            <Link
              href={viewer ? `/proposer/${item.id}` : "/connexion"}
              className="inline-flex min-h-12 w-full items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
            >
              Proposer un troc
            </Link>
          )}

          {viewer && !is_owner ? (
            <ReportButton cible="objet" targetId={item.id} libelle="Signaler cette annonce" />
          ) : null}
        </div>
      </div>
    </main>
  );
}

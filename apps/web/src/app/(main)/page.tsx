import Link from "next/link";
import { cookies } from "next/headers";
import { createApiClient, type FeedResponse } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { CategoryChips } from "./CategoryChips";
import { FeedGrid } from "./FeedGrid";
import { HomeRails } from "./HomeRails";
import { HomeSearch } from "./HomeSearch";

// Home façon Leboncoin (search-first) × Vinted (supply-first) : recherche
// proéminente, chips catégories, rails personnalisés conditionnels, hero
// visiteur qui recrute des proposeurs, grille proximité+fraîcheur en fond.
export const dynamic = "force-dynamic";

function apiClient(cookie: string) {
  return createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", { cookie });
}

export default async function Home() {
  const jar = await cookies();
  const cookie = jar
    .getAll()
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");
  const client = apiClient(cookie);

  const [user, feedRes, categoriesRes] = await Promise.all([
    getCurrentUser(),
    client.GET("/feed", { cache: "no-store" }).catch(() => null),
    client.GET("/categories", { cache: "no-store" }).catch(() => null),
  ]);
  const feed: FeedResponse | null = feedRes?.data ?? null;
  const roots = (categoriesRes?.data ?? []).map((c) => ({ id: c.id, label: c.label }));

  // Connecté : les favoris (cœurs) et le stock (carte in-feed « Publier »).
  let favoriteIds: string[] = [];
  let hasActiveItems = true;
  if (user) {
    const [favorites, myItems] = await Promise.all([
      client.GET("/me/favorites", { cache: "no-store" }).catch(() => null),
      client.GET("/me/items", { cache: "no-store" }).catch(() => null),
    ]);
    favoriteIds = (favorites?.data ?? []).map((f) => f.id);
    hasActiveItems = (myItems?.data ?? []).some((i) => i.status === "disponible");
  }

  return (
    <main className="mx-auto flex w-full max-w-4xl flex-col gap-5 px-6 pb-16">
      {user === null ? (
        <section className="flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm sm:p-8">
          <div className="flex flex-col items-start gap-3">
            <h1 className="font-display text-3xl sm:text-4xl">
              Ce que tu n&apos;utilises plus vaut de l&apos;or pour quelqu&apos;un à côté.
            </h1>
            <p className="max-w-lg text-neutre-700">
              Fais du tri, publie tes objets, et troque-les près de chez toi — sans argent, ou
              presque.
            </p>
            <div className="flex flex-wrap items-center gap-3">
              <Link
                href="/publier"
                className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
              >
                Publie ton premier objet
              </Link>
              <a href="#comment" className="text-sm text-neutre-700 underline">
                Comment ça marche ?
              </a>
            </div>
          </div>
          <HomeSearch />
        </section>
      ) : (
        <HomeSearch />
      )}

      {roots.length > 0 ? <CategoryChips roots={roots} /> : null}

      {user === null ? (
        <section
          id="comment"
          className="grid gap-3 rounded-[32px] bg-sable p-6 shadow-sm sm:grid-cols-3"
        >
          {[
            ["1", "Tu proposes tes objets", "Photo, deux lignes, une valeur indicative."],
            ["2", "On te propose un troc", "Objet contre objet — avec une soulte si besoin."],
            ["3", "Vous échangez à côté", "En main propre ou en point relais, paiement sécurisé."],
          ].map(([n, titre, detail]) => (
            <div key={n} className="flex flex-col gap-1">
              <span className="font-display text-2xl text-terracotta-800">{n}.</span>
              <span className="text-sm font-semibold">{titre}</span>
              <span className="text-xs text-neutre-700">{detail}</span>
            </div>
          ))}
        </section>
      ) : (
        <HomeRails favoriteIds={favoriteIds} />
      )}

      <div className="flex items-baseline gap-2">
        <h2 className="font-display text-2xl">
          {user ? "Autour de toi" : "Ça se troque autour de Nantes"}
        </h2>
        {user ? (
          <span className="text-sm text-neutre-700">les plus proches et les plus récents</span>
        ) : null}
      </div>

      {feed === null ? (
        <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
          <h3 className="font-display text-lg">Le fil fait des siennes</h3>
          <p className="text-sm text-neutre-700">
            On n&apos;arrive pas à charger les objets. Recharge la page dans un instant.
          </p>
        </section>
      ) : feed.items.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h3 className="font-display text-lg">Rien à troquer pour l&apos;instant</h3>
          <p className="text-sm text-neutre-700">
            Sois la première personne à publier un objet — le fil n&apos;attend que toi.
          </p>
          <Link
            href="/publier"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Publier un objet
          </Link>
        </section>
      ) : (
        <FeedGrid
          initial={feed}
          favoriteIds={favoriteIds}
          loggedIn={user !== null}
          showPublishCard={user !== null && !hasActiveItems}
        />
      )}

      {user === null ? (
        <section className="flex flex-wrap items-center justify-between gap-3 rounded-[32px] bg-sauge-100 p-6">
          <p className="font-display text-lg text-sauge-800">
            Prête ou prêt à troquer plutôt qu&apos;acheter ?
          </p>
          <Link
            href="/inscription"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Je crée mon compte
          </Link>
        </section>
      ) : null}
    </main>
  );
}

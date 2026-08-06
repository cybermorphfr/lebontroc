import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient } from "@lebontroc/api-client";

import { ItemCard } from "@/components/ItemCard";
import { Tag } from "@/components/ui/Tag";
import { getCurrentUser } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Mes favoris — Lebontroc",
};

export const dynamic = "force-dynamic";

// Les favoris s'accumulent vite : rangés par rayon, on retrouve « le vélo
// que j'avais repéré » sans dérouler toute la grille.

export default async function FavorisPage() {
  const user = await getCurrentUser();
  if (!user) redirect("/connexion");

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data } = await client.GET("/me/favorites", { cache: "no-store" });
  const items = data ?? [];

  // Un rayon = une catégorie racine. L'ordre suit le volume : le rayon où
  // l'on collectionne le plus arrive en tête.
  const rayons = new Map<string, typeof items>();
  for (const item of items) {
    const liste = rayons.get(item.rayon);
    if (liste) liste.push(item);
    else rayons.set(item.rayon, [item]);
  }
  const groupes = [...rayons.entries()].sort(
    (a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0], "fr"),
  );

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">Mes favoris</h1>
        <Tag variant="neutral">
          {items.length} objet{items.length > 1 ? "s" : ""}
        </Tag>
        {groupes.length > 1 ? (
          <Tag variant="neutral">
            {groupes.length} catégorie{groupes.length > 1 ? "s" : ""}
          </Tag>
        ) : null}
      </div>

      {items.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">Aucun favori pour l&apos;instant</h2>
          <p className="text-sm text-neutre-700">
            Touche le cœur sur une fiche objet pour le retrouver ici — et être prêt le jour où tu
            proposes un troc.
          </p>
          <Link
            href="/"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Explorer le fil
          </Link>
        </section>
      ) : (
        <>
          {/* Sommaire des rayons : un clic descend à la bonne section. */}
          {groupes.length > 1 ? (
            <nav aria-label="Catégories" className="mb-4 flex flex-wrap gap-1.5">
              {groupes.map(([rayon, liste]) => (
                <a
                  key={rayon}
                  href={`#rayon-${encodeURIComponent(rayon)}`}
                  className="rounded-full bg-sable px-3.5 py-1.5 text-sm transition-colors hover:bg-terracotta-100"
                >
                  {rayon} <span className="text-xs text-neutre-700">({liste.length})</span>
                </a>
              ))}
            </nav>
          ) : null}

          <div className="flex flex-col gap-6">
            {groupes.map(([rayon, liste]) => (
              <section
                key={rayon}
                id={`rayon-${encodeURIComponent(rayon)}`}
                className="flex scroll-mt-20 flex-col gap-3"
              >
                <div className="flex items-baseline gap-2">
                  <h2 className="font-display text-lg">{rayon}</h2>
                  <span className="text-xs text-neutre-700">
                    {liste.length} objet{liste.length > 1 ? "s" : ""}
                  </span>
                </div>
                <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3 lg:grid-cols-4">
                  {liste.map((item) => (
                    <li key={item.id} className="flex flex-col gap-1">
                      <ItemCard item={item} source="favorites" />
                      <span className="px-1 text-[11px] text-neutre-700">{item.categorie}</span>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </>
      )}
    </main>
  );
}

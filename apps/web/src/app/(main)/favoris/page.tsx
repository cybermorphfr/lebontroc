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

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      <div className="mb-4 flex items-center gap-2">
        <h1 className="font-display text-2xl">Mes favoris</h1>
        <Tag variant="neutral">
          {items.length} objet{items.length > 1 ? "s" : ""}
        </Tag>
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
        <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3 lg:grid-cols-4">
          {items.map((item) => (
            <li key={item.id}>
              <ItemCard item={item} source="favorites" />
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

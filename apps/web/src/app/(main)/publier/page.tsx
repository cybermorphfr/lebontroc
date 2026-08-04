import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { cookies } from "next/headers";
import { createApiClient, type ItemResponse } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { PublishForm } from "./PublishForm";

export const metadata: Metadata = {
  title: "Publier un objet — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function PublierPage({
  searchParams,
}: {
  searchParams: Promise<{ objet?: string }>;
}) {
  const user = await getCurrentUser();
  if (!user) redirect("/connexion");

  const base = process.env.API_INTERNAL_URL ?? "http://localhost:8080";
  const client = createApiClient(base);
  const { data: categories } = await client.GET("/categories", { cache: "no-store" });

  const { objet } = await searchParams;
  let editItem: ItemResponse | undefined;
  if (objet) {
    const jar = await cookies();
    const authed = createApiClient(base, {
      cookie: jar
        .getAll()
        .map((c) => `${c.name}=${c.value}`)
        .join("; "),
    });
    const { data } = await authed.GET("/items/{id}", {
      params: { path: { id: objet } },
      cache: "no-store",
    });
    if (!data || data.owner_id !== user.id) redirect("/dressing");
    editItem = data;
  }

  return (
    <main className="mx-auto w-full max-w-xl px-6 pb-16">
      <PublishForm
        categories={categories ?? []}
        verified={user.email_verified}
        editItem={editItem}
      />
    </main>
  );
}

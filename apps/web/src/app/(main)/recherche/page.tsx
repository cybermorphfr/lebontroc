import type { Metadata } from "next";
import { createApiClient } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { SearchClient } from "./SearchClient";

export const metadata: Metadata = {
  title: "Rechercher — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function RecherchePage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string; categorie?: string }>;
}) {
  const [{ q, categorie }, user] = await Promise.all([searchParams, getCurrentUser()]);
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080");
  const { data: categories } = await client.GET("/categories", { cache: "no-store" });

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      <SearchClient
        roots={(categories ?? []).map((c) => ({ id: c.id, label: c.label }))}
        initialQuery={q ?? ""}
        initialCategoryId={categorie ?? ""}
        loggedIn={user !== null}
      />
    </main>
  );
}

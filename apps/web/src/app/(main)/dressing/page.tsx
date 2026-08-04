import type { Metadata } from "next";
import Link from "next/link";
import { redirect } from "next/navigation";
import { cookies } from "next/headers";
import { createApiClient } from "@lebontroc/api-client";

import { Tag } from "@/components/ui/Tag";
import { getCurrentUser } from "@/lib/server-api";

import { DressingGrid } from "./DressingGrid";

export const metadata: Metadata = {
  title: "Mon dressing — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function DressingPage() {
  const user = await getCurrentUser();
  if (!user) redirect("/connexion");

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data: items } = await client.GET("/me/items", { cache: "no-store" });

  return (
    <main className="mx-auto w-full max-w-2xl px-6 pb-16">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <h1 className="font-display text-2xl">Mon dressing</h1>
          <Tag variant="neutral">
            {(items ?? []).length} objet{(items ?? []).length > 1 ? "s" : ""}
          </Tag>
        </div>
        <Link
          href="/publier"
          className="rounded-full px-3 py-2 font-display text-sm text-terracotta-700 transition-colors hover:bg-terracotta-500/10"
        >
          Publier
        </Link>
      </div>
      <DressingGrid items={items ?? []} />
    </main>
  );
}

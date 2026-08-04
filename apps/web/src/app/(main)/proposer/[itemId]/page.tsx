import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { ProposalComposer } from "./ProposalComposer";

export const metadata: Metadata = {
  title: "Proposer un troc — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function ProposerPage({
  params,
}: {
  params: Promise<{ itemId: string }>;
}) {
  const [{ itemId }, user] = await Promise.all([params, getCurrentUser()]);
  if (!user) redirect("/connexion");

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });

  const { data: detail } = await client.GET("/items/{id}/public", {
    params: { path: { id: itemId }, query: {} },
    cache: "no-store",
  });
  if (!detail || detail.is_owner) redirect(detail ? "/dressing" : "/");

  const [{ data: profile }, { data: myItems }] = await Promise.all([
    client.GET("/troqueurs/{pseudo}", {
      params: { path: { pseudo: detail.owner.pseudo } },
      cache: "no-store",
    }),
    client.GET("/me/items", { cache: "no-store" }),
  ]);

  const mine = (myItems ?? []).filter((i) => i.status === "disponible");
  const theirs = (profile?.items ?? []).filter((i) => i.status === "disponible");

  if (mine.length === 0) {
    return (
      <main className="mx-auto flex w-full max-w-xl flex-col items-start gap-4 px-6 py-10">
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h1 className="font-display text-2xl">Il te faut un objet à troquer</h1>
          <p className="text-sm text-neutre-700">
            Un troc, c&apos;est « ça contre ça » : publie d&apos;abord un objet de ton dressing,
            et reviens proposer ton échange à {detail.owner.pseudo}.
          </p>
          <Link
            href="/publier"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Publier mon premier objet
          </Link>
        </section>
      </main>
    );
  }

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      <h1 className="mb-1 font-display text-2xl">Proposer un troc</h1>
      <p className="mb-5 text-sm text-neutre-700">
        Compose ton « ça contre ça » avec {detail.owner.pseudo}.
      </p>
      <ProposalComposer
        mine={mine}
        theirs={theirs}
        recipientPseudo={detail.owner.pseudo}
        preselectedRequested={[detail.item.id]}
      />
    </main>
  );
}

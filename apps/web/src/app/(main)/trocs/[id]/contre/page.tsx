import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { ProposalComposer } from "../../../proposer/[itemId]/ProposalComposer";

export const metadata: Metadata = {
  title: "Contre-proposer — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function ContreProposerPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const [{ id }, user] = await Promise.all([params, getCurrentUser()]);
  if (!user) redirect("/connexion");

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data: proposal } = await client.GET("/proposals/{id}", {
    params: { path: { id } },
    cache: "no-store",
  });
  // Seul le destinataire d'une proposition encore ouverte peut contrer.
  if (
    !proposal ||
    proposal.is_proposer ||
    (proposal.status !== "envoyee" && proposal.status !== "vue")
  ) {
    redirect(proposal ? `/trocs/${id}` : "/trocs");
  }

  const [{ data: theirProfile }, { data: myItems }] = await Promise.all([
    client.GET("/troqueurs/{pseudo}", {
      params: { path: { pseudo: proposal.proposer_pseudo } },
      cache: "no-store",
    }),
    client.GET("/me/items", { cache: "no-store" }),
  ]);

  const mine = (myItems ?? []).filter((i) => i.status === "disponible");
  const theirs = (theirProfile?.items ?? []).filter((i) => i.status === "disponible");

  // Point de départ : la composition actuelle, vue de mon côté (inversée).
  const initialOffered = proposal.requested
    .map((i) => i.item_id)
    .filter((itemId) => mine.some((m) => m.id === itemId));
  const initialRequested = proposal.offered
    .map((i) => i.item_id)
    .filter((itemId) => theirs.some((t) => t.id === itemId));

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      <h1 className="mb-1 font-display text-2xl">Contre-proposer</h1>
      <p className="mb-5 text-sm text-neutre-700">
        Ajuste l&apos;échange à ta façon — ta contre-proposition remplacera celle de{" "}
        {proposal.proposer_pseudo}.{" "}
        <Link href={`/trocs/${id}`} className="text-terracotta-700 underline">
          Revenir à la proposition
        </Link>
      </p>
      <ProposalComposer
        mine={mine}
        theirs={theirs}
        recipientPseudo={proposal.proposer_pseudo}
        preselectedRequested={initialRequested}
        preselectedOffered={initialOffered}
        counterOf={id}
      />
    </main>
  );
}

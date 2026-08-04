import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient, type ProposalResponse } from "@lebontroc/api-client";

import { Tag } from "@/components/ui/Tag";
import { getCurrentUser } from "@/lib/server-api";
import { STATUS_PROPOSITION } from "@/lib/format";

export const metadata: Metadata = {
  title: "Mes trocs — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function TrocsPage({
  searchParams,
}: {
  searchParams: Promise<{ box?: string }>;
}) {
  const [{ box }, user] = await Promise.all([searchParams, getCurrentUser()]);
  if (!user) redirect("/connexion");
  const envoyees = box === "envoyees";

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data } = await client.GET("/me/proposals", {
    params: { query: { box: envoyees ? "envoyees" : "recues" } },
    cache: "no-store",
  });
  const proposals = data ?? [];

  return (
    <main className="mx-auto w-full max-w-2xl px-6 pb-16">
      <h1 className="mb-4 font-display text-2xl">Mes trocs</h1>

      <div className="mb-5 inline-flex rounded-full border border-neutre-300">
        <Link
          href="/trocs"
          className={`rounded-full px-5 py-1.5 text-sm ${!envoyees ? "bg-[#c67139] text-creme" : "hover:bg-encre/7"}`}
        >
          Reçues
        </Link>
        <Link
          href="/trocs?box=envoyees"
          className={`rounded-full px-5 py-1.5 text-sm ${envoyees ? "bg-[#c67139] text-creme" : "hover:bg-encre/7"}`}
        >
          Envoyées
        </Link>
      </div>

      {proposals.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">
            {envoyees ? "Aucune proposition envoyée" : "Aucune proposition reçue"}
          </h2>
          <p className="text-sm text-neutre-700">
            {envoyees
              ? "Repère un objet qui te plaît et propose ton premier « ça contre ça »."
              : "Quand un troqueur voudra un de tes objets, sa proposition arrivera ici."}
          </p>
          <Link
            href="/"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Explorer le fil
          </Link>
        </section>
      ) : (
        <ul className="flex flex-col gap-3">
          {proposals.map((proposal) => (
            <ProposalRow key={proposal.id} proposal={proposal} />
          ))}
        </ul>
      )}
    </main>
  );
}

function ProposalRow({ proposal }: { proposal: ProposalResponse }) {
  const statut = STATUS_PROPOSITION[proposal.status] ?? {
    label: proposal.status,
    variant: "neutral" as const,
  };
  const counterpart = proposal.is_proposer
    ? proposal.recipient_pseudo
    : proposal.proposer_pseudo;
  const vignettes = [...proposal.offered, ...proposal.requested]
    .map((i) => i.photo_url)
    .filter((u): u is string => u != null)
    .slice(0, 4);

  return (
    <li>
      <Link
        href={`/trocs/${proposal.id}`}
        className="flex items-center gap-3 rounded-3xl bg-sable p-4 shadow-sm transition-shadow hover:shadow-md"
      >
        <div className="flex -space-x-3">
          {vignettes.map((url, index) => (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              key={index}
              src={url}
              alt=""
              className="size-12 rounded-full border-2 border-sable object-cover"
            />
          ))}
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-semibold">
            {proposal.is_proposer ? `À ${counterpart}` : `De ${counterpart}`}
          </span>
          <span className="truncate text-xs text-neutre-700">
            {proposal.offered.length} objet{proposal.offered.length > 1 ? "s" : ""} contre{" "}
            {proposal.requested.length}
            {proposal.cash_cents > 0 ? ` + ${Math.round(proposal.cash_cents / 100)} €` : ""}
          </span>
        </div>
        <Tag variant={statut.variant}>{statut.label}</Tag>
      </Link>
    </li>
  );
}

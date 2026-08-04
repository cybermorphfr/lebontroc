import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient, type ProposalItemResponse } from "@lebontroc/api-client";

import { Tag } from "@/components/ui/Tag";
import { getCurrentUser } from "@/lib/server-api";
import { STATUS_PROPOSITION } from "@/lib/format";

import { RefuseButton } from "./RefuseButton";

export const metadata: Metadata = {
  title: "Proposition de troc — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function TrocDetailPage({
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

  if (!proposal) {
    return (
      <main className="mx-auto flex w-full max-w-xl flex-col items-start gap-4 px-6 py-16">
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h1 className="font-display text-2xl">Cette proposition n&apos;existe pas.</h1>
          <Link
            href="/trocs"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Retour à mes trocs
          </Link>
        </section>
      </main>
    );
  }

  const statut = STATUS_PROPOSITION[proposal.status] ?? {
    label: proposal.status,
    variant: "neutral" as const,
  };
  const ouverte = proposal.status === "envoyee" || proposal.status === "vue";
  const joursRestants = Math.max(
    0,
    Math.ceil((new Date(proposal.expires_at).getTime() - Date.now()) / (24 * 3600 * 1000)),
  );

  return (
    <main className="mx-auto w-full max-w-2xl px-6 pb-16">
      <div className="mb-4 flex items-center justify-between gap-3">
        <h1 className="font-display text-2xl">
          {proposal.is_proposer
            ? `Ta proposition à ${proposal.recipient_pseudo}`
            : `La proposition de ${proposal.proposer_pseudo}`}
        </h1>
        <Tag variant={statut.variant}>{statut.label}</Tag>
      </div>

      {ouverte ? (
        <p className="mb-4 text-sm text-neutre-700">
          Sans réponse, elle expirera dans {joursRestants} jour{joursRestants > 1 ? "s" : ""}.
        </p>
      ) : null}

      <div className="grid gap-4 sm:grid-cols-2">
        <RecapCard
          title={proposal.is_proposer ? "Tu donnes" : `${proposal.proposer_pseudo} donne`}
          items={proposal.offered}
          cash={proposal.cash_direction === "du_proposant" ? proposal.cash_cents : 0}
        />
        <RecapCard
          title={proposal.is_proposer ? "Tu reçois" : "Tu donnes"}
          items={proposal.requested}
          cash={proposal.cash_direction === "du_destinataire" ? proposal.cash_cents : 0}
        />
      </div>

      {proposal.message ? (
        <section className="mt-4 flex flex-col gap-1 rounded-3xl bg-terracotta-100/60 p-4">
          <h2 className="font-display text-base">
            Le mot de {proposal.is_proposer ? "toi" : proposal.proposer_pseudo}
          </h2>
          <p className="whitespace-pre-line text-sm text-neutre-700">{proposal.message}</p>
        </section>
      ) : null}

      <div className="mt-6 flex flex-col gap-3">
        {!proposal.is_proposer && ouverte ? (
          <>
            <p className="rounded-3xl bg-sable p-4 text-sm text-neutre-700">
              Accepter, négocier ou discuter arrive très bientôt — en attendant, tu peux déjà
              refuser si l&apos;échange ne te tente pas.
            </p>
            <RefuseButton proposalId={proposal.id} />
          </>
        ) : null}
        <Link href="/trocs" className="text-sm text-terracotta-700 hover:underline">
          ← Retour à mes trocs
        </Link>
      </div>
    </main>
  );
}

function RecapCard({
  title,
  items,
  cash,
}: {
  title: string;
  items: ProposalItemResponse[];
  cash: number;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-[32px] bg-sable p-5 shadow-sm">
      <h2 className="font-display text-lg">{title}</h2>
      <ul className="flex flex-col gap-2">
        {items.map((item) => (
          <li key={item.item_id} className="flex items-center gap-3">
            <div className="size-12 shrink-0 overflow-hidden rounded-xl bg-neutre-100">
              {item.photo_url ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img src={item.photo_url} alt="" className="size-full object-cover" />
              ) : null}
            </div>
            <div className="flex min-w-0 flex-col">
              <Link
                href={`/objet/${item.item_id}`}
                className="truncate text-sm font-semibold hover:underline"
              >
                {item.title}
              </Link>
              <span className="text-xs text-neutre-700">
                ~{Math.round(item.value_cents / 100)} €
              </span>
            </div>
          </li>
        ))}
        {cash > 0 ? (
          <li className="font-display text-sm text-terracotta-700">
            + {Math.round(cash / 100)} € de soulte
          </li>
        ) : null}
      </ul>
    </section>
  );
}

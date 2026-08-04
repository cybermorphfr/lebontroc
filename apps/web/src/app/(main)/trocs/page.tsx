import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient, type ConversationResponse } from "@lebontroc/api-client";

import { Tag } from "@/components/ui/Tag";
import { getCurrentUser } from "@/lib/server-api";
import { STATUS_PROPOSITION } from "@/lib/format";

export const metadata: Metadata = {
  title: "Mes trocs — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function TrocsPage() {
  const user = await getCurrentUser();
  if (!user) redirect("/connexion");

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data } = await client.GET("/me/conversations", { cache: "no-store" });
  const conversations = data ?? [];

  return (
    <main className="mx-auto w-full max-w-2xl px-6 pb-16">
      <h1 className="mb-4 font-display text-2xl">Mes trocs</h1>

      {conversations.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">Aucun troc en cours</h2>
          <p className="text-sm text-neutre-700">
            Repère un objet qui te plaît et propose ton premier « ça contre ça » — les
            conversations vivront ici.
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
          {conversations.map((conversation) => (
            <ConversationRow key={conversation.proposal.id} conversation={conversation} />
          ))}
        </ul>
      )}
    </main>
  );
}

function ConversationRow({ conversation }: { conversation: ConversationResponse }) {
  const { proposal } = conversation;
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
  const apercu = conversation.last_message
    ? `${conversation.last_is_mine ? "Toi : " : ""}${conversation.last_message}`
    : `${proposal.offered.length} objet${proposal.offered.length > 1 ? "s" : ""} contre ${proposal.requested.length}${proposal.cash_cents > 0 ? ` + ${Math.round(proposal.cash_cents / 100)} €` : ""}`;

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
          <span
            className={`truncate text-xs ${conversation.unread_count > 0 ? "font-semibold text-encre" : "text-neutre-700"}`}
          >
            {apercu}
          </span>
        </div>
        {conversation.unread_count > 0 ? (
          <span
            aria-label={`${conversation.unread_count} message${conversation.unread_count > 1 ? "s" : ""} non lu${conversation.unread_count > 1 ? "s" : ""}`}
            className="flex size-6 shrink-0 items-center justify-center rounded-full bg-[#c67139] text-xs font-semibold text-creme"
          >
            {conversation.unread_count}
          </span>
        ) : null}
        <Tag variant={statut.variant}>{statut.label}</Tag>
      </Link>
    </li>
  );
}

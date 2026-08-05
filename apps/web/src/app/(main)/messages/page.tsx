"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import type { ConversationResponse } from "@lebontroc/api-client";

import { AvatarLetter } from "@/components/AvatarLetter";
import { apiFetch } from "@/lib/client-api";
import { timeAgo } from "@/lib/format";
import { useRealtime } from "@/lib/realtime";

const STATUTS: Record<string, string> = {
  envoyee: "Proposition envoyée",
  vue: "Proposition vue",
  acceptee: "Troc en cours",
  refusee: "Refusée",
  contre_proposee: "Contre-proposée",
  expiree: "Expirée",
  caduque: "Caduque",
};

/**
 * Boîte de réception (pattern Vinted) : une ligne par conversation —
 * l'autre partie, l'aperçu du dernier message, la vignette de l'objet,
 * le badge de non-lus. Temps réel via le WebSocket.
 */
export default function MessagesPage() {
  const [conversations, setConversations] = useState<ConversationResponse[]>([]);
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(() => {
    apiFetch("/me/conversations")
      .then((r) => (r.ok ? r.json() : null))
      .then((data: ConversationResponse[] | null) => {
        if (data) {
          setConversations(
            [...data].sort((a, b) => {
              const ta = a.last_at ?? a.proposal.created_at;
              const tb = b.last_at ?? b.proposal.created_at;
              return tb.localeCompare(ta);
            }),
          );
        }
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  useEffect(reload, [reload]);
  useRealtime((event) => {
    if (event.type === "message" || event.type === "read") reload();
  });

  const totalUnread = conversations.reduce((sum, c) => sum + c.unread_count, 0);

  return (
    <main className="mx-auto flex w-full max-w-xl flex-col gap-4 px-6 pb-16">
      <div className="flex items-baseline gap-2">
        <h1 className="font-display text-2xl">Messages</h1>
        {totalUnread > 0 ? (
          <span className="text-sm text-neutre-700">
            {totalUnread} non lu{totalUnread > 1 ? "s" : ""}
          </span>
        ) : null}
      </div>

      {loaded && conversations.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">Aucune conversation pour l&apos;instant</h2>
          <p className="text-sm text-neutre-700">
            Chaque proposition de troc ouvre une conversation. Repère un objet qui te plaît et
            lance-toi !
          </p>
          <Link
            href="/recherche"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Explorer le fil
          </Link>
        </section>
      ) : (
        <ul className="flex flex-col gap-2">
          {conversations.map((conversation) => {
            const { proposal } = conversation;
            const other = proposal.is_proposer
              ? proposal.recipient_pseudo
              : proposal.proposer_pseudo;
            // La vignette : ce que JE convoite (ses objets si je propose).
            const vignette = proposal.is_proposer
              ? (proposal.requested[0]?.photo_url ?? proposal.offered[0]?.photo_url ?? null)
              : (proposal.offered[0]?.photo_url ?? proposal.requested[0]?.photo_url ?? null);
            const apercu = conversation.last_message
              ? `${conversation.last_is_mine ? "Toi : " : ""}${conversation.last_message}`
              : (STATUTS[proposal.status] ?? proposal.status);
            const quand = conversation.last_at ?? proposal.created_at;
            const unread = conversation.unread_count > 0;
            return (
              <li key={proposal.id}>
                <Link
                  href={`/trocs/${proposal.id}`}
                  className={`flex items-center gap-3 rounded-3xl p-3.5 shadow-sm transition-colors ${
                    unread ? "bg-sable hover:bg-creme" : "bg-sable/60 hover:bg-sable"
                  }`}
                >
                  <AvatarLetter pseudo={other} size="md" />
                  <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <div className="flex items-baseline justify-between gap-2">
                      <span className={`truncate text-sm ${unread ? "font-bold" : "font-semibold"}`}>
                        {other}
                      </span>
                      <span className="shrink-0 text-xs text-neutre-700">{timeAgo(quand)}</span>
                    </div>
                    <div className="flex items-center justify-between gap-2">
                      <span
                        className={`truncate text-sm ${
                          unread ? "font-semibold text-encre" : "text-neutre-700"
                        }`}
                      >
                        {apercu}
                      </span>
                      {unread ? (
                        <span className="flex h-5 min-w-5 shrink-0 items-center justify-center rounded-full bg-[#c67139] px-1.5 font-display text-[11px] leading-none text-creme">
                          {conversation.unread_count > 9 ? "9+" : conversation.unread_count}
                        </span>
                      ) : null}
                    </div>
                    <span className="text-[11px] text-neutre-700">
                      {STATUTS[proposal.status] ?? proposal.status}
                    </span>
                  </div>
                  {vignette ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={vignette}
                      alt=""
                      className="size-12 shrink-0 rounded-2xl object-cover"
                    />
                  ) : null}
                </Link>
              </li>
            );
          })}
        </ul>
      )}
    </main>
  );
}

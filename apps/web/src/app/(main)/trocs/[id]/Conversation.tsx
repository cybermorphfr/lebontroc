"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { MessageResponse } from "@lebontroc/api-client";

import { apiFetch, apiError } from "@/lib/client-api";
import { compressImage } from "@/lib/photos";
import { useRealtime } from "@/lib/realtime";

/** Fil de conversation temps réel sous la carte de proposition épinglée. */
export function Conversation({
  proposalId,
  myPseudo,
  initialMessages,
  closed,
}: {
  proposalId: string;
  myPseudo: string;
  initialMessages: MessageResponse[];
  closed: boolean;
}) {
  const [messages, setMessages] = useState<MessageResponse[]>(initialMessages);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [redactedNotice, setRedactedNotice] = useState(false);
  const bottom = useRef<HTMLDivElement | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);

  const markRead = useCallback(() => {
    void apiFetch(`/proposals/${proposalId}/read`, { method: "POST" });
  }, [proposalId]);

  useEffect(() => {
    markRead();
  }, [markRead]);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "nearest" });
  }, [messages.length]);

  useRealtime((event) => {
    if (!("proposal_id" in event) || event.proposal_id !== proposalId) return;
    if (event.type === "message") {
      const message = event.message as MessageResponse;
      setMessages((current) =>
        current.some((m) => m.id === message.id) ? current : [...current, message],
      );
      if (message.sender_pseudo !== myPseudo) markRead();
    }
    if (event.type === "read" && event.reader_pseudo !== myPseudo) {
      const now = new Date().toISOString();
      setMessages((current) =>
        current.map((m) =>
          m.sender_pseudo === myPseudo && m.read_at === null ? { ...m, read_at: now } : m,
        ),
      );
    }
  });

  async function send(photoId?: string) {
    if (sending) return;
    const body = draft.trim();
    if (!body && !photoId) return;
    setSending(true);
    setError(null);
    try {
      const response = await apiFetch(`/proposals/${proposalId}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ body: body || null, photo_id: photoId ?? null }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      const message = (await response.json()) as MessageResponse;
      setMessages((current) =>
        current.some((m) => m.id === message.id) ? current : [...current, message],
      );
      if (message.redacted) setRedactedNotice(true);
      setDraft("");
    } finally {
      setSending(false);
    }
  }

  async function attachPhoto(file: File) {
    setSending(true);
    setError(null);
    try {
      const { blob, contentType } = await compressImage(file);
      const presign = await apiFetch("/items/photos/presign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ files: [{ content_type: contentType, size: blob.size }] }),
      });
      if (!presign.ok) {
        setError((await apiError(presign)).message);
        return;
      }
      const [{ photo_id, upload_url }] = (await presign.json()) as {
        photo_id: string;
        upload_url: string;
      }[];
      const put = await fetch(upload_url, {
        method: "PUT",
        headers: { "Content-Type": contentType },
        body: blob,
      });
      if (!put.ok) {
        setError("La photo n'a pas pu être envoyée. Réessaie.");
        return;
      }
      setSending(false);
      await send(photo_id);
    } catch {
      setError("La photo n'a pas pu être lue.");
    } finally {
      setSending(false);
    }
  }

  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-display text-lg">Conversation</h2>

      <div className="flex max-h-[50vh] min-h-32 flex-col gap-2 overflow-y-auto rounded-[24px] bg-sable p-4">
        {messages.length === 0 ? (
          <p className="text-sm text-neutre-700">
            Brise la glace — un petit mot met toutes les chances de ton côté.
          </p>
        ) : null}
        {messages.map((message) => {
          const mine = message.sender_pseudo === myPseudo;
          return (
            <div key={message.id} className={`flex flex-col ${mine ? "items-end" : "items-start"}`}>
              <div
                className={`max-w-[80%] rounded-3xl px-4 py-2 text-sm ${
                  mine ? "bg-[#c67139] text-creme" : "bg-creme"
                }`}
              >
                {message.photo_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={message.photo_url}
                    alt="Photo jointe"
                    className="mb-1 max-h-64 rounded-2xl object-contain"
                  />
                ) : null}
                {message.body ? (
                  <p className="whitespace-pre-line break-words">{message.body}</p>
                ) : null}
              </div>
              <span className="mt-0.5 text-[10px] text-neutre-700">
                {new Intl.DateTimeFormat("fr-FR", {
                  hour: "2-digit",
                  minute: "2-digit",
                }).format(new Date(message.created_at))}
                {mine && message.read_at ? " · Lu" : ""}
                {message.redacted ? " · Coordonnées masquées" : ""}
              </span>
            </div>
          );
        })}
        <div ref={bottom} aria-hidden />
      </div>

      {redactedNotice ? (
        <p className="rounded-3xl bg-terracotta-100/70 px-4 py-2.5 text-xs text-terracotta-800">
          On a masqué des coordonnées dans ton message. Garder les échanges ici te protège :
          c&apos;est ta seule preuve en cas de souci, et c&apos;est ce qui fait vivre Lebontroc.
          Elles seront partagées automatiquement une fois le troc accepté.
        </p>
      ) : null}
      {error ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}

      {closed ? (
        <p className="text-sm text-neutre-700">
          Cette proposition est close — la conversation est en lecture seule.
        </p>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
          className="flex items-center gap-2"
        >
          <input
            ref={fileInput}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void attachPhoto(file);
              e.target.value = "";
            }}
          />
          <button
            type="button"
            aria-label="Joindre une photo"
            onClick={() => fileInput.current?.click()}
            className="flex size-11 shrink-0 cursor-pointer items-center justify-center rounded-full border border-neutre-300 text-encre transition-colors hover:bg-encre/7"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.25" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              <path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" />
              <circle cx="12" cy="13" r="3" />
            </svg>
          </button>
          <input
            aria-label="Ton message"
            placeholder="Écris ton message…"
            value={draft}
            onChange={(e) => setDraft(e.target.value.slice(0, 2000))}
            className="min-w-0 flex-1 rounded-full border border-neutre-300 bg-creme px-4 py-2.5 text-sm outline-none transition-colors focus:border-terracotta-500"
          />
          <button
            type="submit"
            disabled={sending || draft.trim() === ""}
            className="flex min-h-11 shrink-0 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Envoyer
          </button>
        </form>
      )}
    </section>
  );
}

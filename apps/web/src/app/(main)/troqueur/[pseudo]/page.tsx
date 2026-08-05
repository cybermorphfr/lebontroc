import type { Metadata } from "next";
import Link from "next/link";
import { cookies } from "next/headers";
import { createApiClient } from "@lebontroc/api-client";

import { AvatarLetter } from "@/components/AvatarLetter";
import { ProfileActions } from "@/components/ProfileActions";
import { Tag } from "@/components/ui/Tag";
import { ancrage, CONDITION_LABELS } from "@/lib/format";
import { getCurrentUser } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function generateMetadata({
  params,
}: {
  params: Promise<{ pseudo: string }>;
}): Promise<Metadata> {
  const { pseudo } = await params;
  return { title: `${decodeURIComponent(pseudo)} — Lebontroc` };
}

export default async function TroqueurPage({
  params,
}: {
  params: Promise<{ pseudo: string }>;
}) {
  const { pseudo } = await params;
  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const { data: profile, response } = await client.GET("/troqueurs/{pseudo}", {
    params: { path: { pseudo: decodeURIComponent(pseudo) } },
    cache: "no-store",
  });

  if (!profile || response.status === 404) {
    return (
      <main className="mx-auto flex w-full max-w-xl flex-col items-start gap-4 px-6 py-16">
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h1 className="font-display text-2xl">Ce troqueur n&apos;existe pas — ou a plié bagage.</h1>
          <Link
            href="/"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Retour à l&apos;accueil
          </Link>
        </section>
      </main>
    );
  }

  const viewer = await getCurrentUser();
  const isOwner = viewer?.pseudo.toLowerCase() === profile.pseudo.toLowerCase();
  const count = profile.items.length;

  // F5.2 — l'état de blocage pour le bouton Bloquer/Débloquer.
  let initiallyBlocked = false;
  if (viewer && !isOwner) {
    const { data: blocks } = await client.GET("/me/blocks", { cache: "no-store" });
    initiallyBlocked =
      blocks?.pseudos.some((p) => p.toLowerCase() === profile.pseudo.toLowerCase()) ?? false;
  }

  return (
    <main className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 pb-16">
      {isOwner ? (
        <p className="flex flex-wrap items-center justify-center gap-2 rounded-full bg-sauge-100 px-5 py-2 text-sm text-sauge-800">
          C&apos;est ton profil vu par les autres.
          <Link href="/dressing" className="font-semibold underline">
            Gérer mon dressing
          </Link>
        </p>
      ) : null}

      <section className="flex items-center gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
        <AvatarLetter pseudo={profile.pseudo} size="lg" />
        <div className="flex flex-col gap-1">
          <h1 className="font-display text-3xl">{profile.pseudo}</h1>
          <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-neutre-700">
            {profile.city ? (
              <span className="inline-flex items-center gap-1">
                <PinIcon />
                {profile.city}
              </span>
            ) : null}
            <span>· {ancrage(profile.member_since)}</span>
          </p>
          <div className="flex flex-wrap items-center gap-2">
            {profile.reviews_count > 0 && profile.rating_avg != null ? (
              <Tag variant="accent-2">
                ★ {profile.rating_avg.toFixed(1)} · {profile.reviews_count} avis
              </Tag>
            ) : profile.trades_finalized === 0 ? (
              <Tag variant="accent-2">Nouveau troqueur</Tag>
            ) : null}
            {profile.trades_finalized > 0 ? (
              <Tag variant="neutral">
                {profile.trades_finalized} troc{profile.trades_finalized > 1 ? "s" : ""} finalisé
                {profile.trades_finalized > 1 ? "s" : ""}
              </Tag>
            ) : null}
            {profile.avg_ship_days != null ? (
              <Tag variant="neutral">
                📦 expédie en ~{Math.max(1, Math.round(profile.avg_ship_days))} j
              </Tag>
            ) : null}
          </div>
        </div>
      </section>

      {viewer && !isOwner ? (
        <ProfileActions
          pseudo={profile.pseudo}
          userId={profile.user_id}
          initiallyBlocked={initiallyBlocked}
        />
      ) : null}

      <div className="flex items-center gap-2">
        <h2 className="font-display text-xl">Son dressing</h2>
        <Tag variant="neutral">
          {count} objet{count > 1 ? "s" : ""}
        </Tag>
      </div>

      {count === 0 ? (
        <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
          <h3 className="font-display text-lg">Rien en ligne pour l&apos;instant</h3>
          <p className="text-sm text-neutre-700">
            {profile.pseudo} n&apos;a pas encore d&apos;objet à troquer. Repasse voir plus tard !
          </p>
        </section>
      ) : (
        <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3">
          {profile.items.map((item) => (
            <li key={item.id}>
              <Link
                href={`/objet/${item.id}?source=profile`}
                className="flex flex-col overflow-hidden rounded-3xl bg-sable shadow-sm transition-shadow hover:shadow-md"
              >
                <div className="aspect-square bg-neutre-100">
                  {item.photos[0] ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={item.photos[0].url}
                      alt={item.title}
                      className="size-full object-cover"
                    />
                  ) : null}
                </div>
                <div className="flex flex-col gap-1 p-3">
                  <span className="truncate text-sm font-semibold">{item.title}</span>
                  <div className="flex items-center justify-between gap-2 text-xs text-neutre-700">
                    <span>{CONDITION_LABELS[item.condition] ?? item.condition}</span>
                    <span>~{Math.round(item.value_cents / 100)} €</span>
                  </div>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}

      {profile.reviews.length > 0 ? (
        <>
          <h2 className="mt-2 font-display text-xl">Ses évaluations</h2>
          <ul className="flex flex-col gap-3">
            {profile.reviews.map((review, index) => (
              <li
                key={index}
                className="flex flex-col gap-1 rounded-3xl bg-sable p-4 shadow-sm"
              >
                <p className="flex items-center gap-2 text-sm">
                  <span className="text-terracotta-500" aria-label={`${review.rating} sur 5`}>
                    {"★".repeat(review.rating)}
                    <span className="text-neutre-300">{"★".repeat(5 - review.rating)}</span>
                  </span>
                  <span className="font-semibold">{review.reviewer_pseudo}</span>
                </p>
                {review.comment ? (
                  <p className="text-sm text-neutre-700">« {review.comment} »</p>
                ) : null}
                {review.reply ? (
                  <p className="rounded-2xl bg-creme px-3 py-2 text-xs text-neutre-700">
                    Réponse de {profile.pseudo} : « {review.reply} »
                  </p>
                ) : null}
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </main>
  );
}

function PinIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0" />
      <circle cx="12" cy="10" r="3" />
    </svg>
  );
}

import Link from "next/link";
import { cookies } from "next/headers";
import { createApiClient, type FeedResponse } from "@lebontroc/api-client";

import { getCurrentUser } from "@/lib/server-api";

import { FeedGrid } from "./FeedGrid";

// Le fil est recalculé à chaque requête (proximité + fraîcheur).
export const dynamic = "force-dynamic";

async function fetchFeed(): Promise<FeedResponse | null> {
  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  try {
    const { data } = await client.GET("/feed", { cache: "no-store" });
    return data ?? null;
  } catch {
    return null;
  }
}

export default async function Home() {
  const [user, feed] = await Promise.all([getCurrentUser(), fetchFeed()]);

  return (
    <main className="mx-auto w-full max-w-4xl px-6 pb-16">
      {user === null ? (
        <section className="mb-6 flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm sm:p-8">
          <h1 className="font-display text-3xl sm:text-4xl">
            Échange tes objets, sans argent.
          </h1>
          <p className="max-w-md text-neutre-700">
            Publie ce qui dort chez toi, repère ce qui te ferait plaisir près de chez toi, et
            troque.
          </p>
          <Link
            href="/inscription"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Je commence à troquer
          </Link>
        </section>
      ) : null}

      <div className="mb-4 flex items-baseline gap-2">
        <h2 className="font-display text-2xl">
          {user ? "Autour de toi" : "Les trouvailles du moment"}
        </h2>
        {user ? (
          <span className="text-sm text-neutre-700">les plus proches et les plus récents</span>
        ) : null}
      </div>

      {feed === null ? (
        <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
          <h3 className="font-display text-lg">Le fil fait des siennes</h3>
          <p className="text-sm text-neutre-700">
            On n&apos;arrive pas à charger les objets. Recharge la page dans un instant.
          </p>
        </section>
      ) : feed.items.length === 0 ? (
        <section className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
          <h3 className="font-display text-lg">Rien à troquer pour l&apos;instant</h3>
          <p className="text-sm text-neutre-700">
            Sois la première personne à publier un objet — le fil n&apos;attend que toi.
          </p>
          <Link
            href="/publier"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            Publier un objet
          </Link>
        </section>
      ) : (
        <FeedGrid initial={feed} />
      )}
    </main>
  );
}

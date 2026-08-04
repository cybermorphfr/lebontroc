import type { Metadata } from "next";
import { createApiClient, type HealthResponse } from "@lebontroc/api-client";

export const metadata: Metadata = {
  title: "Santé de la plateforme — Lebontroc",
};

// Le statut est lu à chaque requête, jamais figé au build.
export const dynamic = "force-dynamic";

async function fetchHealth(): Promise<HealthResponse | null> {
  // Côté serveur, l'API est jointe en direct sur le réseau interne Docker.
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080");
  try {
    const { data } = await client.GET("/health", { cache: "no-store" });
    return data ?? null;
  } catch {
    return null;
  }
}

function StatusBadge({ health }: { health: HealthResponse | null }) {
  if (health === null) {
    return (
      <span className="inline-flex items-center gap-2 rounded-full bg-terracotta-100 px-4 py-1.5 text-sm font-semibold text-terracotta-800">
        <span className="size-2 rounded-full bg-terracotta-500" aria-hidden />
        API injoignable
      </span>
    );
  }
  if (health.status === "ok") {
    return (
      <span className="inline-flex items-center gap-2 rounded-full bg-sauge-100 px-4 py-1.5 text-sm font-semibold text-sauge-800">
        <span className="size-2 rounded-full bg-sauge-500" aria-hidden />
        API opérationnelle
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-2 rounded-full bg-terracotta-100 px-4 py-1.5 text-sm font-semibold text-terracotta-800">
      <span className="size-2 rounded-full bg-terracotta-500" aria-hidden />
      API dégradée
    </span>
  );
}

export default async function SantePage() {
  const health = await fetchHealth();

  return (
    <main className="mx-auto flex w-full max-w-xl flex-col gap-8 px-6 py-16">
      <header className="flex flex-col gap-3">
        <h1 className="font-display text-5xl">Lebontroc</h1>
        <p className="max-w-md text-lg text-neutre-700">Échange tes objets, sans argent.</p>
      </header>

      <section
        aria-label="Statut de la plateforme"
        className="flex w-full max-w-md flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm"
      >
        <h2 className="font-display text-xl">Statut de la plateforme</h2>
        <StatusBadge health={health} />
        <dl className="flex flex-col gap-1 text-sm text-neutre-700">
          <div className="flex justify-between">
            <dt>Version du build</dt>
            <dd className="font-semibold text-encre">{health?.version ?? "—"}</dd>
          </div>
          <div className="flex justify-between">
            <dt>Base de données</dt>
            <dd className="font-semibold text-encre">
              {health === null ? "—" : health.db === "ok" ? "ok" : "injoignable"}
            </dd>
          </div>
        </dl>
      </section>
    </main>
  );
}

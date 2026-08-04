import type { Metadata } from "next";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { createApiClient } from "@lebontroc/api-client";

import { ResendVerification } from "@/components/ResendVerification";
import { Tag } from "@/components/ui/Tag";
import { getCurrentUser, getSessions } from "@/lib/server-api";

import { LogoutButton } from "./LogoutButton";
import { ProfileForm } from "./ProfileForm";
import { SessionsList } from "./SessionsList";
import { WishlistForm } from "./WishlistForm";

export const metadata: Metadata = {
  title: "Ton profil — Lebontroc",
};

export const dynamic = "force-dynamic";

export default async function ProfilPage() {
  const user = await getCurrentUser();
  if (!user) redirect("/connexion");
  const sessions = await getSessions();

  const jar = await cookies();
  const client = createApiClient(process.env.API_INTERNAL_URL ?? "http://localhost:8080", {
    cookie: jar
      .getAll()
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  });
  const [{ data: wishlist }, { data: categories }] = await Promise.all([
    client.GET("/me/wishlist", { cache: "no-store" }),
    client.GET("/categories", { cache: "no-store" }),
  ]);

  return (
    <main className="flex flex-col gap-6 px-6 pb-16 sm:px-12 lg:px-24">
      <section className="flex max-w-xl flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
        <div className="flex items-center gap-3">
          <span
            aria-hidden
            className="flex size-12 items-center justify-center rounded-full bg-terracotta-100 font-display text-xl text-terracotta-800"
          >
            {user.pseudo.charAt(0).toUpperCase()}
          </span>
          <h1 className="font-display text-3xl">Ton profil</h1>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-sm text-neutre-700">
          <span>{user.email}</span>
          {user.email_verified ? (
            <Tag variant="accent-2">Vérifié</Tag>
          ) : (
            <>
              <Tag variant="accent">À vérifier</Tag>
              <span className="text-terracotta-800">
                <ResendVerification />
              </span>
            </>
          )}
        </div>
        <ProfileForm pseudo={user.pseudo} postalCode={user.postal_code} />
      </section>

      <section
        id="envies"
        className="flex max-w-xl flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm"
      >
        <h2 className="font-display text-xl">Ce que je cherche</h2>
        <WishlistForm
          initial={wishlist ?? []}
          roots={(categories ?? []).map((c) => ({ id: c.id, label: c.label }))}
        />
      </section>

      <section className="flex max-w-xl flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">Tes appareils connectés</h2>
        <SessionsList sessions={sessions} />
      </section>

      <section className="flex max-w-xl flex-col items-start gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
        <LogoutButton />
      </section>
    </main>
  );
}

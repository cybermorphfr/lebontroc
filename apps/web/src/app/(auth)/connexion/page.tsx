import type { Metadata } from "next";

import { LoginForm } from "./LoginForm";

export const metadata: Metadata = {
  title: "Connexion — Lebontroc",
};

export default function ConnexionPage() {
  return (
    <section className="flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <header className="flex flex-col gap-1">
        <h1 className="font-display text-3xl">Content de te revoir</h1>
        <p className="text-neutre-700">Ton dressing n&apos;a pas bougé.</p>
      </header>
      <LoginForm />
    </section>
  );
}

import type { Metadata } from "next";

import { SignupForm } from "./SignupForm";

export const metadata: Metadata = {
  title: "Crée ton compte — Lebontroc",
};

export default function InscriptionPage() {
  return (
    <section className="flex flex-col gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      <header className="flex flex-col gap-1">
        <h1 className="font-display text-3xl">Crée ton compte</h1>
        <p className="text-neutre-700">Une minute, et ton dressing t&apos;attend.</p>
      </header>
      <SignupForm />
    </section>
  );
}

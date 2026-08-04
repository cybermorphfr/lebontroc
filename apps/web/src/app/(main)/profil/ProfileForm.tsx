"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { apiError, apiFetch } from "@/lib/client-api";

export function ProfileForm({ pseudo: initialPseudo, postalCode: initialPostal }: {
  pseudo: string;
  postalCode: string;
}) {
  const router = useRouter();
  const [pseudo, setPseudo] = useState(initialPseudo);
  const [postal, setPostal] = useState(initialPostal);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setLoading(true);
    setMessage(null);
    setError(null);
    const response = await apiFetch("/me", {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ pseudo, postal_code: postal }),
    });
    setLoading(false);
    if (response.ok) {
      setMessage("C'est noté !");
      router.refresh();
    } else {
      setError((await apiError(response)).message);
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-4" noValidate>
      <Input
        id="pseudo"
        label="Pseudo"
        hint="C'est le nom que verront les autres troqueurs."
        value={pseudo}
        onChange={(e) => setPseudo(e.target.value)}
      />
      <Input
        id="postal"
        label="Code postal"
        inputMode="numeric"
        hint="Pour te montrer les objets près de chez toi. On n'affiche jamais ton adresse — juste ta ville et une distance."
        value={postal}
        onChange={(e) => setPostal(e.target.value)}
      />
      {error ? <p className="text-sm text-terracotta-700">{error}</p> : null}
      {message ? <p className="text-sm text-sauge-700">{message}</p> : null}
      <div>
        <Button type="submit" variant="secondary" disabled={loading}>
          {loading ? "Enregistrement…" : "Enregistrer"}
        </Button>
      </div>
    </form>
  );
}

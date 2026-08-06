"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { PasswordInput } from "@/components/ui/PasswordInput";
import { apiError } from "@/lib/client-api";

export function LoginForm() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [totpDemande, setTotpDemande] = useState(false);
  const [totpCode, setTotpCode] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setLoading(true);
    setError(null);
    const response = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });
    if (response.ok) {
      // Compte protégé par la double authentification ? On vérifie le
      // second facteur avant d'aller plus loin.
      const statut = await fetch("/api/me/totp").then((r) => (r.ok ? r.json() : null));
      if (statut?.enabled && !statut.session_verified) {
        setLoading(false);
        setTotpDemande(true);
        return;
      }
      router.push("/");
      router.refresh();
      return;
    }
    setLoading(false);
    setError((await apiError(response)).message);
  }

  async function verifierTotp(event: React.FormEvent) {
    event.preventDefault();
    setLoading(true);
    setError(null);
    const response = await fetch("/api/auth/totp/verify", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code: totpCode }),
    });
    if (response.ok) {
      router.push("/");
      router.refresh();
      return;
    }
    setLoading(false);
    setError((await apiError(response)).message);
  }

  if (totpDemande) {
    return (
      <form onSubmit={verifierTotp} className="flex flex-col gap-4" noValidate>
        <p className="text-sm text-neutre-700">
          🔐 Ce compte est protégé : saisis le code de ton application
          d&apos;authentification — ou un code de secours.
        </p>
        <Input
          id="totp"
          label="Code de vérification"
          placeholder="000 000 ou XXXX-XXXX"
          value={totpCode}
          onChange={(e) => setTotpCode(e.target.value)}
          autoComplete="one-time-code"
          autoFocus
        />
        {error ? (
          <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
            {error}
          </p>
        ) : null}
        <Button type="submit" size="lg" block disabled={loading || totpCode.trim().length === 0}>
          {loading ? "Vérification…" : "Vérifier"}
        </Button>
      </form>
    );
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-4" noValidate>
      <Input
        id="email"
        type="email"
        label="E-mail"
        placeholder="toi@exemple.fr"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        autoComplete="email"
      />
      <PasswordInput
        id="password"
        label="Mot de passe"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        autoComplete="current-password"
      />

      {error ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}

      <Button type="submit" size="lg" block disabled={loading}>
        {loading ? "Connexion…" : "Me connecter"}
      </Button>

      <p className="text-center text-sm text-neutre-700">
        Pas encore de compte ?{" "}
        <Link href="/inscription" className="font-semibold text-encre underline">
          Inscris-toi
        </Link>
      </p>
    </form>
  );
}

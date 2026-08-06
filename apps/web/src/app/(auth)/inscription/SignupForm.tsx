"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { PasswordInput } from "@/components/ui/PasswordInput";
import { apiError } from "@/lib/client-api";

type FieldErrors = Partial<Record<"pseudo" | "email" | "password" | "postal", string>>;

const MESSAGES: Record<string, [keyof FieldErrors, string]> = {
  email_invalide: ["email", "Il nous faut un e-mail valide pour t'envoyer le lien de vérification."],
  email_pris: ["email", "Un compte existe déjà avec cet e-mail."],
  mot_de_passe_trop_court: ["password", "Encore un effort : 8 caractères minimum."],
  pseudo_invalide: ["pseudo", "3 à 30 caractères, lettres, chiffres, tirets ou underscore."],
  pseudo_pris: ["pseudo", "Ce pseudo est déjà pris. Tente une variante ?"],
  code_postal_invalide: ["postal", "Un code postal à 5 chiffres, comme 44000."],
};

export function SignupForm() {
  const router = useRouter();
  const [pseudo, setPseudo] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [postal, setPostal] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [globalError, setGlobalError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const started = useRef(false);

  function trackStart() {
    if (started.current) return;
    started.current = true;
    fetch("/api/analytics/track", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "signup_started" }),
    }).catch(() => {});
  }

  function validate(): FieldErrors {
    const next: FieldErrors = {};
    if (!/^[A-Za-z0-9_-]{3,30}$/.test(pseudo)) next.pseudo = MESSAGES.pseudo_invalide[1];
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) next.email = MESSAGES.email_invalide[1];
    if (password.length < 8) next.password = MESSAGES.mot_de_passe_trop_court[1];
    if (!/^\d{5}$/.test(postal)) next.postal = MESSAGES.code_postal_invalide[1];
    return next;
  }

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const clientErrors = validate();
    setErrors(clientErrors);
    if (Object.keys(clientErrors).length > 0) return;

    setLoading(true);
    setGlobalError(null);
    const response = await fetch("/api/auth/signup", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password, pseudo, postal_code: postal }),
    });
    if (response.ok) {
      router.push("/verification");
      return;
    }
    setLoading(false);
    const { code, message } = await apiError(response);
    const mapped = MESSAGES[code];
    if (mapped) {
      setErrors({ [mapped[0]]: message });
    } else {
      setGlobalError(message);
    }
  }

  return (
    <form onSubmit={submit} onFocus={trackStart} className="flex flex-col gap-4" noValidate>
      <Input
        id="pseudo"
        label="Pseudo"
        placeholder="Ex : camille_troc"
        hint="C'est le nom que verront les autres troqueurs."
        value={pseudo}
        onChange={(e) => setPseudo(e.target.value)}
        error={errors.pseudo}
        autoComplete="username"
      />
      <Input
        id="email"
        type="email"
        label="E-mail"
        placeholder="toi@exemple.fr"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        error={errors.email}
        autoComplete="email"
      />
      <PasswordInput
        id="password"
        label="Mot de passe"
        hint="8 caractères minimum."
        hintValid={password.length >= 8}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        error={errors.password}
        autoComplete="new-password"
      />
      <Input
        id="postal"
        label="Code postal"
        placeholder="Ex : 44000"
        inputMode="numeric"
        hint="Pour te montrer les objets près de chez toi. On n'affiche jamais ton adresse — juste ta ville et une distance."
        value={postal}
        onChange={(e) => setPostal(e.target.value)}
        error={errors.postal}
        autoComplete="postal-code"
      />

      {globalError ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {globalError}
        </p>
      ) : null}

      <Button type="submit" size="lg" block disabled={loading}>
        {loading ? "Création…" : "Créer mon compte"}
      </Button>

      <p className="text-center text-sm text-neutre-700">
        Déjà un compte ?{" "}
        <Link href="/connexion" className="font-semibold text-encre underline">
          Connecte-toi
        </Link>
      </p>
      <p className="text-center text-xs text-neutre-500">
        En créant ton compte, tu acceptes les{" "}
        <Link href="/cgu" className="underline">
          conditions d&apos;utilisation
        </Link>
        .
      </p>
    </form>
  );
}

